//! Reproducible offline benchmarks (`rekora benchmark run`).
//!
//! Both suites run in a fresh temporary directory and never touch the current
//! project:
//!
//! - `core`: seeded synthetic corpora (default 1k and 10k memories) measuring
//!   insert throughput, cold open+first search, warm search and context-pack
//!   latency distributions;
//! - `relevance`: the hand-labelled corpus in `fixtures/benchmark/relevance.json`
//!   (embedded at build time) measuring precision@1, recall@5, MRR, no-answer
//!   specificity and context tokens.
//!
//! Results include the environment so numbers are never quoted without their
//! machine. Peak RSS is read from `/proc/self/status` on Linux and reported as
//! `null` elsewhere rather than guessed.

use std::time::Instant;

use anyhow::{Context, Result};
use rekora_core::{MemoryKind, MemoryStatus, NewMemory, Origin};
use rekora_store::{SearchOptions, SearchStatus, Store};
use serde::Deserialize;
use serde_json::{Value, json};

const RELEVANCE: &str = include_str!("../../../fixtures/benchmark/relevance.json");

/// Runs a suite and returns its JSON report.
pub fn run(suite: &str, sizes: &[usize], queries: usize, seed: u64) -> Result<Value> {
    let report = match suite {
        "core" => core(sizes, queries, seed)?,
        "relevance" => relevance()?,
        other => anyhow::bail!("unknown suite `{other}`; available: core, relevance"),
    };
    Ok(json!({ "suite": suite, "environment": environment(), "results": report }))
}

fn environment() -> Value {
    json!({
        "rekora_version": env!("CARGO_PKG_VERSION"),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).ok(),
        "peak_rss_kib": peak_rss_kib(),
    })
}

fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
}

/// Deterministic xorshift64* so corpora are identical for the same seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

const SUBJECTS: &[&str] = &[
    "order",
    "invoice",
    "payment",
    "shipment",
    "customer",
    "tenant",
    "product",
    "inventory",
    "discount",
    "refund",
    "report",
    "export",
    "webhook",
    "session",
    "token",
    "cache",
    "queue",
    "migration",
    "search",
    "notification",
    "warehouse",
    "carrier",
    "catalog",
    "pricing",
];
const VERBS: &[&str] = &[
    "uses",
    "requires",
    "rejects",
    "caches",
    "validates",
    "retries",
    "streams",
    "locks",
    "encrypts",
    "batches",
    "indexes",
    "logs",
    "throttles",
    "archives",
    "replicates",
];
const OBJECTS: &[&str] = &[
    "optimistic concurrency",
    "a rowversion column",
    "redis",
    "sql server",
    "jwt tokens",
    "exponential backoff",
    "utc timestamps",
    "decimal amounts",
    "idempotency keys",
    "background jobs",
    "an outbox table",
    "feature flags",
    "gzip compression",
    "tenant filters",
    "signed urls",
    "rate limits",
    "structured logs",
    "blob storage",
    "read replicas",
];

fn synthetic(rng: &mut Rng, i: usize) -> NewMemory {
    let kinds = MemoryKind::ALL;
    NewMemory {
        kind: kinds[(rng.next() % kinds.len() as u64) as usize],
        content: format!(
            "The {} service {} {} for {} flows (note {i}).",
            rng.pick(SUBJECTS),
            rng.pick(VERBS),
            rng.pick(OBJECTS),
            rng.pick(SUBJECTS)
        ),
        origin: Origin::Human,
        status: if rng.next().is_multiple_of(4) {
            MemoryStatus::Candidate
        } else {
            MemoryStatus::Verified
        },
        sources: vec![],
        branch: None,
        commit: None,
    }
}

