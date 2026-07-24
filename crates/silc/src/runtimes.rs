//! Silc-owned runtime provisioning.
//!
//! Bun, Go, and CPython (python-build-standalone) are downloaded into
//! `{cache_root}/runtimes/<os>-<arch>/{bun,cpython,go}/<version>/`.
//!
//! The cache root is fixed at `~/.silc`; there are no project, CLI, PATH, or
//! environment overrides for engine selection or placement.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive;

pub const BUN_VERSION: &str = "1.2.18";
pub const GO_VERSION: &str = "1.23.6";
pub const CPYTHON_VERSION: &str = "3.12.12+20251217";
const CPYTHON_RELEASE_TAG: &str = "20251217";

const PYTHON_BUILD_STANDALONE_BASE: &str =
    "https://github.com/astral-sh/python-build-standalone/releases/download";
const GO_DOWNLOAD_BASE: &str = "https://go.dev/dl";
const BUN_DOWNLOAD_BASE: &str = "https://github.com/oven-sh/bun/releases/download";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeLock {
    pub platform: String,
    pub bun_version: String,
    pub python_version: String,
    pub go_version: String,
    pub bun_bin: PathBuf,
    pub python_bin: PathBuf,
    pub go_bin: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Platform {
    id: &'static str,
    bun_os: &'static str,
    bun_arch: &'static str,
    go_suffix: &'static str,
    python_triple: &'static str,
}

impl Platform {
    fn detect() -> Result<Self, String> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Ok(Self {
                id: "darwin-aarch64",
                bun_os: "darwin",
                bun_arch: "aarch64",
                go_suffix: "darwin-arm64",
                python_triple: "aarch64-apple-darwin",
            }),
            ("macos", "x86_64") => Ok(Self {
                id: "darwin-x64",
                bun_os: "darwin",
                bun_arch: "x64",
                go_suffix: "darwin-amd64",
                python_triple: "x86_64-apple-darwin",
            }),
            ("linux", "aarch64") => Ok(Self {
                id: "linux-aarch64",
                bun_os: "linux",
                bun_arch: "aarch64",
                go_suffix: "linux-arm64",
                python_triple: "aarch64-unknown-linux-gnu",
            }),
            ("linux", "x86_64") => Ok(Self {
                id: "linux-x64",
                bun_os: "linux",
                bun_arch: "x64",
                go_suffix: "linux-amd64",
                python_triple: "x86_64-unknown-linux-gnu",
            }),
            (os, arch) => Err(format!(
                "unsupported platform for Silc runtime provisioning: {os}-{arch}"
            )),
        }
    }

    fn bun_zip_name(&self) -> String {
        format!("bun-{}-{}.zip", self.bun_os, self.bun_arch)
    }

    fn bun_dir_name(&self) -> String {
        format!("bun-{}-{}", self.bun_os, self.bun_arch)
    }

    fn go_archive_name(&self) -> String {
        format!("go{GO_VERSION}.{}.tar.gz", self.go_suffix)
    }

    fn cpython_archive_name(&self) -> String {
        format!(
            "cpython-{CPYTHON_VERSION}-{triple}-install_only_stripped.tar.gz",
            triple = self.python_triple
        )
    }
}

/// Fixed cache root for Silc-owned engines.
///
/// There is deliberately no environment, project, PATH, or CLI override.
pub fn cache_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".silc"))
        .ok_or_else(|| "failed to resolve home directory for Silc cache (~/.silc)".to_string())
}

pub fn runtimes_root() -> Result<PathBuf, String> {
    let platform = Platform::detect()?;
    Ok(cache_root()?.join("runtimes").join(platform.id))
}

