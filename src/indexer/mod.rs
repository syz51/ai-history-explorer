//! Index building for Claude Code conversation history
//!
//! # Error Handling Strategy
//!
//! The indexer combines graceful degradation with error rate tracking:
//!
//! - **Project-level failures**: Failed agent file parses are logged but don't stop indexing.
//!   This allows partial index building when some projects have corrupted files.
//!
//! - **Error rate tracking**: Tracks successful vs failed agent file parses. Returns an error
//!   if >50% of agent files fail, preventing acceptance of fundamentally broken data.
//!
//! - **Summary reporting**: Prints statistics showing total entries indexed, files parsed,
//!   and failures, giving users visibility into index completeness.
//!
//! - **Parser integration**: Delegates line-level error handling to parser modules, which
//!   apply their own graceful degradation and failure rate checks.

pub mod builder;
pub mod project_discovery;

pub use builder::build_index;
pub use project_discovery::discover_projects;
