# Decision log

Hypotheses and their accept/reject criteria. Status reflects measurements in this repository only.

| # | Hypothesis | Test | Result | Status |
|---|---|---|---|---|
| H1 | Keyword retrieval with a coverage floor can abstain on unanswerable questions without losing answerable ones | Relevance suite: 30 answerable / 10 unanswerable (`rekora benchmark run --suite relevance`) | Specificity 1.0; 0 answerable abstained (after H2) | Accepted for v0.1, needs a larger, independently labelled corpus |
| H2 | English stemming improves recall without hurting abstention | Same suite before/after migration v4 | P@1 0.833 → 0.933, R@5 0.900 → 1.000, MRR 0.867 → 0.967, specificity 1.0 → 1.0 | Accepted (ADR-0003). Risk: corpus written by the same author as the code |
| H3 | Per-project database files give rank isolation by construction | S03/R03 tests inspect raw JSON of searches and packs | No leakage observed | Accepted (ADR-0001) |
| H4 | One query per objective/remaining item beats one concatenated query for handoffs | `s02_handoff_between_two_agents_in_separate_processes` failed with concatenation, passes with multi-query | Constraint memory now included | Accepted |
| H5 | Hosts accept read-only tools without interactive approval | Codex 0.164.0 `exec` before/after annotations | Blocked → completed | Accepted for Codex; Claude Code `-p` uses `--allowedTools` |
| H6 | Embeddings would materially improve recall | Not run | — | Deferred; must beat H2 numbers on a corpus not written by us, offline, behind a feature flag |
| H7 | Lifecycle hooks (auto-capture) are worth their risk | Not run | — | Deferred; capture would land as `candidate` only |
| H8 | Jev-style probabilistic decision model adds value | Not run | — | Rejected for core; out of scope until a baseline exists |
| H9 | Skill generation (Hermes-style) helps coding continuity | Not run | — | Deferred; no automatic execution ever |