pub fn ensure_runtimes() -> Result<RuntimeLock, String> {
    let platform = Platform::detect()?;
    let root = runtimes_root()?;

    let bun_bin = ensure_bun(&root, platform)?;
    let python_bin = ensure_cpython(&root, platform)?;
    let go_bin = ensure_go(&root, platform)?;

    Ok(RuntimeLock {
        platform: platform.id.to_string(),
        bun_version: BUN_VERSION.to_string(),
        python_version: CPYTHON_VERSION.to_string(),
        go_version: GO_VERSION.to_string(),
        bun_bin,
        python_bin,
        go_bin,
    })
}

pub fn write_lock(workdir: &Path, lock: &RuntimeLock) -> Result<(), String> {
    let dir = workdir.join(".silc");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    let path = dir.join("runtimes.lock.json");
    let json = serde_json::to_string_pretty(lock)
        .map_err(|error| format!("failed to serialize runtimes.lock.json: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[allow(dead_code)]
pub fn read_lock(workdir: &Path) -> Result<RuntimeLock, String> {
    let path = workdir.join(".silc").join("runtimes.lock.json");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn ensure_bun(root: &Path, platform: Platform) -> Result<PathBuf, String> {
    let install_dir = root.join("bun").join(BUN_VERSION);
    let bin = install_dir.join(platform.bun_dir_name()).join("bun");
    if bin.is_file() {
        verify_bun(&bin)?;
        return Ok(bin.canonicalize().unwrap_or(bin));
    }

    fs::create_dir_all(&install_dir).map_err(|error| {
        format!(
            "failed to provision Silc Bun: failed to create {}: {error}",
            install_dir.display()
        )
    })?;

    let zip_name = platform.bun_zip_name();
    let url = format!("{BUN_DOWNLOAD_BASE}/bun-v{BUN_VERSION}/{zip_name}");
    let archive = install_dir.join(&zip_name);
    download_url(&url, &archive, None, "Silc Bun")?;

    extract_zip_with_unzip(&archive, &install_dir, "Silc Bun")?;
    let _ = fs::remove_file(&archive);

    if !bin.is_file() {
        return Err(format!(
            "failed to provision Silc Bun: expected binary at {} after extract",
            bin.display()
        ));
    }
    make_executable(&bin)?;
    verify_bun(&bin)?;
    Ok(bin.canonicalize().unwrap_or(bin))
}

fn ensure_cpython(root: &Path, platform: Platform) -> Result<PathBuf, String> {
    let install_dir = root.join("cpython").join(CPYTHON_VERSION);
    let bin = install_dir.join("python").join("bin").join("python3");
    if bin.is_file() {
        verify_cpython(&bin)?;
        return Ok(bin.canonicalize().unwrap_or(bin));
    }

    fs::create_dir_all(&install_dir).map_err(|error| {
        format!(
            "failed to provision Silc CPython: failed to create {}: {error}",
            install_dir.display()
        )
    })?;

    let archive_name = platform.cpython_archive_name();
    let url = format!("{PYTHON_BUILD_STANDALONE_BASE}/{CPYTHON_RELEASE_TAG}/{archive_name}");
    let archive = install_dir.join(&archive_name);
    let expected_sha256 = fetch_cpython_sha256(&archive_name)?;
    download_url(&url, &archive, Some(&expected_sha256), "Silc CPython")?;

    extract_tar_gz(&archive, &install_dir, "Silc CPython")?;
    let _ = fs::remove_file(&archive);

    if !bin.is_file() {
        return Err(format!(
            "failed to provision Silc CPython: expected binary at {} after extract",
            bin.display()
        ));
    }
    make_executable(&bin)?;
    verify_cpython(&bin)?;
    Ok(bin.canonicalize().unwrap_or(bin))
}

fn ensure_go(root: &Path, platform: Platform) -> Result<PathBuf, String> {
    let install_dir = root.join("go").join(GO_VERSION);
    let bin = install_dir.join("go").join("bin").join("go");
    if bin.is_file() {
        verify_go(&bin)?;
        return Ok(bin.canonicalize().unwrap_or(bin));
    }

    fs::create_dir_all(&install_dir).map_err(|error| {
        format!(
            "failed to provision Silc Go: failed to create {}: {error}",
            install_dir.display()
        )
    })?;

    let archive_name = platform.go_archive_name();
    let url = format!("{GO_DOWNLOAD_BASE}/{archive_name}");
    let archive = install_dir.join(&archive_name);
    let expected_sha256 = fetch_go_sha256(&archive_name)?;
    download_url(&url, &archive, Some(&expected_sha256), "Silc Go")?;

    extract_tar_gz(&archive, &install_dir, "Silc Go")?;
    let _ = fs::remove_file(&archive);

    if !bin.is_file() {
        return Err(format!(
            "failed to provision Silc Go: expected binary at {} after extract",
            bin.display()
        ));
    }
    make_executable(&bin)?;
    verify_go(&bin)?;
    Ok(bin.canonicalize().unwrap_or(bin))
}

fn fetch_cpython_sha256(archive_name: &str) -> Result<String, String> {
    let url = format!("{PYTHON_BUILD_STANDALONE_BASE}/{CPYTHON_RELEASE_TAG}/SHA256SUMS");
    let body = fetch_text(&url, "Silc CPython checksum")?;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (hash, name) = line
            .split_once("  ")
            .or_else(|| line.split_once('\t'))
            .ok_or_else(|| {
                format!("failed to provision Silc CPython: malformed SHA256SUMS line: {line}")
            })?;
        if name.trim() == archive_name {
            return Ok(hash.trim().to_ascii_lowercase());
        }
    }
    Err(format!(
        "failed to provision Silc CPython: SHA256SUMS has no entry for {archive_name}"
    ))
}

fn fetch_go_sha256(archive_name: &str) -> Result<String, String> {
    let url = format!("https://dl.google.com/go/{archive_name}.sha256");
    let body = fetch_text(&url, "Silc Go checksum")?;
    Ok(body.trim().to_ascii_lowercase())
}

fn fetch_text(url: &str, label: &str) -> Result<String, String> {
    ureq::get(url)
        .call()
        .map_err(|error| format!("failed to provision {label}: GET {url}: {error}"))?
        .into_string()
        .map_err(|error| format!("failed to provision {label}: read {url}: {error}"))
}

fn download_url(
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
    label: &str,
) -> Result<(), String> {
    let response = ureq::get(url).call().map_err(|error| {
        format!(
            "failed to provision {label}: download {url} -> {}: {error}",
            dest.display()
        )
    })?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to provision {label}: read download body: {error}"))?;

    if let Some(expected) = expected_sha256 {
        verify_sha256_bytes(&bytes, expected, label, url)?;
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to provision {label}: create {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(dest, &bytes).map_err(|error| {
        format!(
            "failed to provision {label}: write {}: {error}",
            dest.display()
        )
    })
}

fn verify_sha256_bytes(
    data: &[u8],
    expected: &str,
    label: &str,
    context: &str,
) -> Result<(), String> {
    let digest = Sha256::digest(data);
    let actual = hex::encode(digest);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "failed to provision {label}: sha256 mismatch for {context}: expected {expected}, got {actual}"
        ))
    }
}

fn extract_tar_gz(archive: &Path, dest: &Path, label: &str) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| {
        format!(
            "failed to provision {label}: open {}: {error}",
            archive.display()
        )
    })?;
    let decoder = GzDecoder::new(file);
    let mut tar = Archive::new(decoder);
    tar.unpack(dest).map_err(|error| {
        format!(
            "failed to provision {label}: extract {} -> {}: {error}",
            archive.display(),
            dest.display()
        )
    })
}

