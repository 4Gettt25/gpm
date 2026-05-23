pub mod trait_def;
pub mod manifest;
pub mod adapters;
pub mod registry;

pub use trait_def::EcosystemAdapter;
pub use manifest::ManifestGraph;
pub use registry::AdapterRegistry;
