//! Bash script compatibility analyzer
//!
//! This module provides tools to analyze bash scripts and identify syntax features,
//! categorizing them by support status in Rush.
//!
//! The compatibility database maps 50+ bash features to Rush support status:
//! - Supported: Fully implemented
//! - Planned: Will be implemented
//! - Not Supported: Has clear workarounds

pub mod analyzer;
pub mod database;
pub mod features;
pub mod migrate;
pub mod report;

pub use analyzer::{AnalysisResult, ScriptAnalyzer};
pub use database::CompatDatabase;
pub use features::RushCompatFeature;
pub use migrate::MigrationEngine;
pub use report::CompatibilityReport;
