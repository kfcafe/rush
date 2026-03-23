// Library interface for Rush shell
// This allows benchmarks and tests to access internal modules

pub mod arithmetic;
pub mod banner;
pub mod builtins;
pub mod compat;
pub mod completion;
pub mod config;
pub mod context;
pub mod correction;
pub mod daemon;
pub mod error;
pub mod executor;
#[cfg(feature = "git-builtins")]
pub mod git;
pub mod glob_expansion;
pub mod history;
pub mod intent;
pub mod run_api;

pub use run_api::{run, RunOptions, RunResult};
pub mod jobs;
pub mod lexer;
pub mod output;
pub mod parser;
pub mod progress;
pub mod runtime;
pub mod signal;
pub mod stats;
pub mod terminal;
pub mod undo;
pub mod value;
