use std::path::Path;
use anyhow::Result;
use gpm_graph::Ecosystem;
use crate::manifest::ManifestGraph;

/// The extension point for every language ecosystem.
///
/// Implement this trait to add support for a new package manager.
/// Each adapter is responsible for:
///   1. Detecting whether a directory contains its manifest file.
///   2. Parsing that manifest (+ lockfile) into the common graph schema.
///   3. Delegating actual install/remove to the underlying PM binary.
///
/// The trait is object-safe so adapters can be stored as `Box<dyn EcosystemAdapter>`.
#[async_trait::async_trait]
pub trait EcosystemAdapter: Send + Sync {
    /// Which ecosystem this adapter handles.
    fn ecosystem(&self) -> Ecosystem;

    /// Human-readable name, e.g. "npm", "cargo", "uv/pip".
    fn name(&self) -> &'static str;

    /// Returns true if `dir` contains this ecosystem's manifest file.
    fn detect(&self, dir: &Path) -> bool;

    /// Parse the manifest (and lockfile if present) in `dir` into
    /// the common graph representation. Does NOT install anything.
    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph>;

    /// Invoke the underlying package manager to install resolved deps.
    async fn install(&self, dir: &Path) -> Result<()>;

    /// Add a package by name (delegates to underlying PM, then re-parses).
    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()>;

    /// Remove a package by name.
    async fn remove(&self, dir: &Path, package: &str) -> Result<()>;
}
