//! gpm-security: CVE auditing and blast-radius analysis.
//!
//! Pulls advisory data from OSV (osv.dev) and deps.dev,
//! then runs graph queries to find which installed versions
//! are transitively affected.

pub mod audit;
pub mod osv;

pub use audit::AuditReport;
