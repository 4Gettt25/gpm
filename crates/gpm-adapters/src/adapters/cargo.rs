use crate::manifest::ManifestGraph;
use crate::trait_def::EcosystemAdapter;
use anyhow::{Context, Result};
use gpm_graph::{DependencyKind, DependsOnEdge, Ecosystem, PackageNode, VersionNode};
use std::path::Path;

pub struct CargoAdapter;

#[async_trait::async_trait]
impl EcosystemAdapter for CargoAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Cargo
    }
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("Cargo.toml").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        let cargo_toml = dir.join("Cargo.toml");
        let content = tokio::fs::read_to_string(&cargo_toml)
            .await
            .context("reading Cargo.toml")?;

        let doc: toml::Value = toml::from_str(&content).context("parsing Cargo.toml")?;
        let mut graph = ManifestGraph::new();

        // Root package
        let root_name = doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");
        let root_version = doc
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0");

        let root_ver = VersionNode {
            name: root_name.to_string(),
            ecosystem: Ecosystem::Cargo,
            semver: root_version.to_string(),
            published_at: None,
            checksum: None,
            yanked: false,
        };
        graph.packages.push(PackageNode {
            name: root_name.to_string(),
            ecosystem: Ecosystem::Cargo,
            description: doc
                .get("package")
                .and_then(|p| p.get("description"))
                .and_then(|d| d.as_str())
                .map(String::from),
        });
        graph.versions.push(root_ver.clone());

        // Parse [dependencies] and [dev-dependencies]
        for (section, kind) in &[
            ("dependencies", DependencyKind::Normal),
            ("dev-dependencies", DependencyKind::Dev),
            ("build-dependencies", DependencyKind::Build),
        ] {
            if let Some(deps) = doc.get(section).and_then(|d| d.as_table()) {
                for (dep_name, dep_val) in deps {
                    let version_req = match dep_val {
                        toml::Value::String(v) => v.clone(),
                        toml::Value::Table(t) => t
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("*")
                            .to_string(),
                        _ => "*".to_string(),
                    };

                    // We create a placeholder VersionNode for the dep.
                    // gpm-registry will resolve this to a concrete version later.
                    let dep_ver = VersionNode {
                        name: dep_name.clone(),
                        ecosystem: Ecosystem::Cargo,
                        semver: version_req.clone(), // placeholder until resolved
                        published_at: None,
                        checksum: None,
                        yanked: false,
                    };

                    graph.packages.push(PackageNode {
                        name: dep_name.clone(),
                        ecosystem: Ecosystem::Cargo,
                        description: None,
                    });
                    graph.versions.push(dep_ver.clone());
                    graph.depends_on.push(DependsOnEdge {
                        from_id: root_ver.id(),
                        to_id: dep_ver.id(),
                        kind: kind.clone(),
                        version_req,
                    });
                }
            }
        }

        Ok(graph)
    }

    async fn install(&self, dir: &Path) -> Result<()> {
        let status = tokio::process::Command::new("cargo")
            .arg("build")
            .current_dir(dir)
            .status()
            .await
            .context("running cargo build")?;
        anyhow::ensure!(status.success(), "cargo build failed");
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()> {
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.arg("add").arg(package).current_dir(dir);
        if dev {
            cmd.arg("--dev");
        }
        let status = cmd.status().await.context("running cargo add")?;
        anyhow::ensure!(status.success(), "cargo add failed");
        Ok(())
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        let status = tokio::process::Command::new("cargo")
            .args(["remove", package])
            .current_dir(dir)
            .status()
            .await
            .context("running cargo remove")?;
        anyhow::ensure!(status.success(), "cargo remove failed");
        Ok(())
    }
}
