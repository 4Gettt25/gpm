/// GraphStore wraps KuzuDB and exposes typed operations.
///
/// The KuzuDB C FFI crate is stubbed here with comments showing the
/// real calls. Once `kuzu` lands stably on crates.io, swap the
/// `StubDb` type for `kuzu::Database` and implement each method body.
use std::path::Path;
use tracing::{debug, info};

use crate::error::GraphError;
use crate::schema::{DependsOnEdge, PackageNode, VersionNode};

// ── KuzuDB stub ──────────────────────────────────────────────────────────────
// Replace this block with:
//   use kuzu::{Connection, Database, SystemConfig};

struct StubDb {
    path: String,
}

struct StubConn;

impl StubDb {
    fn open(path: &str) -> Result<Self, GraphError> {
        Ok(StubDb { path: path.to_string() })
    }
    fn connect(&self) -> Result<StubConn, GraphError> {
        Ok(StubConn)
    }
}

impl StubConn {
    fn query(&self, cypher: &str) -> Result<Vec<String>, GraphError> {
        debug!(cypher, "stub query");
        Ok(vec![])
    }
}

// ── GraphStore ───────────────────────────────────────────────────────────────

pub struct GraphStore {
    _db: StubDb,
    conn: StubConn,
}

impl GraphStore {
    /// Open (or create) the graph at `db_path` and run schema migrations.
    pub fn open(db_path: &Path) -> Result<Self, GraphError> {
        let path_str = db_path.to_string_lossy().to_string();
        info!(path = %path_str, "opening graph store");

        let db = StubDb::open(&path_str)?;
        let conn = db.connect()?;
        let store = Self { _db: db, conn };

        store.migrate()?;
        Ok(store)
    }

    /// Idempotent schema creation. Safe to call on every startup.
    fn migrate(&self) -> Result<(), GraphError> {
        info!("running schema migrations");

        // KuzuDB uses CREATE NODE TABLE / CREATE REL TABLE DDL.
        // All statements are idempotent via IF NOT EXISTS.

        let stmts = [
            // ── Node tables ─────────────────────────────────────────────
            "CREATE NODE TABLE IF NOT EXISTS Package (
                name        STRING,
                ecosystem   STRING,
                description STRING,
                PRIMARY KEY (name, ecosystem)
            )",

            "CREATE NODE TABLE IF NOT EXISTS Version (
                id           STRING,   -- '{ecosystem}:{name}:{semver}'
                name         STRING,
                ecosystem    STRING,
                semver       STRING,
                published_at TIMESTAMP,
                checksum     STRING,
                yanked       BOOLEAN,
                PRIMARY KEY (id)
            )",

