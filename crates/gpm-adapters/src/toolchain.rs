use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryEntry {
    pub path: PathBuf,
    pub version: String,
    /// true = downloaded and managed by gpm
    pub managed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ToolConfig {
    pub active: Option<PathBuf>,
    pub entries: Vec<BinaryEntry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ToolRegistry {
    #[serde(default)]
    tools: HashMap<String, ToolConfig>,
}

pub struct ToolchainManager {
    home: PathBuf,
    registry: ToolRegistry,
}

// ── ToolchainManager ──────────────────────────────────────────────────────────

impl ToolchainManager {
    pub fn gpm_home() -> PathBuf {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_or_else(|_| PathBuf::from("."), PathBuf::from)
            .join(".gpm")
    }

    pub fn load() -> Result<Self> {
        let home = Self::gpm_home();
        let path = home.join("toolchain.toml");
        let registry = if path.exists() {
            let content =
                std::fs::read_to_string(&path).context("reading ~/.gpm/toolchain.toml")?;
            toml::from_str(&content).context("parsing ~/.gpm/toolchain.toml")?
        } else {
            ToolRegistry::default()
        };
        Ok(Self { home, registry })
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.home)?;
        let content = toml::to_string_pretty(&self.registry)?;
        std::fs::write(self.home.join("toolchain.toml"), content)?;
        Ok(())
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.home.join("bin")
    }

    /// Return the path to `tool`, discovering, prompting, or installing as needed.
    pub async fn resolve(&mut self, tool: &str) -> Result<PathBuf> {
        // Return remembered active path if it still exists on disk
        if let Some(active) = self
            .registry
            .tools
            .get(tool)
            .and_then(|c| c.active.as_ref())
        {
            if active.exists() {
                return Ok(active.clone());
            }
        }
        // Stale active path — clear it and re-discover
        if let Some(c) = self.registry.tools.get_mut(tool) {
            c.active = None;
        }

        let found = discover(tool, &self.bin_dir());

        let active_path: PathBuf = if found.is_empty() {
            eprintln!("\n`{tool}` not found on this system.");
            match install_interactive(tool, &self.bin_dir()).await? {
                Some(entry) => {
                    let path = entry.path.clone();
                    let config = self.registry.tools.entry(tool.to_string()).or_default();
                    config.active = Some(path.clone());
                    config.entries.push(entry);
                    self.save()?;
                    return Ok(path);
                }
                None => bail!(
                    "`{tool}` is required but not installed.\n\
                     Install it manually and re-run, or run `gpm toolchain reset {tool}` to be prompted again."
                ),
            }
        } else if found.len() == 1 {
            eprintln!("Using {tool}: {}", found[0].path.display());
            found[0].path.clone()
        } else {
            eprintln!("\nMultiple versions of `{tool}` found:");
            for (i, e) in found.iter().enumerate() {
                let tag = if e.managed { " [gpm]" } else { " [system]" };
                eprintln!("  [{}] {} ({}){}", i, e.path.display(), e.version, tag);
            }
            let idx = prompt_index("Which would you like to use? [0]: ", found.len())?;
            found[idx].path.clone()
        };

        let config = self.registry.tools.entry(tool.to_string()).or_default();
        config.active = Some(active_path.clone());
        for e in &found {
            if !config.entries.iter().any(|x| x.path == e.path) {
                config.entries.push(e.clone());
            }
        }
        self.save()?;
        Ok(active_path)
    }
}

// ── Discovery ─────────────────────────────────────────────────────────────────

fn discover(tool: &str, gpm_bin: &Path) -> Vec<BinaryEntry> {
    let mut entries: Vec<BinaryEntry> = Vec::new();

    // gpm-managed bin dir first
    if gpm_bin.exists() {
        if let Ok(rd) = std::fs::read_dir(gpm_bin) {
            for de in rd.flatten() {
                let p = de.path();
                if is_match(tool, &p) {
                    let version = probe_version(&p).unwrap_or_else(|| "unknown".into());
                    entries.push(BinaryEntry {
                        path: p,
                        version,
                        managed: true,
                    });
                }
            }
        }
    }

    // System PATH
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(sep) {
        let dir = PathBuf::from(dir);
        if dir == gpm_bin {
            continue;
        }
        for candidate in system_candidates(tool, &dir) {
            if candidate.exists() && !entries.iter().any(|e| e.path == candidate) {
                let version = probe_version(&candidate).unwrap_or_else(|| "unknown".into());
                entries.push(BinaryEntry {
                    path: candidate,
                    version,
                    managed: false,
                });
            }
        }
    }

    entries
}

fn is_match(tool: &str, path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    name == tool
        || name == format!("{tool}.exe")
        || name == format!("{tool}.phar")
        || name.starts_with(&format!("{tool}-"))
}

fn system_candidates(tool: &str, dir: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![dir.join(format!("{tool}.exe")), dir.join(tool)]
    } else {
        vec![dir.join(tool), dir.join(format!("{tool}.phar"))]
    }
}

