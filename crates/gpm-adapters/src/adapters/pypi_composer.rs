use std::path::Path;
use anyhow::Result;
use gpm_graph::Ecosystem;
use crate::trait_def::EcosystemAdapter;
use crate::manifest::ManifestGraph;

// ── PyPI / uv adapter ────────────────────────────────────────────────────────

pub struct PypiAdapter;

#[async_trait::async_trait]
impl EcosystemAdapter for PypiAdapter {
    fn ecosystem(&self) -> Ecosystem { Ecosystem::PyPI }
    fn name(&self) -> &'static str { "pypi" }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("pyproject.toml").exists()
            || dir.join("requirements.txt").exists()
            || dir.join("setup.py").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        // TODO: parse pyproject.toml [project.dependencies] + uv.lock
        // For PEP 508 version specifiers, use the `pep440_rs` crate.
        let _ = dir;
        Ok(ManifestGraph::new())
    }

    async fn install(&self, dir: &Path) -> Result<()> {
        // Prefer uv if available, fall back to pip
        let has_uv = which_binary("uv");
        let (bin, args): (&str, &[&str]) = if has_uv {
            ("uv", &["sync"])
        } else {
            ("pip", &["install", "-r", "requirements.txt"])
        };
        tokio::process::Command::new(bin)
            .args(args)
            .current_dir(dir)
            .status()
            .await?;
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, _dev: bool) -> Result<()> {
        tokio::process::Command::new("uv")
            .args(["add", package])
            .current_dir(dir)
            .status()
            .await?;
        Ok(())
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        tokio::process::Command::new("uv")
            .args(["remove", package])
            .current_dir(dir)
            .status()
            .await?;
        Ok(())
    }
}

// ── Composer adapter ─────────────────────────────────────────────────────────

pub struct ComposerAdapter;

#[async_trait::async_trait]
impl EcosystemAdapter for ComposerAdapter {
    fn ecosystem(&self) -> Ecosystem { Ecosystem::Composer }
    fn name(&self) -> &'static str { "composer" }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("composer.json").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        // TODO: parse composer.json require / require-dev
        let _ = dir;
        Ok(ManifestGraph::new())
    }

    async fn install(&self, dir: &Path) -> Result<()> {
        tokio::process::Command::new("composer")
            .arg("install")
            .current_dir(dir)
            .status()
            .await?;
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()> {
        let mut cmd = tokio::process::Command::new("composer");
        cmd.arg("require").arg(package).current_dir(dir);
        if dev { cmd.arg("--dev"); }
        cmd.status().await?;
        Ok(())
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        tokio::process::Command::new("composer")
            .args(["remove", package])
            .current_dir(dir)
            .status()
            .await?;
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn which_binary(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
