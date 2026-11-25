//! Token usage and estimated cost.
//!
//! Rekora never sends model requests, so it cannot know what a session cost.
//! What it can do is read the usage the host already recorded (Claude Code
//! transcripts, Codex rollouts) and multiply it by a dated list price. The
//! result is an **API-equivalent estimate**: subscription plans are not billed
//! per token, providers change prices, and hosts may apply discounts. Every
//! figure this module produces must be shown with that label and with
//! [`PRICES_AS_OF`].
//!
//! Token categories follow the providers' own split, because they are priced
//! differently by an order of magnitude: under prompt caching most input of a
//! long session is a cheap cache read, so pricing all input at the base rate
//! would overstate cost several times (docs/research/TOKEN_ECONOMICS.md).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Token counts of one or more model requests, in the providers' categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Requests counted.
    pub requests: u64,
    /// Input tokens neither read from nor written to the prompt cache.
    pub uncached_input: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to a short-lived cache (Anthropic 5 minutes).
    pub cache_write_short: u64,
    /// Input tokens written to a long-lived cache (Anthropic 1 hour).
    pub cache_write_long: u64,
    /// Output tokens, reasoning included.
    pub output: u64,
}

impl Usage {
    /// Adds `other` to `self`.
    pub fn add(&mut self, other: &Usage) {
        self.requests += other.requests;
        self.uncached_input += other.uncached_input;
        self.cache_read += other.cache_read;
        self.cache_write_short += other.cache_write_short;
        self.cache_write_long += other.cache_write_long;
        self.output += other.output;
    }

    /// `self − earlier`, never below zero per field: the growth of a running
    /// total between two observations of the same session.
    pub fn since(&self, earlier: &Usage) -> Usage {
        Usage {
            requests: self.requests.saturating_sub(earlier.requests),
            uncached_input: self.uncached_input.saturating_sub(earlier.uncached_input),
            cache_read: self.cache_read.saturating_sub(earlier.cache_read),
            cache_write_short: self
                .cache_write_short
                .saturating_sub(earlier.cache_write_short),
            cache_write_long: self
                .cache_write_long
                .saturating_sub(earlier.cache_write_long),
            output: self.output.saturating_sub(earlier.output),
        }
    }

    /// All input tokens, whatever their cache category.
    pub fn total_input(&self) -> u64 {
        self.uncached_input + self.cache_read + self.cache_write_short + self.cache_write_long
    }

    /// True when nothing was counted.
    pub fn is_empty(&self) -> bool {
        *self == Usage::default()
    }
}

/// Usage per model id, as hosts report it (`claude-opus-5-5`, `gpt-6.1-sol`).
pub type ModelUsage = BTreeMap<String, Usage>;

/// Adds every model of `other` into `into`.
pub fn merge(into: &mut ModelUsage, other: &ModelUsage) {
    for (model, usage) in other {
        into.entry(model.clone()).or_default().add(usage);
    }
}

/// List price of one model, in US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    /// Model id, or a prefix of it; the longest matching prefix wins.
    pub model: &'static str,
    /// Base (uncached) input.
    pub input: f64,
    /// Cache read.
    pub cache_read: f64,
    /// Cache write, short TTL (or the only write price).
    pub cache_write_short: f64,
    /// Cache write, long TTL.
    pub cache_write_long: f64,
    /// Output (reasoning included).
    pub output: f64,
}

/// Date of the price list below. Shown next to every estimate.
pub const PRICES_AS_OF: &str = "2026-11-24";

