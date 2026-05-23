use crate::manifest::ManifestGraph;
use crate::trait_def::EcosystemAdapter;
use anyhow::{Context, Result};
use gpm_graph::{DependencyKind, DependsOnEdge, Ecosystem, PackageNode, VersionNode};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

pub struct NpmAdapter;

#[derive(Deserialize)]
struct PackageJson {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "peerDependencies")]
    peer_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, String>>,
}

#[async_trait::async_trait]
impl EcosystemAdapter for NpmAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Npm
    }
    fn name(&self) -> &'static str {
        "npm"
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("package.json").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        let content = tokio::fs::read_to_string(dir.join("package.json"))
            .await
            .context("reading package.json")?;
        let pkg: PackageJson = serde_json::from_str(&content).context("parsing package.json")?;

        let mut graph = ManifestGraph::new();
        let root_name = pkg.name.as_deref().unwrap_or("unknown");
        let root_semver = pkg.version.as_deref().unwrap_or("0.0.0");

        let root_ver = VersionNode {
            name: root_name.to_string(),
            ecosystem: Ecosystem::Npm,
            semver: root_semver.to_string(),
            published_at: None,
            checksum: None,
            yanked: false,
        };
        graph.packages.push(PackageNode {
            name: root_name.to_string(),
            ecosystem: Ecosystem::Npm,
            description: pkg.description,
        });
        graph.versions.push(root_ver.clone());

        let dep_sections: [(Option<HashMap<String, String>>, DependencyKind); 4] = [
            (pkg.dependencies, DependencyKind::Normal),
            (pkg.dev_dependencies, DependencyKind::Dev),
            (pkg.peer_dependencies, DependencyKind::Peer),
            (pkg.optional_dependencies, DependencyKind::Optional),
        ];

        for (deps_opt, kind) in dep_sections {
            if let Some(deps) = deps_opt {
                for (dep_name, version_req) in deps {
                    let dep_ver = VersionNode {
                        name: dep_name.clone(),
                        ecosystem: Ecosystem::Npm,
                        semver: version_req.clone(),
                        published_at: None,
                        checksum: None,
                        yanked: false,
                    };
                    graph.packages.push(PackageNode {
                        name: dep_name.clone(),
                        ecosystem: Ecosystem::Npm,
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
        let status = tokio::process::Command::new("npm")
            .arg("install")
            .current_dir(dir)
            .status()
            .await
            .context("running npm install")?;
        anyhow::ensure!(status.success(), "npm install failed");
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()> {
        let mut cmd = tokio::process::Command::new("npm");
        cmd.arg("install").arg(package).current_dir(dir);
        if dev {
            cmd.arg("--save-dev");
        }
        cmd.status().await.context("running npm install")?;
        Ok(())
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        tokio::process::Command::new("npm")
            .args(["uninstall", package])
            .current_dir(dir)
            .status()
            .await
            .context("running npm uninstall")?;
        Ok(())
    }
}
