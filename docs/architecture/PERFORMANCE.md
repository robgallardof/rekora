# Performance and retrieval quality

All numbers below were produced on **2026-10-26** with the commands shown. Nothing here is extrapolated.

**Machine:** AMD Ryzen 9 5950X (16 cores / 32 threads), 64 GB RAM, Samsung SSD 980 PRO 1 TB, Windows 11 Pro.
**Build:** `cargo build --release`, rustc 1.99.1, Rekora 0.1.0 (schema v4). Single run per configuration;
no variance across repeated runs has been collected yet. Peak RSS is not measured on Windows (`null` in the JSON).

## Retrieval quality (`rekora benchmark run --suite relevance`)

Corpus: [`fixtures/benchmark/relevance.json`](../../fixtures/benchmark/relevance.json) — 40 memories of a fictional
order system (1 superseded, 1 disputed, 1 candidate), 30 answerable questions with one labelled answer each,
10 unanswerable questions. **Written by the same author as the ranking code**, so treat it as a regression check,
not an independent evaluation.

| Metric | Before stemming (schema v3) | After (schema v4) |
|---|---|---|
| Precision@1 | 0.833 | **0.933** |
| Recall@5 | 0.900 | **1.000** |
| MRR | 0.867 | **0.967** |
| Answerable questions wrongly answered `no_evidence` | 3 | **0** |
| No-answer specificity (10 unanswerable → `no_evidence`) | 1.0 | **1.0** |
| Mean context tokens, budget 1024 | 88 | 98 |

## Latency and throughput (`rekora benchmark run --suite core --size 1000 --size 10000 --size 100000 --queries 300`)

Synthetic corpus, seed 42: sentences from a ~70-word vocabulary, so a typical query term matches 5–15 % of all
memories. That is a near worst case for BM25 candidate generation; real projects have far more varied text.
Every fifth query is unanswerable. Times are wall-clock per call inside one process (no process start-up).

### Before the index fix (same day, same machine)

| Memories | Inserts/s | Cold open + first search | Warm search p50 / p95 / p99 |
|---|---|---|---|
| 1 000 | 2 492 | 15 ms | 9.8 / 13.4 / 14.6 ms |
| 10 000 | 336 | 1 278 ms | 1 313 / 2 104 / 2 391 ms |
| 100 000 | 26 | 14 136 ms | 14 136 / 14 977 / 15 906 ms |

Cause: id lookups used the `(scope_id, status)` index instead of the id index (see ADR-0003).

### After the fix

| Memories | Inserts/s | Cold open + first search | Warm search p50 / p95 / p99 | Context 1024 p50 / p95 | DB size |
|---|---|---|---|---|---|
| 1 000 | 3 044 | 6.7 ms | 2.8 / 3.6 / 3.9 ms | 2.7 / 3.6 ms | 0.95 MB |
| 10 000 | 2 295 | 22.7 ms | 20.2 / 28.9 / 30.4 ms | 19.4 / 24.8 ms | 8.5 MB |
| 100 000 | 1 720 | 139 ms | 131 / 182 / 247 ms | 139 / 253 ms | 84 MB |

Context packs: mean used tokens 196 / ~800 / ~1 580 for budgets 256 / 1024 / 4096 — always within budget.

### Against the targets in the plan

| Target (plan, not a promise) | Result |
|---|---|
| Warm search p95 < 100 ms at 100k short memories | **Not met** on this corpus (182 ms). Met at 10k (29 ms). |
| CLI start p95 < 250 ms | Not measured as a distribution. Cold open + first search is 7 ms (1k), 23 ms (10k), 139 ms (100k). |
| No idle CPU | Met by construction: there is no daemon. |

Remaining cost at 100k is FTS5 scoring every document that contains a query term before taking the top 200.
Options not yet tried: FTS5's built-in `rank` ordering with a smaller candidate limit, per-term document-frequency
cut-offs, or splitting very common terms out of the OR expression. Any change must keep the relevance numbers above.

## Not measured yet

Multiple runs and variance, peak memory, Linux/macOS numbers, `rekora index` throughput on large real repositories,
and any comparison with Kindex on the same corpus.
