/// gpm-registry: async HTTP clients for package registries.
///
/// Each registry client fetches version metadata and maps it into
/// gpm-graph types. Results are cached in-memory (moka) to avoid
/// redundant network calls within a single gpm invocation.
///
/// Supported registries:
///   - crates.io   (Cargo)
///   - registry.npmjs.org (npm)
///   - pypi.org/pypi (PyPI)
///   - packagist.org (Composer)
///   - deps.dev API  (fallback / CVE enrichment)

pub mod client;
pub mod deps_dev;

pub use client::RegistryClient;
