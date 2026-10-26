//! Deterministic keyword retrieval with explanation and abstention.
//!
//! Pipeline (master prompt §6):
//! 1. reduce the query to content terms ([`rekora_core::query::terms`]);
//! 2. FTS5 OR-match restricted to this scope and to *current* memories
//!    (superseded ones are filtered in SQL, before ranking);
//! 3. per-term matching to measure query coverage;
//! 4. abstain: drop candidates below the coverage floor; if nothing is left,
//!    return `no_evidence` instead of the nearest neighbour;
//! 5. score = relevance × kind priority × status weight × freshness × branch,
//!    every factor reported in [`ScoreExplain`].
//!
//! Scores order results; they are **not** calibrated probabilities of truth.

use std::collections::HashMap;

use rekora_core::SourcePolicy;
use rekora_core::query::{self, fts_literal};
use rusqlite::params;
use serde::Serialize;

pub use rekora_core::source::Freshness;

use crate::{MemoryRecord, Store, StoreError};

/// Most candidates pulled from FTS5 before re-ranking. Bounds CPU per query.
const CANDIDATE_LIMIT: usize = 200;
/// Hard ceiling for `limit`, protecting MCP callers from unbounded output.
pub const MAX_LIMIT: usize = 50;

/// Retrieval parameters.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Maximum hits returned (clamped to `1..=MAX_LIMIT`).
    pub limit: usize,
    /// Branch the caller is working on; memories recorded on another branch
    /// are down-weighted and flagged rather than silently mixed in.
    pub branch: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            branch: None,
        }
    }
}

/// Overall answer quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStatus {
    /// At least one hit covers half or more of the query terms.
    Found,
    /// Hits exist but each covers less than half of the query terms.
    LowConfidence,
    /// Nothing in this scope supports an answer.
    NoEvidence,
}

/// Why a hit ranked where it did. All factors multiply into `score`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreExplain {
    /// Query terms found in the memory.
    pub matched_terms: Vec<String>,
    /// `matched_terms / query terms`.
    pub coverage: f64,
    /// Raw FTS5 BM25 (lower is better, SQLite convention).
    pub bm25: f64,
    /// BM25 relative to the best candidate of this query, in `(0, 1]`.
    pub bm25_relative: f64,
    /// `0.6 × coverage + 0.4 × bm25_relative`.
    pub relevance: f64,
    /// From the memory kind.
    pub kind_priority: f64,
    /// From the verification status.
    pub status_weight: f64,
    /// 1.0 fresh or uncited, 0.7 changed source, 0.6 missing source.
    pub freshness_factor: f64,
    /// 1.0 same/unknown branch, 0.8 recorded on a different branch.
    pub branch_factor: f64,
}

/// One ranked result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// The memory.
    pub memory: MemoryRecord,
    /// Final ordering score (see [`ScoreExplain`]).
    pub score: f64,
    /// Per-factor breakdown.
    pub explain: ScoreExplain,
    /// Freshness per cited source, when a policy was supplied.
    pub source_freshness: Vec<(String, Freshness)>,
    /// True when any cited source changed or disappeared.
    pub possibly_stale: bool,
    /// True when the memory was recorded on a different branch than requested.
    pub branch_mismatch: bool,
}

/// Result of [`Store::search`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchOutcome {
    /// Overall status.
    pub status: SearchStatus,
    /// Content terms extracted from the query.
    pub terms: Vec<String>,
    /// Ranked hits, best first.
    pub hits: Vec<Hit>,
    /// FTS candidates examined before the coverage floor.
    pub candidates: usize,
    /// Explanation when `status` is `no_evidence`.
    pub reason: Option<String>,
}

impl SearchOutcome {
    fn none(terms: Vec<String>, candidates: usize, reason: &str) -> Self {
        Self {
            status: SearchStatus::NoEvidence,
            terms,
            hits: Vec::new(),
            candidates,
            reason: Some(reason.to_owned()),
        }
    }
}

/// Minimum matched terms for a candidate to count as evidence. One stray shared
/// word in a long query is not support for an answer.
pub fn coverage_floor(term_count: usize) -> usize {
    match term_count {
        0 => 0,
        1 | 2 => 1,
        3..=5 => 2,
        n => n.div_ceil(3),
    }
}