fn probe_version(binary: &Path) -> Option<String> {
    let out = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    // E.g. "uv 0.7.0 (abc123 2025-05-10)" → strip binary name prefix
    let cleaned = first
        .trim_start_matches(|c: char| c.is_alphabetic() || c == '-' || c == '_' || c == ' ')
        .trim();
    Some(if cleaned.is_empty() {
        first.to_string()
    } else {
        cleaned.to_string()
    })
}

// ── Interactive helpers ───────────────────────────────────────────────────────

fn prompt_input(label: &str) -> Result<String> {
    eprint!("{label}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn prompt_yes_no(label: &str) -> Result<bool> {
    let answer = prompt_input(label)?;
    Ok(matches!(answer.to_lowercase().as_str(), "y" | "yes" | ""))
}

fn prompt_index(label: &str, max: usize) -> Result<usize> {
    let answer = prompt_input(label)?;
    if answer.is_empty() {
        return Ok(0);
    }
    let idx: usize = answer.parse().context("expected a number")?;
    if idx >= max {
        bail!("index {idx} out of range (0..{max})");
    }
    Ok(idx)
}

// ── Installation ──────────────────────────────────────────────────────────────

async fn install_interactive(tool: &str, bin_dir: &Path) -> Result<Option<BinaryEntry>> {
    match tool {
        "uv" | "composer" => {
            let version_hint = if tool == "uv" { "latest" } else { "stable" };
            if !prompt_yes_no(&format!(
                "Install `{tool}` ({version_hint}) managed by gpm? [Y/n]: "
            ))? {
                return Ok(None);
            }
            let version_input =
                prompt_input(&format!("Version (leave blank for {version_hint}): "))?;
            let version = if version_input.is_empty() {
                None
            } else {
                Some(version_input)
            };
            let entry = do_install(tool, version.as_deref(), bin_dir).await?;
            Ok(Some(entry))
        }
        _ => {
            eprintln!("Automatic installation of `{tool}` is not supported.");
            eprintln!("{}", manual_install_hint(tool));
            Ok(None)
        }
    }
}

fn manual_install_hint(tool: &str) -> String {
    match tool {
        "npm" => "→ Install Node.js (includes npm): https://nodejs.org/".into(),
        "cargo" => "→ Install Rust (includes cargo): https://rustup.rs/".into(),
        "pip" => "→ Install Python (includes pip): https://python.org/".into(),
        "pip3" => "→ Install Python 3: https://python.org/".into(),
        _ => format!("→ Install `{tool}` from your system package manager or official website."),
    }
}

async fn do_install(tool: &str, version: Option<&str>, bin_dir: &Path) -> Result<BinaryEntry> {
    std::fs::create_dir_all(bin_dir)?;
    match tool {
        "uv" => install_uv(version, bin_dir).await,
        "composer" => install_composer(bin_dir).await,
        _ => bail!("No installer for `{tool}`"),
    }
}

// ── uv installer ──────────────────────────────────────────────────────────────

async fn install_uv(version: Option<&str>, bin_dir: &Path) -> Result<BinaryEntry> {
    let resolved = match version {
        Some(v) => v.trim_start_matches('v').to_string(),
        None => fetch_latest_github_tag("astral-sh", "uv").await?,
    };
    eprintln!("Downloading uv v{resolved}...");

    let (archive_url, bin_name) = uv_release_url(&resolved);
    let bytes = download_bytes(&archive_url).await?;
    let dest = bin_dir.join(&bin_name);

    if archive_url.ends_with(".tar.gz") {
        extract_first_from_tar_gz(&bytes, &bin_name, &dest)?;
    } else {
        // .zip on Windows — write archive to temp and call Expand-Archive
        extract_from_zip_via_ps(&bytes, &bin_name, &dest)?;
    }

    make_executable(&dest)?;
    let ver = probe_version(&dest).unwrap_or(resolved);
    eprintln!("  installed to {}", dest.display());
    Ok(BinaryEntry {
        path: dest,
        version: ver,
        managed: true,
    })
}

fn uv_release_url(version: &str) -> (String, String) {
    let (target, ext) = if cfg!(target_os = "windows") {
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64"
        };
        (format!("{arch}-pc-windows-msvc"), "zip")
    } else if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64"
        };
        (format!("{arch}-apple-darwin"), "tar.gz")
    } else {
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64"
        };
        (format!("{arch}-unknown-linux-musl"), "tar.gz")
    };

    let bin_name = if cfg!(windows) { "uv.exe" } else { "uv" }.to_string();
    let url =
        format!("https://github.com/astral-sh/uv/releases/download/v{version}/uv-{target}.{ext}");
    (url, bin_name)
}

