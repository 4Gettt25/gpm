// osv.rs — client for https://osv.dev/
// POST https://api.osv.dev/v1/query with {package: {name, ecosystem}, version}

use anyhow::Result;

pub struct OsvClient;

impl OsvClient {
    pub fn new() -> Self { Self }

    pub async fn query_package(
        &self,
        ecosystem: &str,
        name: &str,
        version: &str,
    ) -> Result<Vec<OsvVuln>> {
        // TODO: implement with reqwest
        let _ = (ecosystem, name, version);
        Ok(vec![])
    }
}

#[derive(Debug)]
pub struct OsvVuln {
    pub id: String,
    pub severity: Option<f32>,
    pub summary: Option<String>,
}
