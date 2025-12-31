//! Dependency parser for analyzing imports and dependencies.
//!
//! Parses import statements across languages to extract dependency information
//! for context enrichment and relationship building.

use std::collections::HashSet;

use crate::ast_engine::entity_extractor::Import;

/// Type of dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyType {
    /// Standard library import.
    StandardLib,
    /// Third-party/external package.
    External,
    /// Internal/local module.
    Internal,
    /// Relative import (same package).
    Relative,
    /// Unknown origin.
    Unknown,
}

/// A parsed dependency.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Module or package name.
    pub module: String,
    /// Specific items imported (if any).
    pub items: Vec<String>,
    /// Type of dependency.
    pub dependency_type: DependencyType,
    /// Source line number.
    pub line: usize,
}

impl From<&Import> for Dependency {
    fn from(import: &Import) -> Self {
        let dependency_type = if import.is_relative {
            DependencyType::Relative
        } else {
            DependencyType::Unknown
        };

        Self {
            module: import.module.clone(),
            items: import.items.clone(),
            dependency_type,
            line: import.line,
        }
    }
}

/// Parser for analyzing dependencies.
pub struct DependencyParser {
    /// Known standard library modules by language.
    standard_libs: std::collections::HashMap<String, HashSet<String>>,
    /// Known internal module prefixes.
    internal_prefixes: Vec<String>,
}

