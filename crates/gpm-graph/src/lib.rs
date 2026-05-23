pub mod error;
/// gpm-graph: typed graph layer over KuzuDB.
///
/// Defines the node/edge schema and exposes a `GraphStore` handle
/// that the rest of the workspace uses. KuzuDB is abstracted behind
/// a thin trait so it can be swapped or mocked in tests.
pub mod schema;
pub mod store;

pub use error::GraphError;
pub use schema::{DependencyKind, DependsOnEdge, Ecosystem, PackageNode, VersionNode};
pub use store::GraphStore;