impl Store {
    /// Ranked, explained search over current memories in this scope.
    /// `policy` enables source freshness checks (re-hashing cited files).
    pub fn search(
        &self,
        query: &str,
        options: &SearchOptions,
        policy: Option<&SourcePolicy>,
    ) -> Result<SearchOutcome, StoreError> {
        let terms = query::terms(query);
        if terms.is_empty() {
            return Ok(SearchOutcome::none(
                terms,
                0,
                "query has no searchable terms",
            ));
        }
        let expression = terms
            .iter()
            .map(|t| fts_literal(t))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.id, bm25(memories_fts) AS rank
             FROM memories_fts JOIN memories m ON m.rowid = memories_fts.rowid
             WHERE memories_fts MATCH ?1 AND m.scope_id = ?2 AND m.status != 'superseded'
             ORDER BY rank, m.id LIMIT ?3",
        )?;
        let candidates: Vec<(String, f64)> = stmt
            .query_map(
                params![expression, self.scope_id, CANDIDATE_LIMIT as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<Result<_, _>>()?;
        if candidates.is_empty() {
            return Ok(SearchOutcome::none(
                terms,
                0,
                "no memory in this scope matches any query term",
            ));
        }

        let matched = self.matched_terms(&terms, &candidates)?;
        let floor = coverage_floor(terms.len());
        // BM25 is negative in SQLite; the most negative is the best match.
        let best = candidates.iter().map(|(_, s)| *s).fold(0.0_f64, f64::min);
        let mut hits = Vec::new();
        for (id, bm25) in &candidates {
            let matched_terms = matched.get(id).cloned().unwrap_or_default();
            if matched_terms.len() < floor {
                continue;
            }
            let memory = self.get(id)?;
            hits.push(score(
                memory,
                matched_terms,
                terms.len(),
                *bm25,
                best,
                options,
                policy,
            ));
        }
        if hits.is_empty() {
            return Ok(SearchOutcome::none(
                terms,
                candidates.len(),
                "candidates matched too few query terms to count as evidence",
            ));
        }
        // Deterministic order: score, then id as a stable tie-breaker.
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.memory.id.cmp(&b.memory.id))
        });
        hits.truncate(options.limit.clamp(1, MAX_LIMIT));
        let status = if hits.iter().any(|h| h.explain.coverage >= 0.5) {
            SearchStatus::Found
        } else {
            SearchStatus::LowConfidence
        };
        Ok(SearchOutcome {
            status,
            terms,
            hits,
            candidates: candidates.len(),
            reason: None,
        })
    }

    /// Which query terms each candidate contains, using the same FTS tokenizer
    /// as the main match so diacritics and case behave identically.
    fn matched_terms(
        &self,
        terms: &[String],
        candidates: &[(String, f64)],
    ) -> Result<HashMap<String, Vec<String>>, StoreError> {
        // Only the (≤ 200) candidates are checked per term. Enumerating every
        // document that contains a common term instead made search scale with
        // corpus size rather than with the candidate set.
        let ids = serde_json::to_string(&candidates.iter().map(|(id, _)| id).collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".into());
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.id FROM memories m
             JOIN memories_fts ON memories_fts.rowid = m.rowid
             WHERE m.id IN (SELECT value FROM json_each(?2))
               AND memories_fts MATCH ?1 AND +m.scope_id = ?3",
        )?;
        let mut matched: HashMap<String, Vec<String>> = HashMap::new();
        for term in terms {
            let rows = stmt.query_map(params![fts_literal(term), ids, self.scope_id], |r| {
                r.get::<_, String>(0)
            })?;
            for id in rows {
                matched.entry(id?).or_default().push(term.clone());
            }
        }
        Ok(matched)
    }
}

fn score(
    memory: MemoryRecord,
    matched_terms: Vec<String>,
    term_count: usize,
    bm25: f64,
    best: f64,
    options: &SearchOptions,
    policy: Option<&SourcePolicy>,
) -> Hit {
    let coverage = matched_terms.len() as f64 / term_count as f64;
    let bm25_relative = if best < 0.0 {
        (bm25 / best).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let relevance = 0.6 * coverage + 0.4 * bm25_relative;

    let source_freshness: Vec<(String, Freshness)> = match policy {
        Some(policy) => memory
            .sources
            .iter()
            .map(|s| (s.location(), policy.freshness(s)))
            .collect(),
        None => Vec::new(),
    };
    let freshness_factor = source_freshness
        .iter()
        .map(|(_, f)| match f {
            Freshness::Fresh => 1.0,
            Freshness::Changed => 0.7,
            Freshness::Missing => 0.6,
        })
        .fold(1.0, f64::min);
    let branch_mismatch = matches!(
        (&options.branch, &memory.branch),
        (Some(want), Some(have)) if want != have
    );
    let branch_factor = if branch_mismatch { 0.8 } else { 1.0 };
    let kind_priority = memory.kind.priority();
    let status_weight = memory.status.weight();
    let score = relevance * kind_priority * status_weight * freshness_factor * branch_factor;
    Hit {
        possibly_stale: freshness_factor < 1.0,
        branch_mismatch,
        source_freshness,
        score,
        explain: ScoreExplain {
            matched_terms,
            coverage,
            bm25,
            bm25_relative,
            relevance,
            kind_priority,
            status_weight,
            freshness_factor,
            branch_factor,
        },
        memory,
    }
}

#[cfg(test)]
mod tests {
    use super::coverage_floor;

    #[test]
    fn coverage_floor_grows_with_query_length() {
        assert_eq!(coverage_floor(0), 0);
        assert_eq!(coverage_floor(1), 1);
        assert_eq!(coverage_floor(2), 1);
        assert_eq!(coverage_floor(3), 2);
        assert_eq!(coverage_floor(5), 2);
        assert_eq!(coverage_floor(6), 2);
        assert_eq!(coverage_floor(16), 6);
    }
}
