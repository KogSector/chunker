//! File processor for the code intelligence pipeline.
//!
//! Combines language detection, filtering, and encoding validation
//! to prepare files for AST parsing and chunking.

use anyhow::{anyhow, Result};
use std::str;

use crate::processing::filter::{FileFilter, FilterConfig};
use crate::processing::language::{Language, LanguageDetector, LanguageInfo};

/// A file that has been processed and is ready for parsing.
#[derive(Debug, Clone)]
pub struct ProcessableFile {
    /// Original file path.
    pub path: String,
    /// Detected language.
    pub language: Language,
    /// File content as a string.
    pub content: String,
    /// File size in bytes.
    pub size_bytes: usize,
    /// Language detection confidence (0.0 - 1.0).
    pub language_confidence: f32,
    /// Whether AST parsing is supported.
    pub ast_supported: bool,
}

/// Result of checking if a file is processable.
#[derive(Debug)]
pub struct ProcessableResult {
    /// Whether the file can be processed.
    pub is_processable: bool,
    /// Reason for rejection (if not processable).
    pub reason: Option<String>,
    /// Language info (if processable).
    pub language_info: Option<LanguageInfo>,
    /// Warnings (non-fatal issues).
    pub warnings: Vec<String>,
}

impl ProcessableResult {
    /// Create a successful result.
    pub fn success(language_info: LanguageInfo) -> Self {
        Self {
            is_processable: true,
            reason: None,
            language_info: Some(language_info),
            warnings: Vec::new(),
        }
    }

    /// Create a successful result with warnings.
    pub fn success_with_warnings(language_info: LanguageInfo, warnings: Vec<String>) -> Self {
        Self {
            is_processable: true,
            reason: None,
            language_info: Some(language_info),
            warnings,
        }
    }

    /// Create a rejection result.
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            is_processable: false,
            reason: Some(reason.into()),
            language_info: None,
            warnings: Vec::new(),
        }
    }
}

/// File processor for preparation and validation.
pub struct FileProcessor {
    filter: FileFilter,
    language_detector: LanguageDetector,
}

impl Default for FileProcessor {
    fn default() -> Self {
        Self::new(FilterConfig::default())
    }
}

impl FileProcessor {
    /// Create a new file processor with the given configuration.
    pub fn new(config: FilterConfig) -> Self {
        Self {
            filter: FileFilter::new(config),
            language_detector: LanguageDetector::new(),
        }
    }

    /// Check if a file should be processed (without content).
    pub fn is_processable(&self, path: &str, size: usize) -> ProcessableResult {
        // Check filter rules
        if let Err(reason) = self.filter.should_process(path, size) {
            return ProcessableResult::rejected(reason);
        }

        // Detect language
        let language_info = self.language_detector.detect(path, None);

        let mut warnings = Vec::new();
        if language_info.language == Language::Unknown {
            warnings.push(format!("Unknown language for file: {}", path));
        }

        ProcessableResult::success_with_warnings(language_info, warnings)
    }

    /// Check if content is processable and detect language.
    pub fn is_content_processable(
        &self,
        path: &str,
        content: &[u8],
    ) -> ProcessableResult {
        let size = content.len();

        // Check filter rules
        if let Err(reason) = self.filter.should_process(path, size) {
            return ProcessableResult::rejected(reason);
        }

        // Check if binary
        if self.filter.is_binary_content(content, 8192) {
            return ProcessableResult::rejected("Binary file detected");
        }

        // Try to decode as text
        let text = match self.validate_encoding(content) {
            Ok((text, _)) => text,
            Err(e) => return ProcessableResult::rejected(e.to_string()),
        };

        // Detect language with content
        let language_info = self.language_detector.detect(path, Some(&text));

        let mut warnings = Vec::new();
        if language_info.language == Language::Unknown {
            warnings.push(format!("Unknown language for file: {}", path));
        }

        ProcessableResult::success_with_warnings(language_info, warnings)
    }

