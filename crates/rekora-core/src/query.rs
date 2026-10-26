//! Query normalisation for keyword retrieval.
//!
//! Queries are reduced to a bounded list of content terms. Function words
//! (English and Spanish) are dropped because, matched with OR semantics, they
//! would make every memory look relevant and defeat `no_evidence` abstention.
//! Terms are later quoted for FTS5, so user text can never inject FTS syntax.

/// Maximum number of distinct terms kept from one query.
pub const MAX_TERMS: usize = 16;
/// Terms longer than this (in chars) are truncated; they are almost always noise.
pub const MAX_TERM_CHARS: usize = 64;

#[rustfmt::skip]
const STOPWORDS: &[&str] = &[
    // English
    "a", "about", "after", "all", "an", "and", "any", "are", "as", "at", "be", "been", "but", "by",
    "can", "could", "did", "do", "does", "for", "from", "had", "has", "have", "how", "i", "if",
    "in", "into", "is", "it", "its", "me", "my", "no", "not", "of", "on", "or", "our", "should",
    "so", "than", "that", "the", "their", "them", "then", "there", "these", "this", "to", "us",
    "was", "we", "were", "what", "when", "where", "which", "who", "why", "will", "with", "would",
    "you", "your",
    // Spanish
    "al", "como", "con", "cual", "cuando", "de", "del", "donde", "el", "en", "es", "esta", "este",
    "hay", "la", "las", "lo", "los", "mas", "mi", "para", "pero", "por", "que", "se", "si", "sin",
    "sobre", "su", "sus", "un", "una", "uno", "y", "ya",
];

/// Extracts lowercase content terms in first-seen order, without duplicates.
///
/// ```
/// let t = rekora_core::query::terms("How do we handle the DB concurrency? concurrency!");
/// assert_eq!(t, vec!["handle", "db", "concurrency"]);
/// ```
pub fn terms(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in query.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if raw.is_empty() {
            continue;
        }
        let term: String = raw.to_lowercase().chars().take(MAX_TERM_CHARS).collect();
        if term.chars().count() < 2 && term.is_ascii() {
            continue;
        }
        if STOPWORDS.contains(&term.as_str()) || out.contains(&term) {
            continue;
        }
        out.push(term);
        if out.len() == MAX_TERMS {
            break;
        }
    }
    out
}

/// Quotes a term as an FTS5 string literal so it is matched as a token and
/// never parsed as an operator (`NEAR`, `OR`, `*`, column filters...).
pub fn fts_literal(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_stopwords_and_punctuation() {
        assert_eq!(
            terms("¿Cómo manejamos la concurrencia?"),
            vec!["cómo", "manejamos", "concurrencia"]
        );
        assert!(terms("the of and y la").is_empty());
        assert!(terms("").is_empty());
        assert!(terms("!!! ??? ...").is_empty());
    }

    #[test]
    fn keeps_identifiers_and_cjk() {
        assert_eq!(
            terms("OrderService.update_line"),
            vec!["orderservice", "update_line"]
        );
        assert_eq!(terms("数据"), vec!["数据"]);
    }

    #[test]
    fn bounded_count_and_length() {
        let many: String = (0..100).map(|i| format!("term{i} ")).collect();
        assert_eq!(terms(&many).len(), MAX_TERMS);
        let long = "x".repeat(500);
        assert_eq!(terms(&long)[0].len(), MAX_TERM_CHARS);
    }

    #[test]
    fn fts_literal_neutralises_syntax() {
        assert_eq!(fts_literal("near"), "\"near\"");
        assert_eq!(fts_literal("a\"b"), "\"a\"\"b\"");
    }
}
