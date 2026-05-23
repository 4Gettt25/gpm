// client.rs
// Registry HTTP client. reqwest + in-memory cache.
// Add `reqwest` and `moka` to Cargo.toml when wiring up real HTTP.

use anyhow::Result;
use gpm_graph::Ecosystem;

pub struct RegistryClient;

impl RegistryClient {
    pub fn new() -> Self { Self }

    /// Fetch the latest stable version of a package from its registry.
    pub async fn latest_version(&self, ecosystem: &Ecosystem, name: &str) -> Result<String> {
        // TODO: match ecosystem, call the appropriate registry API
        // Cargo  → GET https://crates.io/api/v1/crates/{name}
        // npm    → GET https://registry.npmjs.org/{name}/latest
        // PyPI   → GET https://pypi.org/pypi/{name}/json
        // deps.dev fallback for others
        let _ = (ecosystem, name);
        Ok("0.0.0".to_string())
    }

    /// Fetch all known versions of a package (for upgrade planning).
    pub async fn all_versions(&self, ecosystem: &Ecosystem, name: &str) -> Result<Vec<String>> {
        let _ = (ecosystem, name);
        Ok(vec![])
    }
}
