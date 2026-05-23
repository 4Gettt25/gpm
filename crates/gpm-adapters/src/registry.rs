use crate::adapters::{CargoAdapter, ComposerAdapter, NpmAdapter, PypiAdapter};
use crate::trait_def::EcosystemAdapter;
use std::path::Path;
use std::sync::Arc;

/// Holds all registered adapters.
/// Call `detect(dir)` to find which ones apply to a given project directory.
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn EcosystemAdapter>>,
}

impl AdapterRegistry {
    /// Build a registry pre-loaded with all built-in adapters.
    pub fn with_defaults() -> Self {
        Self {
            adapters: vec![
                Arc::new(CargoAdapter),
                Arc::new(NpmAdapter),
                Arc::new(PypiAdapter),
                Arc::new(ComposerAdapter),
            ],
        }
    }

    /// Register a custom adapter (for plugins / future extension).
    pub fn register(&mut self, adapter: Arc<dyn EcosystemAdapter>) {
        self.adapters.push(adapter);
    }

    /// Return all adapters whose manifest files are present in `dir`.
    /// A monorepo may match more than one.
    pub fn detect(&self, dir: &Path) -> Vec<Arc<dyn EcosystemAdapter>> {
        self.adapters
            .iter()
            .filter(|a| a.detect(dir))
            .cloned()
            .collect()
    }

    /// Return a specific adapter by ecosystem name string, if registered.
    pub fn get(&self, name: &str) -> Option<Arc<dyn EcosystemAdapter>> {
        self.adapters.iter().find(|a| a.name() == name).cloned()
    }
}