fn percentiles(mut micros: Vec<f64>) -> Value {
    micros.sort_by(f64::total_cmp);
    let at = |p: f64| {
        if micros.is_empty() {
            return 0.0;
        }
        let idx = ((p / 100.0) * (micros.len() - 1) as f64).round() as usize;
        (micros[idx] / 1000.0 * 1000.0).round() / 1000.0
    };
    json!({
        "n": micros.len(),
        "p50_ms": at(50.0),
        "p95_ms": at(95.0),
        "p99_ms": at(99.0),
        "max_ms": at(100.0),
    })
}

fn core(sizes: &[usize], queries: usize, seed: u64) -> Result<Value> {
    let mut out = Vec::new();
    for &size in sizes {
        let dir = tempfile::tempdir().context("creating temp dir")?;
        let path = dir.path().join("bench.db");
        let mut rng = Rng(seed.max(1));
        let (mut store, _) = Store::init(&path)?;
        let started = Instant::now();
        for i in 0..size {
            store.add(synthetic(&mut rng, i).validate()?, "bench")?;
        }
        let insert_secs = started.elapsed().as_secs_f64();
        drop(store);

        let query_list: Vec<String> = (0..queries.max(1))
            .map(|i| {
                if i % 5 == 4 {
                    // Every fifth query is unanswerable, to time the abstain path.
                    format!("kubernetes helm rollout {i}")
                } else {
                    format!(
                        "{} {} {}",
                        rng.pick(SUBJECTS),
                        rng.pick(VERBS),
                        rng.pick(OBJECTS)
                    )
                }
            })
            .collect();

        let cold = Instant::now();
        let store = Store::open(&path)?;
        store.search(&query_list[0], &SearchOptions::default(), None)?;
        let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;

        let mut search_us = Vec::new();
        let mut no_evidence = 0;
        for q in &query_list {
            let t = Instant::now();
            let outcome = store.search(q, &SearchOptions::default(), None)?;
            search_us.push(t.elapsed().as_secs_f64() * 1e6);
            if outcome.status == SearchStatus::NoEvidence {
                no_evidence += 1;
            }
        }
        let mut context = serde_json::Map::new();
        for budget in [256usize, 1024, 4096] {
            let mut us = Vec::new();
            let mut tokens = Vec::new();
            for q in query_list.iter().take(50) {
                let t = Instant::now();
                let pack = store.context(q, budget, None, None)?;
                us.push(t.elapsed().as_secs_f64() * 1e6);
                tokens.push(pack.used_tokens as f64);
            }
            let mean_tokens = tokens.iter().sum::<f64>() / tokens.len().max(1) as f64;
            context.insert(
                budget.to_string(),
                json!({ "latency": percentiles(us), "mean_used_tokens": mean_tokens.round() }),
            );
        }
        let db_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            + std::fs::metadata(path.with_extension("db-wal"))
                .map(|m| m.len())
                .unwrap_or(0);
        out.push(json!({
            "memories": size,
            "seed": seed,
            "insert_per_sec": (size as f64 / insert_secs).round(),
            "cold_open_and_first_search_ms": (cold_ms * 1000.0).round() / 1000.0,
            "warm_search": percentiles(search_us),
            "no_evidence_queries": no_evidence,
            "context": context,
            "db_bytes": db_bytes,
        }));
    }
    Ok(Value::Array(out))
}

#[derive(Deserialize)]
struct Corpus {
    name: String,
    memories: Vec<LabelledMemory>,
    questions: Vec<Question>,
    unanswerable: Vec<String>,
}

#[derive(Deserialize)]
struct LabelledMemory {
    key: String,
    kind: String,
    status: String,
    content: String,
}

#[derive(Deserialize)]
struct Question {
    q: String,
    answer: String,
}

