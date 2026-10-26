//! Token budgeting for context packs.
//!
//! Rekora does not ship a model tokenizer (default install is model-free), so it
//! uses a deliberately conservative estimate: one token per 3 UTF-8 bytes,
//! rounded up. Common BPE tokenizers average roughly 4 bytes per token on
//! English prose, so the estimate over-counts and a pack that fits the estimate
//! should fit the real budget. This is a heuristic, not a measurement.
//!
//! Packing never truncates an item: cutting a decision mid-sentence could change
//! its meaning or detach it from its provenance, so items either fit whole or
//! are skipped.

/// Conservative token estimate for `text` (see module docs).
///
/// ```
/// assert_eq!(rekora_core::budget::estimate_tokens(""), 0);
/// assert_eq!(rekora_core::budget::estimate_tokens("abcd"), 2);
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(3)
}

/// Result of [`pack`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// Indices of the selected items, in input (rank) order.
    pub selected: Vec<usize>,
    /// Estimated tokens used, including `overhead`.
    pub used: usize,
    /// Indices skipped because they did not fit.
    pub skipped: Vec<usize>,
}

/// Greedily selects whole items in rank order while the running estimate stays
/// within `budget`. `overhead` (header text) is charged first; if it alone does
/// not fit, nothing is selected and `used` is 0.
///
/// Later, smaller items may still be selected after a larger one is skipped;
/// order is deterministic for identical input.
pub fn pack<S: AsRef<str>>(items: &[S], budget: usize, overhead: usize) -> Packed {
    let mut packed = Packed {
        selected: Vec::new(),
        used: 0,
        skipped: Vec::new(),
    };
    if overhead > budget {
        packed.skipped = (0..items.len()).collect();
        return packed;
    }
    packed.used = overhead;
    for (index, item) in items.iter().enumerate() {
        let cost = estimate_tokens(item.as_ref());
        if packed.used + cost <= budget {
            packed.used += cost;
            packed.selected.push(index);
        } else {
            packed.skipped.push(index);
        }
    }
    if packed.selected.is_empty() {
        packed.used = 0;
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_budget_selects_nothing() {
        let p = pack(&["abc"], 0, 0);
        assert!(p.selected.is_empty());
        assert_eq!(p.used, 0);
        assert_eq!(p.skipped, vec![0]);
    }

    #[test]
    fn never_exceeds_budget_and_is_deterministic() {
        let items: Vec<String> = (0..50).map(|i| "word ".repeat(i % 7 + 1)).collect();
        for budget in [0, 1, 7, 64, 256, 1024] {
            let a = pack(&items, budget, 5);
            assert!(a.used <= budget, "budget {budget} used {}", a.used);
            assert_eq!(a, pack(&items, budget, 5));
            assert_eq!(a.selected.len() + a.skipped.len(), items.len());
        }
    }

    #[test]
    fn skips_large_item_but_keeps_later_small_one() {
        let items = ["x".repeat(30), "y".repeat(300), "z".repeat(3)];
        let p = pack(&items, 12, 0);
        assert_eq!(p.selected, vec![0, 2]);
        assert_eq!(p.skipped, vec![1]);
    }

    #[test]
    fn overhead_larger_than_budget() {
        let p = pack(&["a"], 3, 4);
        assert!(p.selected.is_empty());
        assert_eq!(p.used, 0);
    }

    #[test]
    fn multibyte_text_counts_bytes() {
        assert_eq!(estimate_tokens("ñ"), 1);
        assert_eq!(estimate_tokens("数据库"), 3);
    }
}