fn extract_zip_with_unzip(archive: &Path, dest: &Path, label: &str) -> Result<(), String> {
    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(archive)
        .arg("-d")
        .arg(dest)
        .status()
        .map_err(|error| {
            format!(
                "failed to provision {label}: unzip {} -> {}: {error}",
                archive.display(),
                dest.display()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to provision {label}: unzip exited with {status} for {}",
            archive.display()
        ))
    }
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path)
            .map_err(|error| format!("failed to chmod {}: {error}", path.display()))?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|error| format!("failed to chmod {}: {error}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn verify_bun(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to run Silc Bun at {}: {error}", bin.display()))?;
    if !output.status.success() {
        return Err(format!(
            "failed to verify Silc Bun at {}: exited with {}",
            bin.display(),
            output.status
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !version.contains(BUN_VERSION) {
        return Err(format!(
            "failed to verify Silc Bun at {}: expected version {BUN_VERSION}, got {version}",
            bin.display()
        ));
    }
    Ok(())
}

fn verify_cpython(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to run Silc CPython at {}: {error}", bin.display()))?;
    if !output.status.success() {
        return Err(format!(
            "failed to verify Silc CPython at {}: exited with {}",
            bin.display(),
            output.status
        ));
    }
    let version = String::from_utf8_lossy(if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    })
    .trim()
    .to_string();
    if !version.contains("3.12.12") {
        return Err(format!(
            "failed to verify Silc CPython at {}: expected 3.12.12, got {version}",
            bin.display()
        ));
    }
    Ok(())
}

fn verify_go(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .arg("version")
        .output()
        .map_err(|error| format!("failed to run Silc Go at {}: {error}", bin.display()))?;
    if !output.status.success() {
        return Err(format!(
            "failed to verify Silc Go at {}: exited with {}",
            bin.display(),
            output.status
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !version.contains(GO_VERSION) {
        return Err(format!(
            "failed to verify Silc Go at {}: expected go{GO_VERSION}, got {version}",
            bin.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_silc_home(prefix: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "silc-runtimes-{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn cache_root_defaults_to_home_silc() {
        let expected = dirs::home_dir().unwrap().join(".silc");
        assert_eq!(cache_root().unwrap(), expected);
    }

    #[test]
    fn write_and_read_lock_roundtrip() {
        let workdir = temp_silc_home("lock-roundtrip");
        let lock = RuntimeLock {
            platform: "darwin-aarch64".to_string(),
            bun_version: BUN_VERSION.to_string(),
            python_version: CPYTHON_VERSION.to_string(),
            go_version: GO_VERSION.to_string(),
            bun_bin: workdir.join("bun"),
            python_bin: workdir.join("python3"),
            go_bin: workdir.join("go"),
        };
        write_lock(&workdir, &lock).expect("write lock");
        let loaded = read_lock(&workdir).expect("read lock");
        assert_eq!(loaded, lock);
        let _ = fs::remove_dir_all(&workdir);
    }

    #[test]
    #[ignore = "downloads pinned engines; exercised by runtime CI build and e2e"]
    fn ensure_runtimes_provisions_into_silc_home() {
        let lock = ensure_runtimes().expect("ensure_runtimes");
        assert_eq!(lock.bun_version, BUN_VERSION);
        assert_eq!(lock.python_version, CPYTHON_VERSION);
        assert_eq!(lock.go_version, GO_VERSION);
        assert!(
            lock.bun_bin.is_file(),
            "bun missing at {}",
            lock.bun_bin.display()
        );
        assert!(
            lock.python_bin.is_file(),
            "python missing at {}",
            lock.python_bin.display()
        );
        assert!(
            lock.go_bin.is_file(),
            "go missing at {}",
            lock.go_bin.display()
        );
        let home = cache_root().unwrap();
        let home_canon = home.canonicalize().unwrap_or_else(|_| home.clone());
        assert!(
            lock.bun_bin.starts_with(&home_canon),
            "bun_bin {} not under Silc cache {}",
            lock.bun_bin.display(),
            home_canon.display()
        );
        assert!(lock.python_bin.starts_with(&home_canon));
        assert!(lock.go_bin.starts_with(&home_canon));

        // Cache hit: second call should succeed without re-downloading.
        let lock2 = ensure_runtimes().expect("ensure_runtimes cache hit");
        assert_eq!(lock.bun_bin, lock2.bun_bin);
    }
}
