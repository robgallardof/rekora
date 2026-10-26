# ADR-0003 — Porter stemming and query-plan discipline

- Status: accepted
- Date: 2026-10-26

## Context

Two problems surfaced from real use and measurement, not design:

1. Codex 0.164.0 searched "invoice archiving" for a memory saying "Invoices are archived" and received
   `no_evidence` — correct by our rules, useless to the user.
2. The first `core` benchmark showed warm search p50 ≈ 1.3 s at 10k memories and ≈ 14 s at 100k, and inserts
   falling to 26/s at 100k.

## Decision

1. **Migration v4** rebuilds the FTS5 table with `tokenize = 'porter unicode61 remove_diacritics 2'`.
   Relevance suite (40 memories, 30 answerable, 10 unanswerable): P@1 0.833 → 0.933, R@5 0.900 → 1.000,
   MRR 0.867 → 0.967, no-answer specificity 1.0 → 1.0.
2. **Id lookups must use the UNIQUE(id) index.** Because each database holds exactly one scope, SQLite's
   planner preferred the `(scope_id, status)` index for `WHERE scope_id = ? AND id …`, which matches every row.
   Lookups now write `+scope_id` (disables that index for the term) and prefix lookups use a range
   (`id >= p AND id < p || 'g'`) instead of `LIKE`, which cannot use a case-sensitive index. A unit test
   asserts the query plan. Per-term coverage checks now run only over the ≤ 200 candidates.

## Consequences

- English inflections match; Spanish words are stemmed by English rules on both sides (exact words still match;
  Spanish inflections generally do not). A Spanish stemmer is future work.
- Numbers before/after are in [PERFORMANCE.md](../architecture/PERFORMANCE.md).
- Rule for contributors: any new lookup on `memories` must be checked with `EXPLAIN QUERY PLAN` at 10k+ rows.
