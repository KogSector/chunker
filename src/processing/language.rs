//! Programming language definitions and detection.
//!
//! Supports detection of 20+ programming languages via file extension
//! and content analysis (shebang detection).

use std::collections::HashMap;
use std::path::Path;

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    // Primary languages (full AST support)
    Python,
    JavaScript,
    TypeScript,
    TypeScriptReact,
    Go,
    Rust,
    Java,
    
    // Secondary languages (AST support)
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Kotlin,
    Swift,
    Scala,
    
    // Markup/Config (partial AST support)
    Html,
    Css,
    Markdown,
    Json,
    Yaml,
    Toml,
    Xml,
    
    // Shell/Script
    Shell,
    Sql,
    
    // Unknown/Plain text
    Unknown,
}

impl Language {
    /// Get the tree-sitter language name for this language.
    pub fn tree_sitter_name(&self) -> Option<&'static str> {
        match self {
            Language::Python => Some("python"),
            Language::JavaScript => Some("javascript"),
            Language::TypeScript => Some("typescript"),
            Language::TypeScriptReact => Some("tsx"),
            Language::Go => Some("go"),
            Language::Rust => Some("rust"),
            Language::Java => Some("java"),
            Language::C => Some("c"),
            Language::Cpp => Some("cpp"),
            Language::CSharp => Some("c_sharp"),
            Language::Ruby => Some("ruby"),
            Language::Php => Some("php"),
            Language::Kotlin => Some("kotlin"),
            Language::Swift => Some("swift"),
            Language::Scala => Some("scala"),
            Language::Html => Some("html"),
            Language::Css => Some("css"),
            Language::Shell => Some("bash"),
            _ => None,
        }
    }

    /// Check if AST parsing is supported for this language.
    pub fn supports_ast(&self) -> bool {
        self.tree_sitter_name().is_some()
    }

    /// Get the language from a string identifier.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "python" | "py" => Language::Python,
            "javascript" | "js" => Language::JavaScript,
            "typescript" | "ts" => Language::TypeScript,
            "tsx" => Language::TypeScriptReact,
            "go" | "golang" => Language::Go,
            "rust" | "rs" => Language::Rust,
            "java" => Language::Java,
            "c" => Language::C,
            "cpp" | "c++" | "cxx" => Language::Cpp,
            "csharp" | "c#" | "cs" => Language::CSharp,
            "ruby" | "rb" => Language::Ruby,
            "php" => Language::Php,
            "kotlin" | "kt" => Language::Kotlin,
            "swift" => Language::Swift,
            "scala" => Language::Scala,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "less" => Language::Css,
            "markdown" | "md" => Language::Markdown,
            "json" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "toml" => Language::Toml,
            "xml" => Language::Xml,
            "shell" | "bash" | "sh" | "zsh" => Language::Shell,
            "sql" => Language::Sql,
            _ => Language::Unknown,
        }
    }

    /// Get a string representation of the language.
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::TypeScriptReact => "tsx",
            Language::Go => "go",
            Language::Rust => "rust",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Kotlin => "kotlin",
            Language::Swift => "swift",
            Language::Scala => "scala",
            Language::Html => "html",
            Language::Css => "css",
            Language::Markdown => "markdown",
            Language::Json => "json",
            Language::Yaml => "yaml",
            Language::Toml => "toml",
            Language::Xml => "xml",
            Language::Shell => "shell",
            Language::Sql => "sql",
            Language::Unknown => "unknown",
        }
    }
}

/// Information about a detected language.
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    /// The detected language.
    pub language: Language,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
    /// Whether AST parsing is supported.
    pub ast_supported: bool,
}

impl LanguageInfo {
    pub fn new(language: Language, confidence: f32) -> Self {
        Self {
            ast_supported: language.supports_ast(),
            language,
            confidence,
        }
    }

    pub fn unknown() -> Self {
        Self {
            language: Language::Unknown,
            confidence: 0.0,
            ast_supported: false,
        }
    }
}

