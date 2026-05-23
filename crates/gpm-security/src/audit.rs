// audit.rs
use anyhow::Result;

#[derive(Debug)]
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
}

#[derive(Debug)]
pub struct AuditFinding {
    pub package: String,
    pub version: String,
    pub cve_id: String,
    pub severity: String,
    /// Packages in your project that transitively pull in the vulnerable version
    pub affected_via: Vec<String>,
}

impl AuditReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn print_summary(&self) {
        if self.is_clean() {
            println!("✓ No known vulnerabilities found.");
            return;
        }
        println!("⚠ {} vulnerabilities found:", self.findings.len());
        for f in &self.findings {
            println!(
                "  {} {} — {} ({})\n    via: {}",
                f.package,
                f.version,
                f.cve_id,
                f.severity,
                f.affected_via.join(" → "),
            );
        }
    }
}

pub struct Auditor;

#[allow(clippy::unused_async)]
impl Auditor {
    pub async fn run(&self, _project: &str) -> Result<AuditReport> {
        // TODO: query OSV for each installed version, then run blast_radius graph query
        Ok(AuditReport { findings: vec![] })
    }
}
