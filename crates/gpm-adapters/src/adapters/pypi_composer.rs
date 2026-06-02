use crate::manifest::ManifestGraph;
use crate::toolchain::ToolchainManager;
use crate::trait_def::EcosystemAdapter;
use anyhow::{Context, Result};
use gpm_graph::{DependencyKind, DependsOnEdge, Ecosystem, PackageNode, VersionNode};
use std::path::Path;
use toml_edit::DocumentMut;

// ── PyPI adapter ──────────────────────────────────────────────────────────────

pub struct PypiAdapter;

#[async_trait::async_trait]
impl EcosystemAdapter for PypiAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::PyPI
    }
    fn name(&self) -> &'static str {
        "pypi"
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        if dir.join("pyproject.toml").exists() {
            parse_pyproject(dir).await
        } else {
            parse_requirements_txt(dir).await
        }
    }

    async fn install(&self, dir: &Path) -> Result<()> {
        // Use uv as the primary tool (gpm can download it); gpm will prompt to install if missing
        let uv = ToolchainManager::load()?.resolve("uv").await?;
        let status = tokio::process::Command::new(&uv)
            .arg("sync")
            .current_dir(dir)
            .status()
            .await
            .with_context(|| format!("running {} sync", uv.display()))?;
        anyhow::ensure!(status.success(), "uv sync failed");
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()> {
        if dir.join("pyproject.toml").exists() {
            pyproject_add(dir, package, dev).await
        } else {
            requirements_txt_add(dir, package).await
        }
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        if dir.join("pyproject.toml").exists() {
            pyproject_remove(dir, package).await
        } else {
            requirements_txt_remove(dir, package).await
        }
    }
}

// ── Composer adapter ──────────────────────────────────────────────────────────

pub struct ComposerAdapter;

#[async_trait::async_trait]
impl EcosystemAdapter for ComposerAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Composer
    }
    fn name(&self) -> &'static str {
        "composer"
    }

    fn detect(&self, dir: &Path) -> bool {
        dir.join("composer.json").exists()
    }

    async fn parse_manifest(&self, dir: &Path) -> Result<ManifestGraph> {
        parse_composer_json(dir).await
    }

    async fn install(&self, dir: &Path) -> Result<()> {
        let composer = ToolchainManager::load()?.resolve("composer").await?;
        let status = tokio::process::Command::new(&composer)
            .arg("install")
            .current_dir(dir)
            .status()
            .await
            .with_context(|| format!("running {} install", composer.display()))?;
        anyhow::ensure!(status.success(), "composer install failed");
        Ok(())
    }

    async fn add(&self, dir: &Path, package: &str, dev: bool) -> Result<()> {
        composer_json_add(dir, package, dev).await
    }

    async fn remove(&self, dir: &Path, package: &str) -> Result<()> {
        composer_json_remove(dir, package).await
    }
}

// ── PyPI: parsing ─────────────────────────────────────────────────────────────

async fn parse_pyproject(dir: &Path) -> Result<ManifestGraph> {
    let content = tokio::fs::read_to_string(dir.join("pyproject.toml"))
        .await
        .context("reading pyproject.toml")?;
    let doc: toml::Value = toml::from_str(&content).context("parsing pyproject.toml")?;
    let mut graph = ManifestGraph::new();

    let pep621 = doc.get("project");
    let poetry = doc.get("tool").and_then(|t| t.get("poetry"));

    let (root_name, root_version, root_desc) = extract_pyproject_root(pep621, poetry);
    let root_ver = make_version(&root_name, &root_version, Ecosystem::PyPI);
    graph.packages.push(PackageNode {
        name: root_name,
        ecosystem: Ecosystem::PyPI,
        description: root_desc,
    });
    graph.versions.push(root_ver.clone());

    push_pep621_deps(&mut graph, &root_ver, pep621);
    push_poetry_deps(&mut graph, &root_ver, poetry);

    Ok(graph)
}

fn extract_pyproject_root(
    pep621: Option<&toml::Value>,
    poetry: Option<&toml::Value>,
) -> (String, String, Option<String>) {
    let section = pep621.or(poetry);
    if let Some(p) = section {
        (
            p.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            p.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string(),
            p.get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
        )
    } else {
        ("unknown".to_string(), "0.0.0".to_string(), None)
    }
}

