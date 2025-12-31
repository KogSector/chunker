//! Processing module for file analysis and preprocessing.
//!
//! This module provides:
//! - Language detection from file extensions and content
//! - File filtering (exclude binaries, vendor directories, etc.)
//! - Encoding validation and normalization

pub mod file_processor;
pub mod filter;
pub mod language;

pub use file_processor::{FileProcessor, ProcessableFile, ProcessableResult};
pub use filter::{FileFilter, FilterConfig};
pub use language::{Language, LanguageInfo};