fn extract_first_from_tar_gz(bytes: &[u8], bin_name: &str, dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| f == bin_name)
        {
            entry.unpack(dest)?;
            return Ok(());
        }
    }
    bail!("binary `{bin_name}` not found in archive")
}

fn extract_from_zip_via_ps(bytes: &[u8], bin_name: &str, dest: &Path) -> Result<()> {
    // Write zip to a temp file, run PowerShell Expand-Archive, copy binary out
    let tmp_dir = std::env::temp_dir().join(format!(
        "gpm-install-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::create_dir_all(&tmp_dir)?;
    let zip_path = tmp_dir.join("archive.zip");
    let extract_dir = tmp_dir.join("extracted");
    std::fs::write(&zip_path, bytes)?;

    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                zip_path.display(),
                extract_dir.display()
            ),
        ])
        .status()
        .context("running PowerShell Expand-Archive")?;
    anyhow::ensure!(status.success(), "Expand-Archive failed");

    // Find the binary in the extracted tree
    let found = find_file_in_dir(&extract_dir, bin_name)?;
    std::fs::copy(&found, dest)
        .with_context(|| format!("copying {} to {}", found.display(), dest.display()))?;

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}

fn find_file_in_dir(dir: &Path, name: &str) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Ok(found) = find_file_in_dir(&p, name) {
                return Ok(found);
            }
        } else if p.file_name().and_then(|f| f.to_str()) == Some(name) {
            return Ok(p);
        }
    }
    bail!("`{name}` not found under {}", dir.display())
}

// ── Composer installer ────────────────────────────────────────────────────────

async fn install_composer(bin_dir: &Path) -> Result<BinaryEntry> {
    eprintln!("Downloading Composer (stable)...");
    let bytes = download_bytes("https://getcomposer.org/composer-stable.phar").await?;
    let dest = bin_dir.join("composer.phar");
    std::fs::write(&dest, &bytes)?;
    make_executable(&dest)?;
    let version = probe_version(&dest).unwrap_or_else(|| "stable".into());
    eprintln!("  installed to {}", dest.display());
    Ok(BinaryEntry {
        path: dest,
        version,
        managed: true,
    })
}

// ── HTTP ──────────────────────────────────────────────────────────────────────

async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("gpm/0.1 (https://github.com/4Gettt25/gpm)")
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    anyhow::ensure!(
        resp.status().is_success(),
        "GET {url} returned {}",
        resp.status()
    );
    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

async fn fetch_latest_github_tag(owner: &str, repo: &str) -> Result<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("gpm/0.1 (https://github.com/4Gettt25/gpm)")
        .build()?;
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await?
        .json()
        .await
        .context("parsing GitHub releases JSON")?;
    let tag = resp["tag_name"]
        .as_str()
        .context("no tag_name in GitHub releases response")?;
    Ok(tag.trim_start_matches('v').to_string())
}

// ── Platform helpers ──────────────────────────────────────────────────────────

#[allow(clippy::unnecessary_wraps)] // Result needed for the Unix cfg path
fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