fn push_pep621_deps(
    graph: &mut ManifestGraph,
    root_ver: &VersionNode,
    pep621: Option<&toml::Value>,
) {
    if let Some(arr) = pep621
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in arr {
            if let Some(spec) = dep.as_str() {
                push_pep508(graph, root_ver, spec, DependencyKind::Normal);
            }
        }
    }

    if let Some(groups) = pep621
        .and_then(|p| p.get("optional-dependencies"))
        .and_then(|d| d.as_table())
    {
        for (_group, deps) in groups {
            if let Some(arr) = deps.as_array() {
                for dep in arr {
                    if let Some(spec) = dep.as_str() {
                        push_pep508(graph, root_ver, spec, DependencyKind::Optional);
                    }
                }
            }
        }
    }
}

fn push_poetry_deps(
    graph: &mut ManifestGraph,
    root_ver: &VersionNode,
    poetry: Option<&toml::Value>,
) {
    if let Some(deps) = poetry
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        for (name, val) in deps {
            if name == "python" {
                continue;
            }
            push_dep_raw(graph, root_ver, name, &poetry_req(val), Ecosystem::PyPI, DependencyKind::Normal);
        }
    }

    if let Some(deps) = poetry
        .and_then(|p| p.get("dev-dependencies"))
        .and_then(|d| d.as_table())
    {
        for (name, val) in deps {
            push_dep_raw(graph, root_ver, name, &poetry_req(val), Ecosystem::PyPI, DependencyKind::Dev);
        }
    }

    // Poetry 1.2+ [tool.poetry.group.*.dependencies]
    if let Some(groups) = poetry
        .and_then(|p| p.get("group"))
        .and_then(|g| g.as_table())
    {
        for (_group_name, group) in groups {
            if let Some(deps) = group.get("dependencies").and_then(|d| d.as_table()) {
                for (name, val) in deps {
                    push_dep_raw(graph, root_ver, name, &poetry_req(val), Ecosystem::PyPI, DependencyKind::Dev);
                }
            }
        }
    }
}

async fn parse_requirements_txt(dir: &Path) -> Result<ManifestGraph> {
    let content = tokio::fs::read_to_string(dir.join("requirements.txt"))
        .await
        .context("reading requirements.txt")?;
    let mut graph = ManifestGraph::new();

    let root_name = dir.file_name().map_or_else(
        || "project".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    let root_ver = make_version(&root_name, "0.0.0", Ecosystem::PyPI);
    graph.packages.push(PackageNode {
        name: root_name,
        ecosystem: Ecosystem::PyPI,
        description: None,
    });
    graph.versions.push(root_ver.clone());

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let line = line.split('#').next().unwrap_or(line).trim();
        if !line.is_empty() {
            push_pep508(&mut graph, &root_ver, line, DependencyKind::Normal);
        }
    }

    Ok(graph)
}

// ── PyPI: manifest editing ────────────────────────────────────────────────────

async fn pyproject_add(dir: &Path, package: &str, dev: bool) -> Result<()> {
    let path = dir.join("pyproject.toml");
    let content = tokio::fs::read_to_string(&path)
        .await
        .context("reading pyproject.toml")?;
    let mut doc = content
        .parse::<DocumentMut>()
        .context("parsing pyproject.toml")?;

    if doc.get("project").is_some() {
        // PEP 621
        if dev {
            if doc["project"]["optional-dependencies"].is_none() {
                doc["project"]["optional-dependencies"] = toml_edit::table();
            }
            if doc["project"]["optional-dependencies"]["dev"].is_none() {
                doc["project"]["optional-dependencies"]["dev"] = toml_edit::array();
            }
            if let Some(arr) = doc["project"]["optional-dependencies"]["dev"].as_array_mut() {
                if !arr.iter().any(|v| {
                    v.as_str()
                        .is_some_and(|s| pep508_name(s) == pep508_name(package))
                }) {
                    arr.push(package);
                }
            }
        } else {
            if doc["project"]["dependencies"].is_none() {
                doc["project"]["dependencies"] = toml_edit::array();
            }
            if let Some(arr) = doc["project"]["dependencies"].as_array_mut() {
                if !arr.iter().any(|v| {
                    v.as_str()
                        .is_some_and(|s| pep508_name(s) == pep508_name(package))
                }) {
                    arr.push(package);
                }
            }
        }
    } else if doc.get("tool").and_then(|t| t.get("poetry")).is_some() {
        // Poetry
        let section = if dev {
            "dev-dependencies"
        } else {
            "dependencies"
        };
        let (name, req) = parse_pep508(package);
        let req_str = if req == "*" { "*" } else { req.as_str() };
        doc["tool"]["poetry"][section][&name] = toml_edit::value(req_str);
    } else {
        anyhow::bail!("unsupported pyproject.toml — no [project] or [tool.poetry] section found");
    }

    tokio::fs::write(&path, doc.to_string())
        .await
        .context("writing pyproject.toml")?;
    Ok(())
}

