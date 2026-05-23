pub mod config;
/// gpm-core: orchestration layer.
///
/// Owns the GraphStore handle and coordinates between adapters,
/// the registry client, and the graph queries.
pub mod engine;
pub mod query;
pub mod sync;

pub use config::Config;
pub use engine::Engine;
