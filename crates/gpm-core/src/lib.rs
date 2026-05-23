/// gpm-core: orchestration layer.
///
/// Owns the GraphStore handle and coordinates between adapters,
/// the registry client, and the graph queries.
pub mod engine;
pub mod sync;
pub mod query;
pub mod config;

pub use engine::Engine;
pub use config::Config;
