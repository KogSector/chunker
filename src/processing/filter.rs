//! File filtering configuration and rules.
//!
//! Provides configurable filtering for directories, extensions, and patterns
//! to exclude from processing (e.g., node_modules, binaries, vendor dirs).

use std::collections::HashSet;
use std::path::Path;

/// Configuration for file filtering.
#[derive(Debug, Clone)]
pub struct FilterConfig {
    /// Directories to exclude from processing.
    pub excluded_directories: HashSet<String>,
    /// File extensions to exclude (e.g., ".pyc", ".exe").
    pub excluded_extensions: HashSet<String>,
    /// Maximum file size in bytes (default: 1MB).
    pub max_file_size: usize,
    /// Minimum file size in bytes (default: 1).
    pub min_file_size: usize,
    /// Whether to include hidden files (starting with .).
    pub include_hidden: bool,
    /// Whether to include test files.
    pub include_tests: bool,
    /// Patterns for generated files to exclude.
    pub generated_patterns: Vec<String>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            excluded_directories: default_excluded_directories(),
            excluded_extensions: default_excluded_extensions(),
            max_file_size: 1024 * 1024, // 1MB
            min_file_size: 1,
            include_hidden: false,
            include_tests: true,
            generated_patterns: default_generated_patterns(),
        }
    }
}

fn default_excluded_directories() -> HashSet<String> {
    [
        // Version control
        ".git",
        ".svn",
        ".hg",
        // Dependencies
        "node_modules",
        "vendor",
        ".venv",
        "venv",
        "env",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".tox",
        "target",
        "build",
        "dist",
        "out",
        ".next",
        ".nuxt",
        ".output",
        // IDE
        ".idea",
        ".vscode",
        ".vs",
        // Package managers
        ".npm",
        ".yarn",
        ".pnpm",
        // Misc
        "coverage",
        ".coverage",
        "htmlcov",
        "eggs",
        ".eggs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_excluded_extensions() -> HashSet<String> {
    [
        // Compiled
        ".pyc", ".pyo", ".pyd", ".so", ".dll", ".dylib",
        ".class", ".o", ".obj", ".exe", ".bin", ".a",
        // Archives
        ".zip", ".tar", ".gz", ".bz2", ".xz", ".rar", ".7z",
        // Images
        ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".ico", ".svg", ".webp",
        ".tiff", ".psd", ".ai",
        // Documents
        ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        // Media
        ".mp3", ".mp4", ".avi", ".mov", ".wav", ".flac", ".mkv",
        ".wmv", ".ogg", ".webm",
        // Data
        ".db", ".sqlite", ".sqlite3", ".mdb",
        // Lock files
        ".lock",
        // Fonts
        ".woff", ".woff2", ".ttf", ".eot", ".otf",
        // Misc binary
        ".DS_Store", ".ico",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_generated_patterns() -> Vec<String> {
    [
        r".*\.generated\.",
        r".*\.g\.",
        r".*\.min\.",
        r".*bundle\..*",
        r".*-lock\.",
        r".*\.map$",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// File filter for determining which files to process.
pub struct FileFilter {
    config: FilterConfig,
    generated_regexes: Vec<regex::Regex>,
}

impl FileFilter {
    /// Create a new file filter with the given configuration.
    pub fn new(config: FilterConfig) -> Self {
        let generated_regexes = config
            .generated_patterns
            .iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        Self {
            config,
            generated_regexes,
        }
    }

    /// Create a filter with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(FilterConfig::default())
    }

    /// Check if a file should be processed.
    ///
    /// Returns `Ok(())` if the file should be processed, or `Err(reason)` if it should be skipped.
    pub fn should_process(&self, path: &str, size: usize) -> Result<(), String> {
        let path_obj = Path::new(path);

        // Check size limits
        if size < self.config.min_file_size {
            return Err("File is empty".to_string());
        }

        if size > self.config.max_file_size {
            return Err(format!(
                "File too large: {} bytes (max: {})",
                size, self.config.max_file_size
            ));
        }

        // Check excluded directories
        for component in path_obj.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if self.config.excluded_directories.contains(name) {
                    return Err(format!("In excluded directory: {}", name));
                }

                // Check hidden directories
                if !self.config.include_hidden && name.starts_with('.') {
                    // Allow some common config directories
                    if !matches!(name, ".github" | ".vscode" | ".circleci") {
                        return Err(format!("Hidden directory: {}", name));
                    }
                }
            }
        }

        // Check file extension
        if let Some(ext) = path_obj.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext.to_lowercase());
            if self.config.excluded_extensions.contains(&ext_with_dot) {
                return Err(format!("Excluded extension: {}", ext_with_dot));
            }
        }

        // Check filename
        if let Some(filename) = path_obj.file_name().and_then(|n| n.to_str()) {
            // Check hidden files
            if !self.config.include_hidden && filename.starts_with('.') {
                // Allow some common config files
                if !matches!(
                    filename,
                    ".env" | ".gitignore" | ".eslintrc" | ".prettierrc" | ".editorconfig"
                ) {
                    return Err("Hidden file".to_string());
                }
            }

            // Check generated patterns
            for regex in &self.generated_regexes {
                if regex.is_match(filename) {
                    return Err(format!("Generated file pattern: {}", regex.as_str()));
                }
            }
        }

        Ok(())
    }

    /// Check if content appears to be binary.
    pub fn is_binary_content(&self, content: &[u8], sample_size: usize) -> bool {
        let sample = &content[..content.len().min(sample_size)];

        // Check for null bytes (strong indicator of binary)
        if sample.contains(&0) {
            return true;
        }

        // Check ratio of non-printable characters
        let non_printable = sample
            .iter()
            .filter(|&&b| b < 32 && !matches!(b, 9 | 10 | 13)) // tab, newline, carriage return
            .count();

        if !sample.is_empty() && (non_printable as f64 / sample.len() as f64) > 0.1 {
            return true;
        }

        false
    }

    /// Get the configuration.
    pub fn config(&self) -> &FilterConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excluded_directories() {
        let filter = FileFilter::with_defaults();

        assert!(filter
            .should_process("node_modules/foo/bar.js", 100)
            .is_err());
        assert!(filter.should_process(".git/config", 100).is_err());
        assert!(filter.should_process("__pycache__/foo.pyc", 100).is_err());
    }

    #[test]
    fn test_excluded_extensions() {
        let filter = FileFilter::with_defaults();

        assert!(filter.should_process("image.png", 100).is_err());
        assert!(filter.should_process("video.mp4", 100).is_err());
        assert!(filter.should_process("archive.zip", 100).is_err());
    }

    #[test]
    fn test_allowed_files() {
        let filter = FileFilter::with_defaults();

        assert!(filter.should_process("src/main.rs", 100).is_ok());
        assert!(filter.should_process("lib/utils.py", 100).is_ok());
        assert!(filter.should_process("app/index.js", 100).is_ok());
    }

    #[test]
    fn test_size_limits() {
        let filter = FileFilter::with_defaults();

        // Empty file
        assert!(filter.should_process("empty.txt", 0).is_err());

        // Too large
        assert!(filter.should_process("huge.txt", 10 * 1024 * 1024).is_err());

        // Just right
        assert!(filter.should_process("normal.txt", 1000).is_ok());
    }

    #[test]
    fn test_binary_detection() {
        let filter = FileFilter::with_defaults();

        // Text content
        assert!(!filter.is_binary_content(b"Hello, world!", 1024));

        // Binary content (null bytes)
        assert!(filter.is_binary_content(b"\x00\x01\x02\x03", 1024));
    }
}
