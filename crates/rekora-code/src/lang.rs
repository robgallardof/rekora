//! Language detection and syntax-only symbol extraction with tree-sitter.
//!
//! What this extracts: *declarations* (functions, methods, types, classes,
//! interfaces, enums, modules/namespaces) with their line. What it does not:
//! references, call graphs, type resolution, macro expansion, dynamic
//! attributes. Callers report that limit explicitly (master prompt §7) instead
//! of implying semantic understanding.

use rekora_store::Symbol;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

/// Languages with a real parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// `.rs`
    Rust,
    /// `.py`, `.pyi`
    Python,
    /// `.js`, `.jsx`, `.mjs`, `.cjs`
    JavaScript,
    /// `.ts`, `.mts`, `.cts`
    TypeScript,
    /// `.tsx`
    Tsx,
    /// `.cs`
    CSharp,
}

/// Classification of a file for indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detected {
    /// Parsed for symbols.
    Parsed(Lang),
    /// Recognised source, kept as file-level evidence only (e.g. `.vue`).
    FileOnly(&'static str),
    /// Not indexed.
    Ignored,
}

impl Lang {
    /// Stable language id stored in the index.
    pub fn id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::CSharp => "csharp",
        }
    }

    fn language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        }
    }

    /// Declaration patterns. Each captures the declaration as `@def` and its
    /// identifier as `@name`; the kind comes from the `@def` node type.
    fn query(self) -> &'static str {
        match self {
            Self::Rust => {
                r#"
                (function_item name: (_) @name) @def
                (function_signature_item name: (_) @name) @def
                (struct_item name: (_) @name) @def
                (enum_item name: (_) @name) @def
                (union_item name: (_) @name) @def
                (trait_item name: (_) @name) @def
                (type_item name: (_) @name) @def
                (mod_item name: (_) @name) @def
                (const_item name: (_) @name) @def
                (static_item name: (_) @name) @def
                (macro_definition name: (_) @name) @def
                "#
            }
            Self::Python => {
                r#"
                (function_definition name: (_) @name) @def
                (class_definition name: (_) @name) @def
                "#
            }
            Self::JavaScript => {
                r#"
                (function_declaration name: (_) @name) @def
                (generator_function_declaration name: (_) @name) @def
                (class_declaration name: (_) @name) @def
                (method_definition name: (_) @name) @def
                (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @def
                "#
            }
            Self::TypeScript | Self::Tsx => {
                r#"
                (function_declaration name: (_) @name) @def
                (generator_function_declaration name: (_) @name) @def
                (class_declaration name: (_) @name) @def
                (abstract_class_declaration name: (_) @name) @def
                (method_definition name: (_) @name) @def
                (interface_declaration name: (_) @name) @def
                (type_alias_declaration name: (_) @name) @def
                (enum_declaration name: (_) @name) @def
                (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @def
                "#
            }
            Self::CSharp => {
                r#"
                (class_declaration name: (_) @name) @def
                (struct_declaration name: (_) @name) @def
                (record_declaration name: (_) @name) @def
                (interface_declaration name: (_) @name) @def
                (enum_declaration name: (_) @name) @def
                (method_declaration name: (_) @name) @def
                (constructor_declaration name: (_) @name) @def
                (property_declaration name: (_) @name) @def
                (namespace_declaration name: (_) @name) @def
                (file_scoped_namespace_declaration name: (_) @name) @def
                "#
            }
        }
    }
}

/// Classifies a path by extension.
pub fn detect(path: &str) -> Detected {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => Detected::Parsed(Lang::Rust),
        "py" | "pyi" => Detected::Parsed(Lang::Python),
        "js" | "jsx" | "mjs" | "cjs" => Detected::Parsed(Lang::JavaScript),
        "ts" | "mts" | "cts" => Detected::Parsed(Lang::TypeScript),
        "tsx" => Detected::Parsed(Lang::Tsx),
        "cs" => Detected::Parsed(Lang::CSharp),
        "vue" => Detected::FileOnly("vue"),
        "svelte" => Detected::FileOnly("svelte"),
        "go" => Detected::FileOnly("go"),
        "java" => Detected::FileOnly("java"),
        "kt" => Detected::FileOnly("kotlin"),
        "rb" => Detected::FileOnly("ruby"),
        "php" => Detected::FileOnly("php"),
        "c" | "h" => Detected::FileOnly("c"),
        "cpp" | "cc" | "hpp" => Detected::FileOnly("cpp"),
        "md" => Detected::FileOnly("markdown"),
        "toml" => Detected::FileOnly("toml"),
        "json" => Detected::FileOnly("json"),
        "yaml" | "yml" => Detected::FileOnly("yaml"),
        "csproj" | "sln" => Detected::FileOnly("msbuild"),
        "sql" => Detected::FileOnly("sql"),
        _ => Detected::Ignored,
    }
}

