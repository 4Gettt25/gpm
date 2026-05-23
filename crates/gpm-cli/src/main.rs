use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use gpm_core::{Config, Engine};

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "gpm",
    about = "graph package manager — cross-language dependency intelligence",
    version,
    propagate_version = true,
)]
struct Cli {
    /// Override the project root directory
    #[arg(long, global = true)]
    root: Option<std::path::PathBuf>,

    /// Force a specific ecosystem (cargo, npm, pypi, composer…)
    #[arg(long, short = 'e', global = true)]
    ecosystem: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a package and update the dependency graph
    Add {
        package: String,
        /// Add as a dev dependency
        #[arg(long, short)]
        dev: bool,
    },

    /// Remove a package and prune the graph
    Remove { package: String },

    /// Install all dependencies listed in the manifest
    Install,

    /// Sync the graph with the current state of all manifests
    Sync,

    /// Explain why a package is installed (shortest path from root)
    Why {
        package: String,
        /// Project name (defaults to directory name)
        #[arg(long)]
        project: Option<String>,
    },

    /// Scan for known CVEs in the transitive dependency graph
    Audit {
        /// Check blast radius of a specific CVE
        #[arg(long)]
        cve: Option<String>,
    },

    /// List all transitive licenses
    Licenses {
        /// Show only copyleft licenses
        #[arg(long)]
        copyleft: bool,
    },

    /// Export the dependency graph
    Graph {
        /// Output format: dot | json | cytoscape
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Run a raw Cypher query against the graph store
    Query { cypher: String },
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time()
        .init();

    let cli = Cli::parse();

    // Build config from CLI flags
    let config = if let Some(root) = cli.root {
        Config::with_root(root)
    } else {
        Config::from_cwd()?
    };

    let engine = Engine::new(&config)?;

    match cli.command {
        Commands::Sync => {
            engine.sync().await?;
            println!("✓ graph synced");
        }

        Commands::Install => {
            let adapters = engine.registry.detect(&config.project_root);
            if adapters.is_empty() {
                anyhow::bail!("no supported manifest found");
            }
            for adapter in &adapters {
                println!("→ installing with {}", adapter.name());
                adapter.install(&config.project_root).await?;
            }
            engine.sync().await?;
            println!("✓ installed + graph synced");
        }

        Commands::Add { package, dev } => {
            println!("→ adding {package}");
            engine.add(&package, dev, cli.ecosystem.as_deref()).await?;
            println!("✓ added {package}");
        }

        Commands::Remove { package } => {
            println!("→ removing {package}");
            engine.remove(&package, cli.ecosystem.as_deref()).await?;
            println!("✓ removed {package}");
        }

        Commands::Why { package, project } => {
            let proj = project.unwrap_or_else(|| {
                config.project_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "project".to_string())
            });
            let chain = engine.why(&proj, &package)?;
            if chain.is_empty() {
                println!("{package} is not in the graph");
            } else {
                println!("Why is {package} installed?");
                println!("  {}", chain.join(" → "));
            }
        }

        Commands::Audit { cve } => {
            if let Some(cve_id) = cve {
                let proj = config.project_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "project".to_string());
                let hits = engine.audit_cve(&proj, &cve_id)?;
                if hits.is_empty() {
                    println!("✓ No packages affected by {cve_id}");
                } else {
                    println!("⚠ Packages affected by {cve_id}:");
                    for h in hits { println!("  {h}"); }
                }
            } else {
                println!("Running full audit…");
                // TODO: integrate gpm-security::Auditor
                println!("(full audit not yet implemented — use --cve <ID> for now)");
            }
        }

        Commands::Licenses { copyleft } => {
            let proj = config.project_root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string());
            if copyleft {
                let deps = engine.licenses(&proj)?;
                for d in deps { println!("{d}"); }
            } else {
                println!("(full license list not yet implemented — use --copyleft for now)");
            }
        }

        Commands::Graph { format } => {
            println!("exporting graph as {format}… (not yet implemented)");
        }

        Commands::Query { cypher } => {
            let rows = engine.store.query_raw(&cypher)?;
            for r in rows { println!("{r}"); }
        }
    }

    Ok(())
}
