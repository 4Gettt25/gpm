// deps_dev.rs
// Client for Google's deps.dev API — used for transitive dep data and CVE enrichment.
// API docs: https://deps.dev/

use anyhow::Result;

pub struct DepsDev;

impl DepsDev {
    pub fn new() -> Self { Self }

    /// Fetch transitive dependencies for a specific package version.
    /// GET https://api.deps.dev/v3alpha/systems/{system}/packages/{name}/versions/{version}:dependencies
    pub async fn transitive_deps(
        &self,
        ecosystem: &str,
        name: &str,
        version: &str,
    ) -> Result<Vec<(String, String)>> {
        // TODO: implement with reqwest
        let _ = (ecosystem, name, version);
        Ok(vec![])
    }

    /// Fetch known vulnerabilities for a package version.
    pub async fn vulnerabilities(
        &self,
        ecosystem: &str,
        name: &str,
        version: &str,
    ) -> Result<Vec<String>> {
        let _ = (ecosystem, name, version);
        Ok(vec![])
    }
}
