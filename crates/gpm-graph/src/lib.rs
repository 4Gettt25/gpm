/// gpm-graph: typed graph layer over KuzuDB.
///
/// Defines the node/edge schema and exposes a `GraphStore` handle
/// that the rest of the workspace uses. KuzuDB is abstracted behind
/// a thin trait so it can be swapped or mocked in tests.
pub mod schema;
pub mod store;
pub mod error;

pub use schema::{Ecosystem, PackageNode, VersionNode, DependsOnEdge, DependencyKind};
pub use store::GraphStore;
pub use error::GraphError;