/// Dated list prices (USD per million tokens) from the providers' official
/// pricing pages, fetched on [`PRICES_AS_OF`]; sources and caveats in
/// docs/research/MODEL_DELEGATION.md. Prefixes let one row cover dated model
/// ids (`claude-haiku-5-5-20261001`).
///
/// Known simplifications: long-context tiers (Haiku 5.5 above 100K prompt
/// tokens, OpenAI above 272K) are priced at the base tier, which
/// *under*-estimates those requests; DeepSeek is priced at its peak rate, which
/// *over*-estimates off-peak use; OpenAI's two write columns are its 30-minute
/// and 12-hour cache writes; providers without write charges repeat the input
/// price there. Gemini 4 Carbon/Argon prices double on 2026-12-01 and are not
/// updated here yet.
pub const PRICES: &[Price] = &[
    Price {
        model: "claude-fable-5-2",
        input: 8.0,
        cache_read: 0.2,
        cache_write_short: 10.0,
        cache_write_long: 16.0,
        output: 40.0,
    },
    Price {
        model: "claude-mythos-5-2",
        input: 8.0,
        cache_read: 0.2,
        cache_write_short: 10.0,
        cache_write_long: 16.0,
        output: 40.0,
    },
    Price {
        model: "claude-fable-5-1",
        input: 10.0,
        cache_read: 0.25,
        cache_write_short: 12.5,
        cache_write_long: 20.0,
        output: 50.0,
    },
    Price {
        model: "claude-mythos-5-1",
        input: 10.0,
        cache_read: 0.25,
        cache_write_short: 12.5,
        cache_write_long: 20.0,
        output: 50.0,
    },
    Price {
        model: "claude-fable-5",
        input: 10.0,
        cache_read: 1.0,
        cache_write_short: 12.5,
        cache_write_long: 20.0,
        output: 50.0,
    },
    Price {
        model: "claude-mythos-5",
        input: 10.0,
        cache_read: 1.0,
        cache_write_short: 12.5,
        cache_write_long: 20.0,
        output: 50.0,
    },
    Price {
        model: "claude-opus-5-5",
        input: 4.0,
        cache_read: 0.2,
        cache_write_short: 5.0,
        cache_write_long: 8.0,
        output: 20.0,
    },
    Price {
        model: "claude-opus-5",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 6.25,
        cache_write_long: 10.0,
        output: 25.0,
    },
    Price {
        model: "claude-opus-4-8",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 6.25,
        cache_write_long: 10.0,
        output: 25.0,
    },
    Price {
        model: "claude-opus-4-7",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 6.25,
        cache_write_long: 10.0,
        output: 25.0,
    },
    Price {
        model: "claude-opus-4-6",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 6.25,
        cache_write_long: 10.0,
        output: 25.0,
    },
    Price {
        model: "claude-opus-4-5",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 6.25,
        cache_write_long: 10.0,
        output: 25.0,
    },
    Price {
        model: "claude-opus-4-1",
        input: 15.0,
        cache_read: 1.5,
        cache_write_short: 18.75,
        cache_write_long: 30.0,
        output: 75.0,
    },
    Price {
        model: "claude-opus-4",
        input: 15.0,
        cache_read: 1.5,
        cache_write_short: 18.75,
        cache_write_long: 30.0,
        output: 75.0,
    },
    Price {
        model: "claude-sonnet-5-5",
        input: 2.0,
        cache_read: 0.1,
        cache_write_short: 2.5,
        cache_write_long: 4.0,
        output: 10.0,
    },
    Price {
        model: "claude-sonnet-5",
        input: 2.0,
        cache_read: 0.2,
        cache_write_short: 2.5,
        cache_write_long: 4.0,
        output: 10.0,
    },
    Price {
        model: "claude-sonnet-4",
        input: 3.0,
        cache_read: 0.3,
        cache_write_short: 3.75,
        cache_write_long: 6.0,
        output: 15.0,
    },
    Price {
        model: "claude-haiku-5-5",
        input: 0.1,
        cache_read: 0.01,
        cache_write_short: 0.125,
        cache_write_long: 0.2,
        output: 0.5,
    },
    Price {
        model: "claude-haiku-4-5",
        input: 1.0,
        cache_read: 0.1,
        cache_write_short: 1.25,
        cache_write_long: 2.0,
        output: 5.0,
    },
    Price {
        model: "gpt-6.2-astra",
        input: 8.0,
        cache_read: 0.4,
        cache_write_short: 10.0,
        cache_write_long: 20.0,
        output: 40.0,
    },
    Price {
        model: "gpt-6.2-sol",
        input: 2.0,
        cache_read: 0.1,
        cache_write_short: 2.5,
        cache_write_long: 5.0,
        output: 10.0,
    },
    Price {
        model: "gpt-6.1-sol",
        input: 2.0,
        cache_read: 0.1,
        cache_write_short: 2.5,
        cache_write_long: 5.0,
        output: 10.0,
    },
    Price {
        model: "gpt-6.1-terra",
        input: 1.0,
        cache_read: 0.05,
        cache_write_short: 1.25,
        cache_write_long: 2.5,
        output: 5.0,
    },
    Price {
        model: "gpt-6-astra",
        input: 10.0,
        cache_read: 1.0,
        cache_write_short: 12.5,
        cache_write_long: 25.0,
        output: 50.0,
    },
    Price {
        model: "gpt-6-sol",
        input: 2.0,
        cache_read: 0.2,
        cache_write_short: 2.5,
        cache_write_long: 5.0,
        output: 10.0,
    },
    Price {
        model: "gpt-6-luna",
        input: 0.1,
        cache_read: 0.01,
        cache_write_short: 0.125,
        cache_write_long: 0.25,
        output: 0.5,
    },
    Price {
        model: "gpt-5.6-sol",
        input: 4.0,
        cache_read: 0.4,
        cache_write_short: 5.0,
        cache_write_long: 10.0,
        output: 20.0,
    },
    Price {
        model: "gpt-5.6-terra",
        input: 2.0,
        cache_read: 0.2,
        cache_write_short: 2.5,
        cache_write_long: 5.0,
        output: 12.0,
    },
    Price {
        model: "gpt-5.6-luna",
        input: 0.2,
        cache_read: 0.02,
        cache_write_short: 0.25,
        cache_write_long: 0.5,
        output: 1.2,
    },
    Price {
        model: "gpt-5.5",
        input: 5.0,
        cache_read: 0.5,
        cache_write_short: 5.0,
        cache_write_long: 5.0,
        output: 30.0,
    },
    Price {
        model: "gpt-5.4-mini",
        input: 0.75,
        cache_read: 0.075,
        cache_write_short: 0.75,
        cache_write_long: 0.75,
        output: 4.5,
    },
    Price {
        model: "gpt-5.4",
        input: 2.5,
        cache_read: 0.25,
        cache_write_short: 2.5,
        cache_write_long: 2.5,
        output: 15.0,
    },
    Price {
        model: "gpt-5.3-codex",
        input: 1.75,
        cache_read: 0.175,
        cache_write_short: 1.75,
        cache_write_long: 1.75,
        output: 14.0,
    },
    Price {
        model: "gpt-5.2",
        input: 1.75,
        cache_read: 0.175,
        cache_write_short: 1.75,
        cache_write_long: 1.75,
        output: 14.0,
    },
    Price {
        model: "gpt-5.1",
        input: 1.25,
        cache_read: 0.125,
        cache_write_short: 1.25,
        cache_write_long: 1.25,
        output: 10.0,
    },
    Price {
        model: "gpt-5",
        input: 1.25,
        cache_read: 0.125,
        cache_write_short: 1.25,
        cache_write_long: 1.25,
        output: 10.0,
    },
    Price {
        model: "gemini-4-carbon",
        input: 2.0,
        cache_read: 0.1,
        cache_write_short: 2.0,
        cache_write_long: 2.0,
        output: 10.0,
    },
    Price {
        model: "gemini-4-argon",
        input: 2.0,
        cache_read: 0.1,
        cache_write_short: 2.0,
        cache_write_long: 2.0,
        output: 10.0,
    },
    Price {
        model: "gemini-4-neon",
        input: 3.0,
        cache_read: 0.15,
        cache_write_short: 3.0,
        cache_write_long: 3.0,
        output: 15.0,
    },
    Price {
        model: "gemini-4-hydrogen",
        input: 1.5,
        cache_read: 0.075,
        cache_write_short: 1.5,
        cache_write_long: 1.5,
        output: 7.5,
    },
    Price {
        model: "gemini-3.8-flash",
        input: 0.75,
        cache_read: 0.075,
        cache_write_short: 0.75,
        cache_write_long: 0.75,
        output: 3.75,
    },
    Price {
        model: "gemini-3.6-flash",
        input: 0.75,
        cache_read: 0.075,
        cache_write_short: 0.75,
        cache_write_long: 0.75,
        output: 3.75,
    },
    Price {
        model: "gemini-3.5-flash-lite",
        input: 0.3,
        cache_read: 0.03,
        cache_write_short: 0.3,
        cache_write_long: 0.3,
        output: 2.5,
    },
    Price {
        model: "gemini-3.1-flash-lite",
        input: 0.25,
        cache_read: 0.025,
        cache_write_short: 0.25,
        cache_write_long: 0.25,
        output: 1.5,
    },
    Price {
        model: "gemini-3.1-pro",
        input: 2.0,
        cache_read: 0.2,
        cache_write_short: 2.0,
        cache_write_long: 2.0,
        output: 12.0,
    },
    Price {
        model: "gemini-3-flash",
        input: 0.5,
        cache_read: 0.05,
        cache_write_short: 0.5,
        cache_write_long: 0.5,
        output: 3.0,
    },
    Price {
        model: "deepseek-flash",
        input: 0.3,
        cache_read: 0.006,
        cache_write_short: 0.3,
        cache_write_long: 0.3,
        output: 1.2,
    },
    Price {
        model: "deepseek-v4-pro",
        input: 1.32,
        cache_read: 0.044,
        cache_write_short: 1.32,
        cache_write_long: 1.32,
        output: 3.96,
    },
];