async fn pyproject_remove(dir: &Path, package: &str) -> Result<()> {
    let path = dir.join("pyproject.toml");
    let content = tokio::fs::read_to_string(&path)
        .await
        .context("reading pyproject.toml")?;
    let mut doc = content
        .parse::<DocumentMut>()
        .context("parsing pyproject.toml")?;
    let target = pep508_name(package);

    if doc.get("project").is_some() {
        // PEP 621 [project.dependencies]
        if let Some(arr) = doc["project"]["dependencies"].as_array_mut() {
            arr.retain(|v| v.as_str().is_none_or(|s| pep508_name(s) != target));
        }
        // PEP 621 [project.optional-dependencies.*]
        let group_keys: Vec<String> = doc["project"]["optional-dependencies"]
            .as_table()
            .map(|t| t.iter().map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default();
        for key in group_keys {
            if let Some(arr) = doc["project"]["optional-dependencies"][&key].as_array_mut() {
                arr.retain(|v| v.as_str().is_none_or(|s| pep508_name(s) != target));
            }
        }
    } else if doc.get("tool").and_then(|t| t.get("poetry")).is_some() {
        for section in &["dependencies", "dev-dependencies"] {
            if let Some(table) = doc["tool"]["poetry"][section].as_table_mut() {
                table.remove(&target);
            }
        }
    }

    tokio::fs::write(&path, doc.to_string())
        .await
        .context("writing pyproject.toml")?;
    Ok(())
}

async fn requirements_txt_add(dir: &Path, package: &str) -> Result<()> {
    let path = dir.join("requirements.txt");
    let mut content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let target = pep508_name(package);
    let already_present = content.lines().any(|l| {
        let l = l.split('#').next().unwrap_or(l).trim();
        !l.is_empty() && pep508_name(l) == target
    });
    if !already_present {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(package);
        content.push('\n');
        tokio::fs::write(&path, content)
            .await
            .context("writing requirements.txt")?;
    }
    Ok(())
}

async fn requirements_txt_remove(dir: &Path, package: &str) -> Result<()> {
    let path = dir.join("requirements.txt");
    let content = tokio::fs::read_to_string(&path)
        .await
        .context("reading requirements.txt")?;
    let target = pep508_name(package);
    let filtered = content
        .lines()
        .filter(|l| {
            let stripped = l.split('#').next().unwrap_or(l).trim();
            stripped.is_empty()
                || l.trim_start().starts_with('#')
                || pep508_name(stripped) != target
        })
        .fold(String::new(), |mut acc, l| {
            acc.push_str(l);
            acc.push('\n');
            acc
        });
    tokio::fs::write(&path, filtered)
        .await
        .context("writing requirements.txt")?;
    Ok(())
}

// ── Composer: parsing ─────────────────────────────────────────────────────────

async fn parse_composer_json(dir: &Path) -> Result<ManifestGraph> {
    let content = tokio::fs::read_to_string(dir.join("composer.json"))
        .await
        .context("reading composer.json")?;
    let json: serde_json::Value =
        serde_json::from_str(&content).context("parsing composer.json")?;
    let mut graph = ManifestGraph::new();

    let root_name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let root_version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");

    let root_ver = make_version(root_name, root_version, Ecosystem::Composer);
    graph.packages.push(PackageNode {
        name: root_name.to_string(),
        ecosystem: Ecosystem::Composer,
        description: json
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
    });
    graph.versions.push(root_ver.clone());

    for (section, kind) in &[
        ("require", DependencyKind::Normal),
        ("require-dev", DependencyKind::Dev),
    ] {
        if let Some(obj) = json.get(*section).and_then(|v| v.as_object()) {
            for (pkg_name, constraint) in obj {
                // Skip PHP platform requirements
                if pkg_name == "php" || pkg_name.starts_with("ext-") || pkg_name.starts_with("lib-")
                {
                    continue;
                }
                let req = constraint.as_str().unwrap_or("*").to_string();
                push_dep_raw(
                    &mut graph,
                    &root_ver,
                    pkg_name,
                    &req,
                    Ecosystem::Composer,
                    kind.clone(),
                );
            }
        }
    }

    Ok(graph)
}

// ── Composer: manifest editing ────────────────────────────────────────────────

async fn composer_json_add(dir: &Path, package: &str, dev: bool) -> Result<()> {
    let path = dir.join("composer.json");
    let content = tokio::fs::read_to_string(&path)
        .await
        .context("reading composer.json")?;
    let mut json: serde_json::Value =
        serde_json::from_str(&content).context("parsing composer.json")?;

    let section = if dev { "require-dev" } else { "require" };
    // Accept "vendor/package:^1.0" or just "vendor/package"
    let (name, req) = package.split_once(':').map_or_else(
        || (package.to_string(), "*".to_string()),
        |(n, v)| (n.trim().to_string(), v.trim().to_string()),
    );

    json.as_object_mut()
        .context("composer.json root is not a JSON object")?
        .entry(section)
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("require section is not a JSON object")?
        .insert(name, serde_json::Value::String(req));

    tokio::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&json)?))
        .await
        .context("writing composer.json")?;
    Ok(())
}