/// Language detector using extension and content analysis.
pub struct LanguageDetector {
    extension_map: HashMap<String, LanguageInfo>,
    filename_map: HashMap<String, Language>,
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageDetector {
    /// Create a new language detector with default mappings.
    pub fn new() -> Self {
        let mut extension_map = HashMap::new();
        let mut filename_map = HashMap::new();

        // Python
        for ext in &[".py", ".pyi", ".pyx", ".pyw"] {
            extension_map.insert(
                ext.to_string(),
                LanguageInfo::new(Language::Python, 1.0),
            );
        }

        // JavaScript
        for ext in &[".js", ".mjs", ".cjs", ".jsx"] {
            extension_map.insert(
                ext.to_string(),
                LanguageInfo::new(Language::JavaScript, 1.0),
            );
        }

        // TypeScript
        extension_map.insert(".ts".to_string(), LanguageInfo::new(Language::TypeScript, 1.0));
        extension_map.insert(".tsx".to_string(), LanguageInfo::new(Language::TypeScriptReact, 1.0));
        extension_map.insert(".d.ts".to_string(), LanguageInfo::new(Language::TypeScript, 1.0));

        // Go
        extension_map.insert(".go".to_string(), LanguageInfo::new(Language::Go, 1.0));

        // Rust
        extension_map.insert(".rs".to_string(), LanguageInfo::new(Language::Rust, 1.0));

        // Java
        extension_map.insert(".java".to_string(), LanguageInfo::new(Language::Java, 1.0));

        // C/C++
        extension_map.insert(".c".to_string(), LanguageInfo::new(Language::C, 1.0));
        extension_map.insert(".h".to_string(), LanguageInfo::new(Language::C, 0.8));
        for ext in &[".cpp", ".cc", ".cxx", ".hpp", ".hxx", ".hh"] {
            extension_map.insert(ext.to_string(), LanguageInfo::new(Language::Cpp, 1.0));
        }

        // C#
        extension_map.insert(".cs".to_string(), LanguageInfo::new(Language::CSharp, 1.0));

        // Ruby
        extension_map.insert(".rb".to_string(), LanguageInfo::new(Language::Ruby, 1.0));
        extension_map.insert(".rake".to_string(), LanguageInfo::new(Language::Ruby, 0.9));

        // PHP
        extension_map.insert(".php".to_string(), LanguageInfo::new(Language::Php, 1.0));

        // Kotlin
        extension_map.insert(".kt".to_string(), LanguageInfo::new(Language::Kotlin, 1.0));
        extension_map.insert(".kts".to_string(), LanguageInfo::new(Language::Kotlin, 1.0));

        // Swift
        extension_map.insert(".swift".to_string(), LanguageInfo::new(Language::Swift, 1.0));

        // Scala
        extension_map.insert(".scala".to_string(), LanguageInfo::new(Language::Scala, 1.0));
        extension_map.insert(".sc".to_string(), LanguageInfo::new(Language::Scala, 0.9));

        // Markup/Config
        extension_map.insert(".html".to_string(), LanguageInfo::new(Language::Html, 1.0));
        extension_map.insert(".htm".to_string(), LanguageInfo::new(Language::Html, 1.0));
        extension_map.insert(".css".to_string(), LanguageInfo::new(Language::Css, 1.0));
        extension_map.insert(".scss".to_string(), LanguageInfo::new(Language::Css, 0.9));
        extension_map.insert(".less".to_string(), LanguageInfo::new(Language::Css, 0.9));
        extension_map.insert(".md".to_string(), LanguageInfo::new(Language::Markdown, 1.0));
        extension_map.insert(".markdown".to_string(), LanguageInfo::new(Language::Markdown, 1.0));
        extension_map.insert(".json".to_string(), LanguageInfo::new(Language::Json, 1.0));
        extension_map.insert(".yaml".to_string(), LanguageInfo::new(Language::Yaml, 1.0));
        extension_map.insert(".yml".to_string(), LanguageInfo::new(Language::Yaml, 1.0));
        extension_map.insert(".toml".to_string(), LanguageInfo::new(Language::Toml, 1.0));
        extension_map.insert(".xml".to_string(), LanguageInfo::new(Language::Xml, 1.0));

        // Shell
        extension_map.insert(".sh".to_string(), LanguageInfo::new(Language::Shell, 1.0));
        extension_map.insert(".bash".to_string(), LanguageInfo::new(Language::Shell, 1.0));
        extension_map.insert(".zsh".to_string(), LanguageInfo::new(Language::Shell, 1.0));

        // SQL
        extension_map.insert(".sql".to_string(), LanguageInfo::new(Language::Sql, 1.0));

        // Filename mappings
        filename_map.insert("Dockerfile".to_string(), Language::Shell);
        filename_map.insert("Makefile".to_string(), Language::Shell);
        filename_map.insert("CMakeLists.txt".to_string(), Language::Shell);
        filename_map.insert("Jenkinsfile".to_string(), Language::Shell);
        filename_map.insert("Rakefile".to_string(), Language::Ruby);
        filename_map.insert("Gemfile".to_string(), Language::Ruby);
        filename_map.insert("package.json".to_string(), Language::Json);
        filename_map.insert("tsconfig.json".to_string(), Language::Json);
        filename_map.insert("pyproject.toml".to_string(), Language::Toml);
        filename_map.insert("Cargo.toml".to_string(), Language::Toml);
        filename_map.insert("go.mod".to_string(), Language::Go);
        filename_map.insert("go.sum".to_string(), Language::Go);

        Self {
            extension_map,
            filename_map,
        }
    }

    /// Detect language from file path and optional content.
    pub fn detect(&self, path: &str, content: Option<&str>) -> LanguageInfo {
        let path = Path::new(path);
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Check filename first
        if let Some(&lang) = self.filename_map.get(filename) {
            return LanguageInfo::new(lang, 0.95);
        }

        // Check extension
        let ext_key = format!(".{}", extension.to_lowercase());
        if let Some(info) = self.extension_map.get(&ext_key) {
            return info.clone();
        }

        // Try shebang detection from content
        if let Some(content) = content {
            if content.starts_with("#!") {
                let first_line = content.lines().next().unwrap_or("");
                return self.detect_from_shebang(first_line);
            }
        }

        LanguageInfo::unknown()
    }

    /// Detect language from shebang line.
    fn detect_from_shebang(&self, shebang: &str) -> LanguageInfo {
        let lower = shebang.to_lowercase();

        if lower.contains("python") {
            LanguageInfo::new(Language::Python, 0.95)
        } else if lower.contains("node") || lower.contains("deno") {
            LanguageInfo::new(Language::JavaScript, 0.95)
        } else if lower.contains("ruby") {
            LanguageInfo::new(Language::Ruby, 0.95)
        } else if lower.contains("php") {
            LanguageInfo::new(Language::Php, 0.95)
        } else if lower.contains("bash") || lower.contains("/sh") {
            LanguageInfo::new(Language::Shell, 0.95)
        } else {
            LanguageInfo::new(Language::Shell, 0.5) // Default shebang to shell
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_detection() {
        let detector = LanguageDetector::new();

        assert_eq!(
            detector.detect("main.py", None).language,
            Language::Python
        );
        assert_eq!(
            detector.detect("app.js", None).language,
            Language::JavaScript
        );
        assert_eq!(
            detector.detect("lib.rs", None).language,
            Language::Rust
        );
        assert_eq!(
            detector.detect("main.go", None).language,
            Language::Go
        );
    }

    #[test]
    fn test_filename_detection() {
        let detector = LanguageDetector::new();

        assert_eq!(
            detector.detect("Dockerfile", None).language,
            Language::Shell
        );
        assert_eq!(
            detector.detect("Cargo.toml", None).language,
            Language::Toml
        );
        assert_eq!(
            detector.detect("package.json", None).language,
            Language::Json
        );
    }

    #[test]
    fn test_shebang_detection() {
        let detector = LanguageDetector::new();

        assert_eq!(
            detector.detect("script", Some("#!/usr/bin/env python3\nprint('hello')")).language,
            Language::Python
        );
        assert_eq!(
            detector.detect("script", Some("#!/bin/bash\necho hello")).language,
            Language::Shell
        );
    }

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("python"), Language::Python);
        assert_eq!(Language::from_str("RUST"), Language::Rust);
        assert_eq!(Language::from_str("c++"), Language::Cpp);
    }
}