/// Reusable parser + compiled queries. Construct once per scan.
pub struct Extractor {
    parsers: Vec<(Lang, Parser, Query)>,
}

impl Extractor {
    /// Builds parsers for every supported language. Fails only if a bundled
    /// grammar or query is broken, which the unit tests catch.
    pub fn new() -> Result<Self, String> {
        let mut parsers = Vec::new();
        for lang in [
            Lang::Rust,
            Lang::Python,
            Lang::JavaScript,
            Lang::TypeScript,
            Lang::Tsx,
            Lang::CSharp,
        ] {
            let language = lang.language();
            let mut parser = Parser::new();
            parser
                .set_language(&language)
                .map_err(|e| format!("{}: {e}", lang.id()))?;
            let query = Query::new(&language, lang.query())
                .map_err(|e| format!("{} query: {e}", lang.id()))?;
            parsers.push((lang, parser, query));
        }
        Ok(Self { parsers })
    }

    /// Extracts declarations from `source`, sorted by line then name.
    /// Syntax errors are tolerated: tree-sitter recovers and the declarations
    /// it can still see are returned.
    pub fn symbols(&mut self, lang: Lang, source: &[u8]) -> Vec<Symbol> {
        let Some((_, parser, query)) = self.parsers.iter_mut().find(|(l, _, _)| *l == lang) else {
            return Vec::new();
        };
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let name_index = query.capture_index_for_name("name");
        let def_index = query.capture_index_for_name("def");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source);
        let mut out = Vec::new();
        while let Some(m) = matches.next() {
            let name = m.captures().iter().find(|c| Some(c.index) == name_index);
            let def = m.captures().iter().find(|c| Some(c.index) == def_index);
            let (Some(name), Some(def)) = (name, def) else {
                continue;
            };
            let Ok(text) = name.node.utf8_text(source) else {
                continue;
            };
            out.push(Symbol {
                name: text.to_owned(),
                kind: kind_label(def.node.kind()).to_owned(),
                line: name.node.start_position().row as u32 + 1,
            });
        }
        out.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.name.cmp(&b.name)));
        out.dedup();
        out
    }
}

