use std::path::Path;

use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Go,
    JavaScript,
    Php,
    Python,
    Rust,
    TypeScript,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("go") => Some(Self::Go),
            Some("js" | "mjs" | "cjs" | "jsx") => Some(Self::JavaScript),
            Some("php") => Some(Self::Php),
            Some("py") => Some(Self::Python),
            Some("rs") => Some(Self::Rust),
            Some("ts" | "tsx") => Some(Self::TypeScript),
            _ => None,
        }
    }

    pub fn parser_language(self) -> Language {
        match self {
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::Php => "php",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
        }
    }

    pub fn is_definition_kind(self, kind: &str) -> bool {
        match self {
            Self::Go => matches!(
                kind,
                "function_declaration" | "method_declaration" | "type_declaration"
            ),
            Self::JavaScript | Self::TypeScript => matches!(
                kind,
                "function_declaration"
                    | "method_definition"
                    | "class_declaration"
                    | "interface_declaration"
                    | "abstract_class_declaration"
            ),
            Self::Php => matches!(
                kind,
                "function_definition"
                    | "method_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "trait_declaration"
            ),
            Self::Python => matches!(kind, "function_definition" | "class_definition"),
            Self::Rust => matches!(
                kind,
                "function_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item"
            ),
        }
    }

    pub fn symbol_kind(self, node_kind: &str) -> &'static str {
        match node_kind {
            "class_declaration" | "class_definition" => "class",
            "abstract_class_declaration" => "class",
            "interface_declaration" => "interface",
            "trait_declaration" | "trait_item" => "trait",
            "struct_item" => "struct",
            "enum_item" => "enum",
            "method_declaration" | "method_definition" => "method",
            "impl_item" => "impl",
            "type_declaration" => "type",
            _ => "function",
        }
    }
}