            "CREATE NODE TABLE IF NOT EXISTS Project (
                name      STRING,
                root_path STRING,
                PRIMARY KEY (root_path)
            )",

            "CREATE NODE TABLE IF NOT EXISTS Manifest (
                path      STRING,
                ecosystem STRING,
                PRIMARY KEY (path)
            )",

            "CREATE NODE TABLE IF NOT EXISTS Vulnerability (
                cve_id   STRING,
                cvss     DOUBLE,
                severity STRING,
                summary  STRING,
                PRIMARY KEY (cve_id)
            )",

            "CREATE NODE TABLE IF NOT EXISTS License (
                spdx_id      STRING,
                is_osi       BOOLEAN,
                is_copyleft  BOOLEAN,
                PRIMARY KEY (spdx_id)
            )",

            // ── Relationship tables ──────────────────────────────────────
            "CREATE REL TABLE IF NOT EXISTS HAS_VERSION (
                FROM Package TO Version
            )",

            // Core edge: Version → Version dependency
            "CREATE REL TABLE IF NOT EXISTS DEPENDS_ON (
                FROM Version TO Version,
                kind        STRING,
                version_req STRING
            )",

            "CREATE REL TABLE IF NOT EXISTS HAS_MANIFEST (
                FROM Project TO Manifest
            )",

            "CREATE REL TABLE IF NOT EXISTS REQUIRES (
                FROM Manifest TO Version
            )",

            "CREATE REL TABLE IF NOT EXISTS HAS_VULN (
                FROM Version TO Vulnerability
            )",

            "CREATE REL TABLE IF NOT EXISTS LICENSED_UNDER (
                FROM Version TO License
            )",
        ];

        for stmt in &stmts {
            self.conn.query(stmt).map_err(|e| {
                GraphError::Migration(format!("statement failed: {e}\nSQL: {stmt}"))
            })?;
        }

        info!("schema migrations complete");
        Ok(())
    }

    // ── Write operations ─────────────────────────────────────────────────────

    pub fn upsert_package(&self, pkg: &PackageNode) -> Result<(), GraphError> {
        let cypher = format!(
            "MERGE (p:Package {{name: '{}', ecosystem: '{}'}})
             ON CREATE SET p.description = '{}'
             ON MATCH SET  p.description = '{}'",
            pkg.name,
            pkg.ecosystem,
            pkg.description.as_deref().unwrap_or(""),
            pkg.description.as_deref().unwrap_or(""),
        );
        self.conn.query(&cypher)?;
        Ok(())
    }

    pub fn upsert_version(&self, ver: &VersionNode) -> Result<(), GraphError> {
        let cypher = format!(
            "MERGE (v:Version {{id: '{}'}})
             ON CREATE SET
               v.name = '{}',
               v.ecosystem = '{}',
               v.semver = '{}',
               v.yanked = {}",
            ver.id(),
            ver.name,
            ver.ecosystem,
            ver.semver,
            ver.yanked,
        );
        self.conn.query(&cypher)?;
        Ok(())
    }

    pub fn upsert_depends_on(&self, edge: &DependsOnEdge) -> Result<(), GraphError> {
        let cypher = format!(
            "MATCH (a:Version {{id: '{}'}}), (b:Version {{id: '{}'}})
             MERGE (a)-[r:DEPENDS_ON {{kind: '{}', version_req: '{}'}}]->(b)",
            edge.from_id,
            edge.to_id,
            edge.kind,
            edge.version_req,
        );
        self.conn.query(&cypher)?;
        Ok(())
    }

    // ── Read / query operations ───────────────────────────────────────────────

    /// Raw Cypher passthrough — used by QueryPlanner in gpm-core.
    pub fn query_raw(&self, cypher: &str) -> Result<Vec<String>, GraphError> {
        self.conn.query(cypher)
    }

    /// Shortest path from a project root to a named package.
    /// Returns each hop in the chain as a string.
    pub fn why_is_installed(
        &self,
        project: &str,
        package: &str,
    ) -> Result<Vec<String>, GraphError> {
        let cypher = format!(
            "MATCH path = shortestPath(
               (root:Project {{name: '{project}'}})
                 -[:HAS_MANIFEST|REQUIRES|DEPENDS_ON*]->
               (v:Version)-[:HAS_VERSION]->(p:Package {{name: '{package}'}})
             )
             RETURN [node IN nodes(path) | node.name] AS chain"
        );
        self.conn.query(&cypher)
    }

    /// All versions in the project transitively affected by a CVE.
    pub fn blast_radius(
        &self,
        project: &str,
        cve_id: &str,
    ) -> Result<Vec<String>, GraphError> {
        let cypher = format!(
            "MATCH (v:Version)-[:DEPENDS_ON*]->(vv:Version)
                   -[:HAS_VULN]->(vuln:Vulnerability {{cve_id: '{cve_id}'}})
             WHERE (v)<-[:REQUIRES]-(:Manifest)<-[:HAS_MANIFEST]-
                   (:Project {{name: '{project}'}})
             RETURN DISTINCT v.semver, vuln.severity
             ORDER BY vuln.severity DESC"
        );
        self.conn.query(&cypher)
    }

    /// All transitive dependencies with copyleft licenses.
    pub fn copyleft_deps(&self, project: &str) -> Result<Vec<String>, GraphError> {
        let cypher = format!(
            "MATCH (root:Project {{name: '{project}'}})
               -[:HAS_MANIFEST|REQUIRES|DEPENDS_ON*]->
               (v:Version)-[:LICENSED_UNDER]->(l:License)
             WHERE l.is_copyleft = true
             RETURN DISTINCT v.name, l.spdx_id
             ORDER BY l.spdx_id"
        );
        self.conn.query(&cypher)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_and_migrate() {
        let dir = tempdir().unwrap();
        let store = GraphStore::open(dir.path());
        // With the stub db this always succeeds; replace with real assertions
        // once KuzuDB is wired in.
        assert!(store.is_ok());
    }

    #[test]
    fn test_version_id_format() {
        use crate::schema::{Ecosystem, VersionNode};
        let v = VersionNode {
            name: "lodash".into(),
            ecosystem: Ecosystem::Npm,
            semver: "4.17.21".into(),
            published_at: None,
            checksum: None,
            yanked: false,
        };
        assert_eq!(v.id(), "npm:lodash:4.17.21");
    }
}
