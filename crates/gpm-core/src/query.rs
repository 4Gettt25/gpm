// query.rs — typed query planner wrapping raw Cypher
use anyhow::Result;
use gpm_graph::GraphStore;

pub struct QueryPlanner<'a> {
    store: &'a GraphStore,
}

impl<'a> QueryPlanner<'a> {
    pub fn new(store: &'a GraphStore) -> Self {
        Self { store }
    }

    pub fn raw(&self, cypher: &str) -> Result<Vec<String>> {
        Ok(self.store.query_raw(cypher)?)
    }
}
