use std::path::PathBuf;

/// Runtime configuration, built from CLI flags + ~/.config/gpm/config.toml.
pub struct Config {
    /// Directory containing the project manifest(s).
    pub project_root: PathBuf,
    /// Path to the KuzuDB graph file. Defaults to <project_root>/.gpm/graph.
    pub graph_db_path: PathBuf,
}

impl Config {
    pub fn from_cwd() -> anyhow::Result<Self> {
        let root = std::env::current_dir()?;
        let db = root.join(".gpm").join("graph");
        Ok(Self {
            project_root: root,
            graph_db_path: db,
        })
    }

    pub fn with_root(root: PathBuf) -> Self {
        let db = root.join(".gpm").join("graph");
        Self {
            project_root: root,
            graph_db_path: db,
        }
    }
}
