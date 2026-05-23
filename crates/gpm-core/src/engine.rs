use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::config::Config;
use crate::sync::GraphSync;
use gpm_adapters::AdapterRegistry;
use gpm_graph::GraphStore;

/// The central handle for all gpm operations.
/// Instantiate once per CLI invocation.
pub struct Engine {
    pub store: GraphStore,
    pub registry: AdapterRegistry,
    pub project_root: PathBuf,
}

impl Engine {
    /// Initialise from a config, opening the graph store and detecting ecosystems.
    pub fn new(config: &Config) -> Result<Self> {
        let store = GraphStore::open(&config.graph_db_path)?;
        let registry = AdapterRegistry::with_defaults();
        Ok(Self {
            store,
            registry,
            project_root: config.project_root.clone(),
        })
    }

    /// Scan the project directory, parse all manifests, and sync into the graph.
    pub async fn sync(&self) -> Result<()> {
        let adapters = self.registry.detect(&self.project_root);
        if adapters.is_empty() {
            anyhow::bail!(
                "no supported package manifest found in {}",
                self.project_root.display()
            );
        }

        for adapter in &adapters {
            info!(ecosystem = adapter.name(), "syncing manifest");
            let graph = adapter.parse_manifest(&self.project_root).await?;
            GraphSync::write_to_store(&self.store, &graph)?;
        }
        Ok(())
    }

    /// Add a package in the detected ecosystem(s).
    pub async fn add(&self, package: &str, dev: bool, ecosystem: Option<&str>) -> Result<()> {
        let adapter = self.resolve_adapter(ecosystem)?;
        adapter.add(&self.project_root, package, dev).await?;
        // Re-sync graph after add
        self.sync().await
    }

    /// Remove a package.
    pub async fn remove(&self, package: &str, ecosystem: Option<&str>) -> Result<()> {
        let adapter = self.resolve_adapter(ecosystem)?;
        adapter.remove(&self.project_root, package).await?;
        self.sync().await
    }

    /// Explain why a package is installed (shortest path from root).
    pub fn why(&self, project: &str, package: &str) -> Result<Vec<String>> {
        Ok(self.store.why_is_installed(project, package)?)
    }

    /// Find all versions transitively affected by a CVE.
    pub fn audit_cve(&self, project: &str, cve: &str) -> Result<Vec<String>> {
        Ok(self.store.blast_radius(project, cve)?)
    }

    /// List all transitive copyleft licenses.
    pub fn licenses(&self, project: &str) -> Result<Vec<String>> {
        Ok(self.store.copyleft_deps(project)?)
    }

    fn resolve_adapter(
        &self,
        ecosystem: Option<&str>,
    ) -> Result<Arc<dyn gpm_adapters::EcosystemAdapter>> {
        if let Some(name) = ecosystem {
            self.registry
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("unknown ecosystem: {name}"))
        } else {
            let detected = self.registry.detect(&self.project_root);
            match detected.len() {
                0 => anyhow::bail!("no ecosystem detected"),
                1 => Ok(detected.into_iter().next().unwrap()),
                _ => anyhow::bail!("multiple ecosystems detected — use --ecosystem to specify one"),
            }
        }
    }
}
