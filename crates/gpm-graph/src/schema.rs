use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Ecosystem ────────────────────────────────────────────────────────────────

/// Every supported package ecosystem. Stored as a string label in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    PyPI,
    Cargo,
    Composer,
    Maven,
    NuGet,
    Go,
    RubyGems,
    Hex,
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Ecosystem::Npm => "npm",
            Ecosystem::PyPI => "pypi",
            Ecosystem::Cargo => "cargo",
            Ecosystem::Composer => "composer",
            Ecosystem::Maven => "maven",
            Ecosystem::NuGet => "nuget",
            Ecosystem::Go => "go",
            Ecosystem::RubyGems => "rubygems",
            Ecosystem::Hex => "hex",
        };
        write!(f, "{}", s)
    }
}

// ── Nodes ────────────────────────────────────────────────────────────────────

/// A package — the logical name within an ecosystem.
/// e.g. { name: "lodash", ecosystem: Npm }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageNode {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub description: Option<String>,
}

/// A specific published version of a package.
/// Nodes are keyed by (name, ecosystem, semver).
///
/// DEPENDS_ON edges connect Version → Version, not Package → Package.
/// This lets us model exact transitive resolution across all ecosystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNode {
    /// Canonical package name
    pub name: String,
    pub ecosystem: Ecosystem,
    /// Normalized semver string, e.g. "1.2.3"
    pub semver: String,
    pub published_at: Option<DateTime<Utc>>,
    /// SHA-256 or ecosystem-native checksum
    pub checksum: Option<String>,
    /// Yanked / retracted versions are kept but flagged
    pub yanked: bool,
}

impl VersionNode {
    /// Stable unique key for use as a graph node ID.
    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.ecosystem, self.name, self.semver)
    }
}

/// A project on disk that owns one or more manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNode {
    pub name: String,
    pub root_path: String,
}

/// A manifest file parsed from disk (package.json, Cargo.toml, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestNode {
    pub path: String,
    pub ecosystem: Ecosystem,
}

/// A known vulnerability (sourced from OSV / deps.dev).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityNode {
    pub cve_id: String,
    /// CVSS score 0.0–10.0
    pub cvss: Option<f32>,
    pub severity: VulnSeverity,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VulnSeverity {
    Critical,
    High,
    Medium,
    Low,
    None,
}

/// An SPDX license.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseNode {
    pub spdx_id: String,
    pub is_osi_approved: bool,
    pub is_copyleft: bool,
}

// ── Edges ────────────────────────────────────────────────────────────────────

/// The core edge: one version depends on another.
///
/// `kind` encodes whether this is a runtime, dev, optional, or peer dep.
/// `version_req` stores the original constraint string (e.g. "^1.2", ">=3,<4").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependsOnEdge {
    pub from_id: String, // VersionNode::id()
    pub to_id: String,   // VersionNode::id()
    pub kind: DependencyKind,
    /// Raw version constraint as written in the manifest
    pub version_req: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyKind {
    /// Normal runtime dependency
    Normal,
    /// Dev / test only
    Dev,
    /// Optional feature dependency
    Optional,
    /// Peer dependency (npm-style)
    Peer,
    /// Build-time only
    Build,
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DependencyKind::Normal => "normal",
            DependencyKind::Dev => "dev",
            DependencyKind::Optional => "optional",
            DependencyKind::Peer => "peer",
            DependencyKind::Build => "build",
        };
        write!(f, "{}", s)
    }
}