/// Loads the labelled corpus into `store`, returning `memory id -> key`.
fn load_corpus(
    store: &mut Store,
    corpus: &Corpus,
) -> Result<std::collections::HashMap<String, String>> {
    let mut ids = std::collections::HashMap::new();
    let mut by_key = std::collections::HashMap::new();
    let mut supersede = Vec::new();
    for m in &corpus.memories {
        let (status, replaced_by) = match m.status.strip_prefix("superseded_by:") {
            Some(by) => (MemoryStatus::Verified, Some(by.to_owned())),
            None => (m.status.parse()?, None),
        };
        let disputed = status == MemoryStatus::Disputed;
        let id = store
            .add(
                NewMemory {
                    kind: m.kind.parse()?,
                    content: m.content.clone(),
                    origin: Origin::Human,
                    status: if disputed {
                        MemoryStatus::Candidate
                    } else {
                        status
                    },
                    sources: vec![],
                    branch: None,
                    commit: None,
                }
                .validate()?,
                "bench",
            )?
            .memory
            .id;
        if disputed {
            store.set_status(&id, MemoryStatus::Disputed, None, "bench")?;
        }
        if let Some(by) = replaced_by {
            supersede.push((id.clone(), by));
        }
        by_key.insert(m.key.clone(), id.clone());
        ids.insert(id, m.key.clone());
    }
    for (old, by) in supersede {
        let new = by_key
            .get(&by)
            .with_context(|| format!("corpus supersedes with unknown key {by}"))?;
        store.supersede(&old, new, "bench")?;
    }
    Ok(ids)
}

fn relevance() -> Result<Value> {
    let corpus: Corpus = serde_json::from_str(RELEVANCE).context("parsing relevance corpus")?;
    let dir = tempfile::tempdir()?;
    let (mut store, _) = Store::init(&dir.path().join("relevance.db"))?;
    let keys = load_corpus(&mut store, &corpus)?;

    let mut hits_at_1 = 0;
    let mut hits_at_5 = 0;
    let mut reciprocal = 0.0;
    let mut wrongly_abstained = 0;
    let mut misses = Vec::new();
    let mut tokens = Vec::new();
    for question in &corpus.questions {
        let outcome = store.search(&question.q, &SearchOptions::default(), None)?;
        let ranked: Vec<&str> = outcome
            .hits
            .iter()
            .filter_map(|h| keys.get(&h.memory.id).map(String::as_str))
            .collect();
        match ranked.iter().position(|k| *k == question.answer) {
            Some(pos) => {
                if pos == 0 {
                    hits_at_1 += 1;
                }
                if pos < 5 {
                    hits_at_5 += 1;
                }
                reciprocal += 1.0 / (pos + 1) as f64;
            }
            None => misses.push(
                json!({ "q": question.q, "expected": question.answer, "got": ranked.first() }),
            ),
        }
        if outcome.status == SearchStatus::NoEvidence {
            wrongly_abstained += 1;
        }
        tokens.push(store.context(&question.q, 1024, None, None)?.used_tokens);
    }
    let mut correctly_abstained = 0;
    let mut false_answers = Vec::new();
    for q in &corpus.unanswerable {
        let outcome = store.search(q, &SearchOptions::default(), None)?;
        if outcome.status == SearchStatus::NoEvidence {
            correctly_abstained += 1;
        } else {
            false_answers.push(json!({
                "q": q,
                "status": outcome.status,
                "top": outcome.hits.first().and_then(|h| keys.get(&h.memory.id)),
            }));
        }
    }
    let n = corpus.questions.len() as f64;
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    Ok(json!({
        "corpus": corpus.name,
        "memories": corpus.memories.len(),
        "answerable": corpus.questions.len(),
        "unanswerable": corpus.unanswerable.len(),
        "precision_at_1": round(hits_at_1 as f64 / n),
        "recall_at_5": round(hits_at_5 as f64 / n),
        "mrr": round(reciprocal / n),
        "answerable_wrongly_abstained": wrongly_abstained,
        "no_answer_specificity": round(correctly_abstained as f64 / corpus.unanswerable.len() as f64),
        "mean_context_tokens_budget_1024": round(tokens.iter().sum::<usize>() as f64 / n),
        "misses": misses,
        "false_answers": false_answers,
    }))
}