/// Maps grammar node types to a small shared vocabulary.
fn kind_label(node_kind: &str) -> &'static str {
    match node_kind {
        "function_item"
        | "function_signature_item"
        | "function_definition"
        | "function_declaration"
        | "generator_function_declaration" => "function",
        "variable_declarator" => "function",
        "method_definition" | "method_declaration" | "constructor_declaration" => "method",
        "struct_item" | "struct_declaration" => "struct",
        "enum_item" | "enum_declaration" => "enum",
        "union_item" => "union",
        "trait_item" => "trait",
        "type_item" | "type_alias_declaration" => "type",
        "mod_item" => "module",
        "namespace_declaration" | "file_scoped_namespace_declaration" => "namespace",
        "const_item" | "static_item" => "constant",
        "macro_definition" => "macro",
        "class_definition" | "class_declaration" | "abstract_class_declaration" => "class",
        "record_declaration" => "record",
        "interface_declaration" => "interface",
        "property_declaration" => "property",
        _ => "declaration",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(lang: Lang, src: &str) -> Vec<(String, String, u32)> {
        let mut ex = Extractor::new().unwrap();
        ex.symbols(lang, src.as_bytes())
            .into_iter()
            .map(|s| (s.name, s.kind, s.line))
            .collect()
    }

    fn has(v: &[(String, String, u32)], name: &str, kind: &str, line: u32) -> bool {
        v.iter()
            .any(|(n, k, l)| n == name && k == kind && *l == line)
    }

    #[test]
    fn rust_declarations() {
        let v = names(
            Lang::Rust,
            "pub trait Storage {\n    fn put(&self);\n}\npub struct Memory { id: u64 }\nimpl Memory {\n    pub fn new() -> Self { todo!() }\n}\nmod inner {}\nmacro_rules! m { () => {} }\n",
        );
        assert!(has(&v, "Storage", "trait", 1));
        assert!(has(&v, "put", "function", 2));
        assert!(has(&v, "Memory", "struct", 4));
        assert!(has(&v, "new", "function", 6));
        assert!(has(&v, "inner", "module", 8));
        assert!(has(&v, "m", "macro", 9));
    }

    #[test]
    fn python_declarations_and_dynamic_code_ignored() {
        let v = names(
            Lang::Python,
            "class OrderRepo:\n    def save(self):\n        pass\n\ndef load():\n    pass\n\nsetattr(OrderRepo, 'dyn', lambda s: 1)\n",
        );
        assert!(has(&v, "OrderRepo", "class", 1));
        assert!(has(&v, "save", "function", 2));
        assert!(has(&v, "load", "function", 5));
        assert!(
            !v.iter().any(|(n, ..)| n == "dyn"),
            "dynamic attribute claimed as symbol"
        );
    }

    #[test]
    fn typescript_and_tsx_declarations() {
        let v = names(
            Lang::TypeScript,
            "export interface Order { id: string }\nexport type Id = string;\nexport enum State { Open }\nexport class OrderList {\n  render() {}\n}\nexport const useOrders = () => [];\nfunction helper() {}\n",
        );
        for (n, k, l) in [
            ("Order", "interface", 1),
            ("Id", "type", 2),
            ("State", "enum", 3),
            ("OrderList", "class", 4),
            ("render", "method", 5),
            ("useOrders", "function", 7),
            ("helper", "function", 8),
        ] {
            assert!(has(&v, n, k, l), "{n} {k} {l} missing in {v:?}");
        }
        let tsx = names(
            Lang::Tsx,
            "export function OrderEditor() { return <div/>; }\n",
        );
        assert!(has(&tsx, "OrderEditor", "function", 1));
    }

    #[test]
    fn javascript_declarations() {
        let v = names(
            Lang::JavaScript,
            "class A { m() {} }\nconst f = function () {};\nfunction* g() {}\n",
        );
        assert!(has(&v, "A", "class", 1));
        assert!(has(&v, "m", "method", 1));
        assert!(has(&v, "f", "function", 2));
        assert!(has(&v, "g", "function", 3));
    }

    #[test]
    fn csharp_declarations() {
        let v = names(
            Lang::CSharp,
            "namespace Shop.Orders;\npublic interface IOrderRepository { Task SaveAsync(Order o); }\npublic sealed class OrderService {\n  public OrderService() {}\n  public async Task UpdateAsync() {}\n  public byte[] RowVersion { get; set; }\n}\npublic record OrderLine(int Qty);\n",
        );
        for (n, k, l) in [
            ("Shop.Orders", "namespace", 1),
            ("IOrderRepository", "interface", 2),
            ("SaveAsync", "method", 2),
            ("OrderService", "class", 3),
            ("OrderService", "method", 4),
            ("UpdateAsync", "method", 5),
            ("RowVersion", "property", 6),
            ("OrderLine", "record", 8),
        ] {
            assert!(has(&v, n, k, l), "{n} {k} {l} missing in {v:?}");
        }
    }

    #[test]
    fn broken_syntax_still_yields_visible_declarations() {
        let v = names(Lang::Rust, "fn ok() {}\nfn broken( {\n");
        assert!(has(&v, "ok", "function", 1));
    }

    #[test]
    fn detection() {
        assert_eq!(detect("a/B.CS"), Detected::Parsed(Lang::CSharp));
        assert_eq!(detect("web/App.vue"), Detected::FileOnly("vue"));
        assert_eq!(detect("logo.png"), Detected::Ignored);
        assert_eq!(detect("Makefile"), Detected::Ignored);
    }
}
