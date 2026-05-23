pub mod adapters;
pub mod manifest;
pub mod registry;
pub mod trait_def;

pub use manifest::ManifestGraph;
pub use registry::AdapterRegistry;
pub use trait_def::EcosystemAdapter;
