use anyhow::{Context, Result};

use crate::cmd::style;

const GITHUB_REPO: &str = "lee-to/hlv";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Determine the expected asset name for the current platform.
fn asset_name() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("hlv-aarch64-apple-darwin.tar.gz"),
        ("macos", "x86_64") => Ok("hlv-x86_64-apple-darwin.tar.gz"),
        ("linux", "aarch64") => Ok("hlv-aarch64-unknown-linux-gnu.tar.gz"),
        ("linux", "x86_64") => Ok("hlv-x86_64-unknown-linux-gnu.tar.gz"),
        ("windows", "x86_64") => Ok("hlv-x86_64-pc-windows-msvc.zip"),
        (os, arch) => anyhow::bail!("Unsupported platform: {os}/{arch}"),
    }
}

/// Strip leading 'v' from tag_name and compare with current.
fn is_newer(tag: &str) -> bool {
    let remote = tag.strip_prefix('v').unwrap_or(tag);
    version_cmp(remote, CURRENT_VERSION) == std::cmp::Ordering::Greater
}

/// Simple semver comparison (major.minor.patch).
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse =
        |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect() };
    parse(a).cmp(&parse(b))
}

fn build_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("hlv-updater")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Cannot build HTTP client")
}

/// Download bytes from a URL.
fn download(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to download: {url}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }
    Ok(resp.bytes()?.to_vec())
}

/// Extract the `hlv` binary from a tar.gz archive, return the binary bytes.
fn extract_from_tar_gz(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.file_name().and_then(|n| n.to_str()) == Some("hlv") {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf)?;
            return Ok(buf);
        }
    }
    anyhow::bail!("Binary 'hlv' not found in archive")
}

/// Extract the `hlv.exe` binary from a zip archive, return the binary bytes.
fn extract_from_zip(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.ends_with("hlv.exe") || name.ends_with("hlv") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    anyhow::bail!("Binary 'hlv' not found in archive")
}

/// Replace the current executable with new binary bytes.
fn self_replace(new_binary: &[u8]) -> Result<()> {
    let current_exe = std::env::current_exe()
        .context("Cannot determine current executable path")?
        .canonicalize()?;

    let dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?;

    let tmp_path = dir.join(".hlv-update-tmp");
    let backup_path = dir.join(".hlv-update-backup");

    // Write new binary to temp
    std::fs::write(&tmp_path, new_binary)
        .with_context(|| format!("Cannot write to {}", tmp_path.display()))?;

    // Set executable permission on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Backup current → backup, tmp → current
    if backup_path.exists() {
        std::fs::remove_file(&backup_path).ok();
    }
    std::fs::rename(&current_exe, &backup_path).with_context(|| "Cannot backup current binary")?;
    if let Err(e) = std::fs::rename(&tmp_path, &current_exe) {
        // Restore backup on failure
        std::fs::rename(&backup_path, &current_exe).ok();
        return Err(e).context("Cannot replace binary");
    }
    std::fs::remove_file(&backup_path).ok();

    Ok(())
}

/// `hlv update [--check]`
pub fn run(check_only: bool) -> Result<()> {
    style::header("update");
    style::detail("current", CURRENT_VERSION);

    // Fetch latest release
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = build_client()?;
    let resp = client.get(&url).send().context("Cannot reach GitHub API")?;

    if !resp.status().is_success() {
        anyhow::bail!("GitHub API error: HTTP {}", resp.status());
    }

    let release: Release = resp.json().context("Cannot parse release info")?;
    let remote_version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);

    style::detail("latest", remote_version);

    if !is_newer(&release.tag_name) {
        style::ok("Already up to date");
        return Ok(());
    }

    if check_only {
        style::warn(&format!(
            "Update available: {CURRENT_VERSION} → {remote_version}"
        ));
        style::hint("Run 'hlv update' to install");
        return Ok(());
    }

    // Find the right asset
    let expected = asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == expected)
        .ok_or_else(|| anyhow::anyhow!("No asset '{expected}' in release {}", release.tag_name))?;

    style::detail("platform", expected);
    println!();
    style::hint(&format!("Downloading {}...", release.tag_name));

    let data = download(&client, &asset.browser_download_url)?;

    // Extract binary
    let binary = if expected.ends_with(".tar.gz") {
        extract_from_tar_gz(&data)?
    } else {
        extract_from_zip(&data)?
    };

    // Replace current executable
    let exe_path = std::env::current_exe()?.canonicalize()?;
    style::hint(&format!("Replacing {}...", exe_path.display()));
    self_replace(&binary)?;

    style::ok(&format!("Updated to {remote_version}"));
    Ok(())
}