/// Price for `model`: the row with the longest prefix of the model id.
pub fn price_for(model: &str) -> Option<&'static Price> {
    PRICES
        .iter()
        .filter(|p| model.starts_with(p.model))
        .max_by_key(|p| p.model.len())
}

/// Estimated cost of `usage` at `price`, in US dollars.
pub fn cost(usage: &Usage, price: &Price) -> f64 {
    let per = |tokens: u64, usd_per_million: f64| tokens as f64 * usd_per_million / 1_000_000.0;
    per(usage.uncached_input, price.input)
        + per(usage.cache_read, price.cache_read)
        + per(usage.cache_write_short, price.cache_write_short)
        + per(usage.cache_write_long, price.cache_write_long)
        + per(usage.output, price.output)
}

/// Estimate for a set of models.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Estimate {
    /// Estimated US dollars for the models that have a price.
    pub usd: f64,
    /// Models with usage but no known price (their cost is not included).
    pub unpriced: Vec<String>,
}

/// Estimated cost of all models in `usage`.
pub fn estimate(usage: &ModelUsage) -> Estimate {
    let mut out = Estimate::default();
    for (model, u) in usage {
        match price_for(model) {
            Some(price) => out.usd += cost(u, price),
            None if !u.is_empty() => out.unpriced.push(model.clone()),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_prices_each_category_separately() {
        let price = Price {
            model: "m",
            input: 5.0,
            cache_read: 0.5,
            cache_write_short: 6.25,
            cache_write_long: 10.0,
            output: 25.0,
        };
        let usage = Usage {
            requests: 1,
            uncached_input: 1_000_000,
            cache_read: 2_000_000,
            cache_write_short: 0,
            cache_write_long: 100_000,
            output: 10_000,
        };
        let usd = cost(&usage, &price);
        assert!((usd - (5.0 + 1.0 + 1.0 + 0.25)).abs() < 1e-9, "{usd}");
    }

    #[test]
    fn since_and_add_are_inverse_and_never_negative() {
        let a = Usage {
            requests: 2,
            output: 10,
            ..Usage::default()
        };
        let mut b = a;
        b.add(&Usage {
            requests: 1,
            cache_read: 7,
            ..Usage::default()
        });
        assert_eq!(b.since(&a).cache_read, 7);
        assert_eq!(a.since(&b), Usage::default());
    }

    #[test]
    fn unknown_models_are_reported_not_guessed() {
        let mut usage = ModelUsage::new();
        usage.insert(
            "totally-new-model".into(),
            Usage {
                requests: 1,
                output: 5,
                ..Usage::default()
            },
        );
        let e = estimate(&usage);
        assert_eq!(e.usd, 0.0);
        assert_eq!(e.unpriced, vec!["totally-new-model".to_owned()]);
    }
}