    /// Process a file's content and prepare it for parsing.
    pub fn process(
        &self,
        path: &str,
        content: &[u8],
    ) -> Result<ProcessableFile> {
        let size = content.len();

        // Check filter rules
        if let Err(reason) = self.filter.should_process(path, size) {
            return Err(anyhow!("File rejected: {}", reason));
        }

        // Check if binary
        if self.filter.is_binary_content(content, 8192) {
            return Err(anyhow!("Binary file detected"));
        }

        // Decode content
        let (text, _encoding) = self.validate_encoding(content)?;

        // Normalize line endings
        let text = self.normalize_line_endings(&text);

        // Detect language
        let language_info = self.language_detector.detect(path, Some(&text));

        Ok(ProcessableFile {
            path: path.to_string(),
            language: language_info.language,
            content: text,
            size_bytes: size,
            language_confidence: language_info.confidence,
            ast_supported: language_info.ast_supported,
        })
    }

    /// Validate and decode content with encoding detection.
    ///
    /// Returns the decoded string and the encoding used.
    pub fn validate_encoding(&self, content: &[u8]) -> Result<(String, String)> {
        // Try UTF-8 first (most common)
        if let Ok(s) = str::from_utf8(content) {
            return Ok((s.to_string(), "utf-8".to_string()));
        }

        // Try UTF-16 LE (Windows)
        if content.len() >= 2 && content[0] == 0xFF && content[1] == 0xFE {
            // UTF-16 LE BOM
            let utf16: Vec<u16> = content[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&utf16) {
                return Ok((s, "utf-16-le".to_string()));
            }
        }

        // Try UTF-16 BE
        if content.len() >= 2 && content[0] == 0xFE && content[1] == 0xFF {
            // UTF-16 BE BOM
            let utf16: Vec<u16> = content[2..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&utf16) {
                return Ok((s, "utf-16-be".to_string()));
            }
        }

        // Fallback to Latin-1 (always succeeds)
        let s: String = content.iter().map(|&b| b as char).collect();
        Ok((s, "latin-1".to_string()))
    }

    /// Normalize line endings to Unix-style (LF).
    pub fn normalize_line_endings(&self, content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Get file statistics.
    pub fn get_file_stats(&self, content: &str) -> FileStats {
        let lines: Vec<&str> = content.lines().collect();
        let non_empty_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();

        FileStats {
            total_lines: lines.len(),
            non_empty_lines,
            total_chars: content.len(),
            avg_line_length: if lines.is_empty() {
                0.0
            } else {
                content.len() as f64 / lines.len() as f64
            },
        }
    }

    /// Get the underlying filter.
    pub fn filter(&self) -> &FileFilter {
        &self.filter
    }

    /// Get the language detector.
    pub fn language_detector(&self) -> &LanguageDetector {
        &self.language_detector
    }
}

/// Statistics about a file's content.
#[derive(Debug, Clone)]
pub struct FileStats {
    /// Total number of lines.
    pub total_lines: usize,
    /// Number of non-empty lines.
    pub non_empty_lines: usize,
    /// Total characters.
    pub total_chars: usize,
    /// Average line length.
    pub avg_line_length: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_python_file() {
        let processor = FileProcessor::default();
        let content = b"def hello():\n    print('Hello, world!')\n";

        let result = processor.process("main.py", content).unwrap();

        assert_eq!(result.language, Language::Python);
        assert!(result.ast_supported);
        assert!(result.language_confidence > 0.9);
    }

    #[test]
    fn test_process_rust_file() {
        let processor = FileProcessor::default();
        let content = b"fn main() {\n    println!(\"Hello, world!\");\n}\n";

        let result = processor.process("main.rs", content).unwrap();

        assert_eq!(result.language, Language::Rust);
        assert!(result.ast_supported);
    }

    #[test]
    fn test_reject_binary() {
        let processor = FileProcessor::default();
        let content = b"\x00\x01\x02\x03\x04\x05";

        let result = processor.process("binary.dat", content);
        assert!(result.is_err());
    }

    #[test]
    fn test_line_ending_normalization() {
        let processor = FileProcessor::default();

        // Windows line endings
        let content = "line1\r\nline2\r\nline3";
        let normalized = processor.normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3");

        // Old Mac line endings
        let content = "line1\rline2\rline3";
        let normalized = processor.normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3");
    }

    #[test]
    fn test_file_stats() {
        let processor = FileProcessor::default();
        let content = "line1\nline2\n\nline4\n";

        let stats = processor.get_file_stats(content);

        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.non_empty_lines, 3);
    }
}
