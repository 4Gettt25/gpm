use gpm_graph::{PackageNode, VersionNode, DependsOnEdge};

/// The parsed output of any manifest file, expressed as graph nodes + edges.
/// This is what every EcosystemAdapter returns from `parse_manifest`.
///
/// gpm-core then writes this into the GraphStore.
#[derive(Debug, Default)]
pub struct ManifestGraph {
    pub packages:  Vec<PackageNode>,
    pub versions:  Vec<VersionNode>,
    pub depends_on: Vec<DependsOnEdge>,
}

impl ManifestGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&mut self, other: ManifestGraph) {
        self.packages.extend(other.packages);
        self.versions.extend(other.versions);
        self.depends_on.extend(other.depends_on);
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}