impl Default for DependencyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyParser {
    /// Create a new dependency parser with default settings.
    pub fn new() -> Self {
        let mut standard_libs = std::collections::HashMap::new();

        // Python standard library (common modules)
        let python_std: HashSet<String> = [
            "os", "sys", "re", "json", "math", "datetime", "collections",
            "itertools", "functools", "typing", "pathlib", "dataclasses",
            "abc", "asyncio", "concurrent", "contextlib", "copy", "csv",
            "enum", "glob", "hashlib", "http", "io", "logging", "pickle",
            "random", "shutil", "socket", "sqlite3", "string", "subprocess",
            "tempfile", "threading", "time", "traceback", "unittest", "urllib",
            "uuid", "warnings", "xml", "zipfile",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        standard_libs.insert("python".to_string(), python_std);

        // Go standard library
        let go_std: HashSet<String> = [
            "fmt", "io", "os", "net", "http", "encoding", "json", "xml",
            "strings", "bytes", "time", "context", "sync", "errors", "log",
            "path", "filepath", "regexp", "sort", "strconv", "math", "crypto",
            "database", "sql", "testing", "bufio", "runtime", "reflect",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        standard_libs.insert("go".to_string(), go_std);

        // Rust standard library (crates)
        let rust_std: HashSet<String> = [
            "std", "core", "alloc",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        standard_libs.insert("rust".to_string(), rust_std);

        // JavaScript/TypeScript built-ins (Node.js)
        let js_std: HashSet<String> = [
            "fs", "path", "http", "https", "url", "util", "os", "events",
            "stream", "crypto", "buffer", "child_process", "cluster",
            "dns", "net", "readline", "repl", "tls", "zlib", "assert",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        standard_libs.insert("javascript".to_string(), js_std.clone());
        standard_libs.insert("typescript".to_string(), js_std);

        Self {
            standard_libs,
            internal_prefixes: Vec::new(),
        }
    }

    /// Set internal module prefixes.
    pub fn with_internal_prefixes(mut self, prefixes: Vec<String>) -> Self {
        self.internal_prefixes = prefixes;
        self
    }

    /// Parse imports into dependencies with type classification.
    pub fn parse_imports(&self, imports: &[Import], language: &str) -> Vec<Dependency> {
        imports
            .iter()
            .map(|import| {
                let mut dep = Dependency::from(import);
                dep.dependency_type = self.classify_dependency(&dep.module, language, import.is_relative);
                dep
            })
            .collect()
    }

    /// Classify a dependency by type.
    fn classify_dependency(&self, module: &str, language: &str, is_relative: bool) -> DependencyType {
        if is_relative {
            return DependencyType::Relative;
        }

        // Check internal prefixes
        for prefix in &self.internal_prefixes {
            if module.starts_with(prefix) {
                return DependencyType::Internal;
            }
        }

        // Check standard library
        if let Some(std_mods) = self.standard_libs.get(language) {
            let root_module = module.split(&['.', '/', ':'][..]).next().unwrap_or(module);
            if std_mods.contains(root_module) {
                return DependencyType::StandardLib;
            }
        }

        // Heuristics for internal vs external
        if module.starts_with('.') || module.starts_with("./") || module.starts_with("../") {
            return DependencyType::Relative;
        }

        // If it looks like a scoped package (@org/pkg) it's likely external
        if module.starts_with('@') {
            return DependencyType::External;
        }

        // Default to external for unrecognized modules
        DependencyType::External
    }

    /// Get unique external dependencies from a list of imports.
    pub fn get_external_deps(&self, imports: &[Import], language: &str) -> Vec<String> {
        let deps = self.parse_imports(imports, language);
        deps.iter()
            .filter(|d| d.dependency_type == DependencyType::External)
            .map(|d| d.module.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get unique internal dependencies from a list of imports.
    pub fn get_internal_deps(&self, imports: &[Import], language: &str) -> Vec<String> {
        let deps = self.parse_imports(imports, language);
        deps.iter()
            .filter(|d| {
                matches!(
                    d.dependency_type,
                    DependencyType::Internal | DependencyType::Relative
                )
            })
            .map(|d| d.module.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get all unique dependencies grouped by type.
    pub fn get_grouped_deps(
        &self,
        imports: &[Import],
        language: &str,
    ) -> std::collections::HashMap<DependencyType, Vec<String>> {
        let deps = self.parse_imports(imports, language);
        let mut grouped: std::collections::HashMap<DependencyType, Vec<String>> =
            std::collections::HashMap::new();

        for dep in deps {
            grouped
                .entry(dep.dependency_type)
                .or_default()
                .push(dep.module);
        }

        // Deduplicate
        for modules in grouped.values_mut() {
            let unique: HashSet<_> = modules.drain(..).collect();
            modules.extend(unique);
            modules.sort();
        }

        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_import(module: &str, is_relative: bool) -> Import {
        Import {
            module: module.to_string(),
            items: Vec::new(),
            alias: None,
            line: 1,
            is_relative,
        }
    }

    #[test]
    fn test_python_stdlib_detection() {
        let parser = DependencyParser::new();
        let imports = vec![
            create_import("os", false),
            create_import("json", false),
            create_import("requests", false),
            create_import(".utils", true),
        ];

        let deps = parser.parse_imports(&imports, "python");

        assert_eq!(deps[0].dependency_type, DependencyType::StandardLib); // os
        assert_eq!(deps[1].dependency_type, DependencyType::StandardLib); // json
        assert_eq!(deps[2].dependency_type, DependencyType::External); // requests
        assert_eq!(deps[3].dependency_type, DependencyType::Relative); // .utils
    }

    #[test]
    fn test_internal_prefix() {
        let parser = DependencyParser::new()
            .with_internal_prefixes(vec!["myapp".to_string(), "internal".to_string()]);

        let imports = vec![
            create_import("myapp.services.user", false),
            create_import("internal.utils", false),
            create_import("requests", false),
        ];

        let deps = parser.parse_imports(&imports, "python");

        assert_eq!(deps[0].dependency_type, DependencyType::Internal);
        assert_eq!(deps[1].dependency_type, DependencyType::Internal);
        assert_eq!(deps[2].dependency_type, DependencyType::External);
    }

    #[test]
    fn test_get_external_deps() {
        let parser = DependencyParser::new();
        let imports = vec![
            create_import("os", false),
            create_import("requests", false),
            create_import("flask", false),
            create_import("sys", false),
        ];

        let external = parser.get_external_deps(&imports, "python");

        assert!(external.contains(&"requests".to_string()));
        assert!(external.contains(&"flask".to_string()));
        assert!(!external.contains(&"os".to_string()));
        assert!(!external.contains(&"sys".to_string()));
    }

    #[test]
    fn test_scoped_package_detection() {
        let parser = DependencyParser::new();
        let imports = vec![
            create_import("@babel/core", false),
            create_import("react", false),
        ];

        let deps = parser.parse_imports(&imports, "javascript");

        assert_eq!(deps[0].dependency_type, DependencyType::External);
        assert_eq!(deps[1].dependency_type, DependencyType::External);
    }
}