async fn composer_json_remove(dir: &Path, package: &str) -> Result<()> {
    let path = dir.join("composer.json");
    let content = tokio::fs::read_to_string(&path)
        .await
        .context("reading composer.json")?;
    let mut json: serde_json::Value =
        serde_json::from_str(&content).context("parsing composer.json")?;

    if let Some(obj) = json.as_object_mut() {
        for section in &["require", "require-dev"] {
            if let Some(req) = obj.get_mut(*section).and_then(|v| v.as_object_mut()) {
                req.remove(package);
            }
        }
    }

    tokio::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&json)?))
        .await
        .context("writing composer.json")?;
    Ok(())
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Parse a PEP 508 specifier into (normalized_name, version_req).
fn parse_pep508(spec: &str) -> (String, String) {
    // Strip environment markers (everything after ';')
    let spec = spec.split(';').next().unwrap_or(spec).trim();
    // Name ends at the first version operator, extra bracket, or whitespace
    let name_end = spec
        .find(['>', '<', '=', '!', '~', '[', ' '])
        .unwrap_or(spec.len());
    let name = pep503_normalize(spec[..name_end].trim());
    let rest = spec[name_end..].trim();
    let version_req = if rest.starts_with('[') {
        // Skip past the extras bracket
        rest.find(']')
            .map(|i| rest[i + 1..].trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "*".to_string())
    } else if rest.is_empty() {
        "*".to_string()
    } else {
        rest.to_string()
    };
    (name, version_req)
}

/// Extract just the normalized name from a PEP 508 specifier.
fn pep508_name(spec: &str) -> String {
    parse_pep508(spec).0
}

/// PEP 503 name normalization: lowercase + collapse runs of [-_.] to '-'.
fn pep503_normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_sep = false;
    for c in name.to_lowercase().chars() {
        if matches!(c, '-' | '_' | '.') {
            if !prev_sep {
                out.push('-');
                prev_sep = true;
            }
        } else {
            out.push(c);
            prev_sep = false;
        }
    }
    out
}

/// Extract a version constraint from a Poetry dependency value.
fn poetry_req(val: &toml::Value) -> String {
    match val {
        toml::Value::String(v) => v.clone(),
        toml::Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        _ => "*".to_string(),
    }
}

fn make_version(name: &str, semver: &str, ecosystem: Ecosystem) -> VersionNode {
    VersionNode {
        name: name.to_string(),
        ecosystem,
        semver: semver.to_string(),
        published_at: None,
        checksum: None,
        yanked: false,
    }
}

fn push_pep508(graph: &mut ManifestGraph, root: &VersionNode, spec: &str, kind: DependencyKind) {
    let (name, req) = parse_pep508(spec);
    push_dep_raw(graph, root, &name, &req, Ecosystem::PyPI, kind);
}

fn push_dep_raw(
    graph: &mut ManifestGraph,
    root: &VersionNode,
    name: &str,
    version_req: &str,
    ecosystem: Ecosystem,
    kind: DependencyKind,
) {
    let dep_ver = make_version(name, version_req, ecosystem.clone());
    graph.packages.push(PackageNode {
        name: name.to_string(),
        ecosystem,
        description: None,
    });
    graph.depends_on.push(DependsOnEdge {
        from_id: root.id(),
        to_id: dep_ver.id(),
        kind,
        version_req: version_req.to_string(),
    });
    graph.versions.push(dep_ver);
}
