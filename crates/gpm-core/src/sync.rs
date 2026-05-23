// sync.rs — writes a ManifestGraph into the GraphStore
use anyhow::Result;
use gpm_graph::GraphStore;
use gpm_adapters::ManifestGraph;

pub struct GraphSync;

impl GraphSync {
    pub fn write_to_store(store: &GraphStore, graph: ManifestGraph) -> Result<()> {
        for pkg in &graph.packages {
            store.upsert_package(pkg)?;
        }
        for ver in &graph.versions {
            store.upsert_version(ver)?;
        }
        for edge in &graph.depends_on {
            store.upsert_depends_on(edge)?;
        }
        Ok(())
    }
}
