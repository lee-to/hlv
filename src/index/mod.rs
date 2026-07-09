pub mod builder;
pub mod languages;

use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{Context, Result};
use languages::SupportedLanguage;
use tree_sitter::{Node, Parser};

use crate::model::index::Symbol;

pub fn extract_symbols_from_source(path: &Path, source: &str) -> Result<Vec<Symbol>> {
    let Some(language) = SupportedLanguage::from_path(path) else {
        tracing::debug!(path = %path.display(), "Skipping unsupported source file for index extraction");
        return Ok(Vec::new());
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language.parser_language())
        .context("failed to load tree-sitter language")?;

    tracing::debug!(path = %path.display(), language = language.label(), "Parsing source for signature index");
    let tree = parser
        .parse(source, None)
        .context("tree-sitter parser returned no tree")?;

    let mut symbols = Vec::new();
    collect_symbols(
        tree.root_node(),
        path,
        source,
        language,
        None,
        namespace_for(language, source),
        &mut symbols,
    );
    tracing::debug!(
        path = %path.display(),
        language = language.label(),
        symbol_count = symbols.len(),
        "Signature extraction complete"
    );
    Ok(symbols)
}

fn collect_symbols(
    node: Node<'_>,
    path: &Path,
    source: &str,
    language: SupportedLanguage,
    scope: Option<String>,
    namespace: Option<String>,
    symbols: &mut Vec<Symbol>,
) {
    let next_scope = if language.is_definition_kind(node.kind()) {
        symbol_from_node(
            node,
            path,
            source,
            language,
            scope.clone(),
            namespace.clone(),
        )
        .map(|symbol| {
            let next = if matches!(symbol.kind.as_str(), "class" | "struct" | "trait" | "impl") {
                Some(symbol.name.clone())
            } else {
                scope.clone()
            };
            symbols.push(symbol);
            next
        })
        .unwrap_or(scope)
    } else {
        scope
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            collect_symbols(
                child,
                path,
                source,
                language,
                next_scope.clone(),
                namespace.clone(),
                symbols,
            );
        }
    }
}

fn symbol_from_node(
    node: Node<'_>,
    path: &Path,
    source: &str,
    language: SupportedLanguage,
    scope: Option<String>,
    namespace: Option<String>,
) -> Option<Symbol> {
    let name = node_name(node, source)?;
    let signature = node_signature(node, source);
    let file = path.to_string_lossy().replace('\\', "/");
    let kind = language.symbol_kind(node.kind()).to_string();
    let id = stable_symbol_id(language.label(), &file, scope.as_deref(), &name, &kind);
    let line = node.start_position().row as u32 + 1;
    let visibility = visibility_for(&signature, language).to_string();
    let source_fingerprint = format!("fnv64:{:016x}", fingerprint(&signature));

    Some(Symbol {
        id,
        name,
        file,
        line,
        signature,
        visibility,
        kind,
        language: language.label().to_string(),
        namespace,
        scope,
        source_fingerprint,
    })
}

fn node_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(name) = node.child_by_field_name("name") {
        return node_text(name, source);
    }

    let mut cursor = node.walk();
    let name = node
        .children(&mut cursor)
        .find(|child| {
            matches!(
                child.kind(),
                "identifier" | "type_identifier" | "property_identifier"
            )
        })
        .and_then(|child| node_text(child, source));
    name
}

fn node_signature(node: Node<'_>, source: &str) -> String {
    node_text(node, source)
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn node_text(node: Node<'_>, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn stable_symbol_id(
    language: &str,
    file: &str,
    scope: Option<&str>,
    name: &str,
    kind: &str,
) -> String {
    match scope {
        Some(scope) => format!("{language}:{file}:{scope}::{name}:{kind}"),
        None => format!("{language}:{file}:{name}:{kind}"),
    }
}

fn visibility_for(signature: &str, _language: SupportedLanguage) -> &'static str {
    let trimmed = signature.trim_start();
    if trimmed.starts_with("private ") || trimmed.starts_with("private function ") {
        "private"
    } else if trimmed.starts_with("protected ") || trimmed.starts_with("protected function ") {
        "protected"
    } else {
        "public"
    }
}

fn namespace_for(language: SupportedLanguage, source: &str) -> Option<String> {
    if language != SupportedLanguage::Php {
        return None;
    }
    source.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("namespace ")
            .map(|rest| rest.trim_end_matches(';').trim().to_string())
            .filter(|namespace| !namespace.is_empty())
    })
}

fn fingerprint(text: &str) -> u64 {
    let mut hasher = StableHasher::default();
    text.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        if self.0 == 0 {
            self.0 = FNV_OFFSET;
        }
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names_for(path: &str, source: &str) -> Vec<String> {
        extract_symbols_from_source(Path::new(path), source)
            .unwrap()
            .into_iter()
            .map(|symbol| symbol.name)
            .collect()
    }

    #[test]
    fn extracts_rust_definitions() {
        let names = names_for(
            "src/lib.rs",
            "pub struct User;\npub fn greeting(name: &str) -> String { name.to_string() }\n",
        );
        assert!(names.contains(&"User".to_string()));
        assert!(names.contains(&"greeting".to_string()));
    }

    #[test]
    fn extracts_go_definitions() {
        let names = names_for(
            "internal/greeting/greeting.go",
            "package greeting\nfunc Message(name string) string { return name }\n",
        );
        assert!(names.contains(&"Message".to_string()));
    }

    #[test]
    fn extracts_python_definitions() {
        let names = names_for(
            "src/app.py",
            "class Service:\n    def greet(self):\n        pass\n",
        );
        assert!(names.contains(&"Service".to_string()));
        assert!(names.contains(&"greet".to_string()));
    }

    #[test]
    fn extracts_typescript_definitions() {
        let names = names_for(
            "src/app.ts",
            "export class Service { greet(name: string): string { return name; } }\n",
        );
        assert!(names.contains(&"Service".to_string()));
        assert!(names.contains(&"greet".to_string()));
    }

    #[test]
    fn extracts_javascript_definitions() {
        let names = names_for(
            "src/app.js",
            "class Service { greet(name) { return name; } }\nfunction run() {}\n",
        );
        assert!(names.contains(&"Service".to_string()));
        assert!(names.contains(&"greet".to_string()));
        assert!(names.contains(&"run".to_string()));
    }

    #[test]
    fn extracts_php_definitions() {
        let symbols = extract_symbols_from_source(
            Path::new("app/Services/GreetingService.php"),
            "<?php\nnamespace App\\Services;\nfinal class GreetingService { public function greet(string $name): string { return $name; } }\n",
        )
        .unwrap();
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "GreetingService"));
        assert!(symbols.iter().any(|symbol| symbol.name == "greet"));
        assert!(symbols
            .iter()
            .all(|symbol| symbol.namespace.as_deref() == Some("App\\Services")));
    }
}
