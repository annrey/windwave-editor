//! Path Scanner — auto-detect installed CLI agents on the user's PATH.
//!
//! Mirrors Open Design's:
//! - `apps/daemon/src/runtimes/executables.ts` (PATH scanning + fallbackBins)
//! - `apps/daemon/src/runtimes/detection.ts` (version/help/model probing)
//! - `packages/platform/src/index.ts` (wellKnownUserToolchainBins)

use crate::cli_defs::types::AgentDef;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Result of scanning the system for a particular CLI agent.
#[derive(Debug, Clone)]
pub struct CliDetection {
    pub def: &'static AgentDef,
    /// Resolved absolute path to the executable
    pub executable_path: PathBuf,
    /// Actual binary name used (bin or fallback)
    pub resolved_bin: String,
    /// Version string from `--version`
    pub version: Option<String>,
    /// Available models if discoverable
    pub available_models: Vec<String>,
    /// Whether the agent has valid auth
    pub has_auth: bool,
    /// Whether detection succeeded
    pub detected: bool,
    /// Error message if detection failed
    pub error: Option<String>,
}

impl CliDetection {
    pub fn not_found(def: &'static AgentDef, error: String) -> Self {
        Self {
            def,
            executable_path: PathBuf::new(),
            resolved_bin: def.bin.to_string(),
            version: None,
            available_models: vec![],
            has_auth: false,
            detected: false,
            error: Some(error),
        }
    }
}

/// Scan the user's PATH for all known CLI agents.
/// Returns detections for every registered agent def.
pub fn scan_all_agents(defs: &[&'static AgentDef]) -> Vec<CliDetection> {
    defs.iter().map(|def| detect_agent(def)).collect()
}

/// Detect a single agent: find binary, probe version, check auth.
pub fn detect_agent(def: &'static AgentDef) -> CliDetection {
    let path_env = std::env::var("PATH").unwrap_or_default();
    let home_dir = dirs_home();

    // Check env override first (e.g. CLAUDE_BIN=/custom/path/claude)
    if let Some(env_var) = def.env_override {
        if let Ok(custom_path) = std::env::var(env_var) {
            let p = Path::new(&custom_path);
            if p.is_file() {
                return CliDetection {
                    def,
                    executable_path: p.to_path_buf(),
                    resolved_bin: p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(def.bin)
                        .to_string(),
                    version: probe_version(p),
                    available_models: probe_models(def, p),
                    has_auth: probe_auth(def, p),
                    detected: true,
                    error: None,
                };
            }
        }
    }

    // Try all known bin names (bin + fallbacks)
    for bin_name in def.all_known_bins() {
        if let Some(resolved_path) = resolve_on_path(bin_name, &path_env, &home_dir) {
            return CliDetection {
                def,
                resolved_bin: bin_name.to_string(),
                version: probe_version(&resolved_path),
                available_models: probe_models(def, &resolved_path),
                has_auth: probe_auth(def, &resolved_path),
                detected: true,
                error: None,
                executable_path: resolved_path,
            };
        }
    }

    // Check well-known toolchain directories
    for tc_dir in well_known_user_toolchain_bins(&home_dir) {
        for bin_name in def.all_known_bins() {
            let candidate = tc_dir.join(Path::new(bin_name));
            if candidate.is_file() {
                return CliDetection {
                    def,
                    resolved_bin: bin_name.to_string(),
                    version: probe_version(&candidate),
                    available_models: probe_models(def, &candidate),
                    has_auth: probe_auth(def, &candidate),
                    detected: true,
                    error: None,
                    executable_path: candidate,
                };
            }
        }
    }

    CliDetection::not_found(
        def,
        format!("{} not found on PATH (tried: {})", def.display_name, {
            let bins: Vec<&str> = def.all_known_bins().collect();
            bins.join(", ")
        }),
    )
}

/// Resolve a binary name on the PATH, returning the absolute path if found.
fn resolve_on_path(bin_name: &str, path_env: &str, _home_dir: &Option<PathBuf>) -> Option<PathBuf> {
    for dir in path_env.split(':') {
        let candidate = Path::new(dir).join(bin_name);
        if candidate.is_file() {
            // Check executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = candidate.metadata() {
                    if meta.permissions().mode() & 0o111 != 0 {
                        return Some(candidate);
                    }
                }
            }
            #[cfg(not(unix))]
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// Well-known user toolchain directories that may not be on PATH
/// (especially for macOS GUI-launched processes).
fn well_known_user_toolchain_bins(home_dir: &Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let home = home_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("/Users/unknown"));

    // Standard local bins
    dirs.push(home.join(".local/bin"));
    dirs.push(home.join(".cargo/bin"));

    // npm global
    if let Ok(prefix) = std::env::var("NPM_CONFIG_PREFIX") {
        dirs.push(PathBuf::from(prefix).join("bin"));
    }
    dirs.push(home.join(".npm-global/bin"));

    // Volta
    dirs.push(home.join(".volta/bin"));

    // fnm (Fast Node Manager)
    let fnm_dir = home.join("Library/Application Support/fnm/node-versions");
    if fnm_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&fnm_dir) {
            for entry in entries.flatten() {
                let install_dir = entry.path().join("installation/bin");
                if install_dir.exists() {
                    dirs.push(install_dir);
                }
            }
        }
    }

    // mise
    let mise_node = home.join(".local/share/mise/installs/node");
    if mise_node.exists() {
        if let Ok(entries) = std::fs::read_dir(&mise_node) {
            for entry in entries.flatten() {
                dirs.push(entry.path().join("bin"));
            }
        }
    }

    // Homebrew (macOS GUI launch PATH compensation)
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    // Filter to existing dirs
    dirs.into_iter().filter(|d| d.exists()).collect()
}

/// Get user home directory.
fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from).or({
        #[cfg(target_os = "macos")]
        {
            None
        }
        #[cfg(not(target_os = "macos"))]
        {
            dirs::home_dir()
        }
    })
}

/// Probe agent version by running `<bin> --version`.
fn probe_version(executable: &Path) -> Option<String> {
    let mut cmd = Command::new(executable);
    cmd.arg("--version");
    output_with_timeout(&mut cmd, Duration::from_secs(2))
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                String::from_utf8(output.stdout)
                    .or_else(|_| String::from_utf8(output.stderr))
                    .ok()
                    .map(|s| s.trim().to_string())
            }
        })
}

fn output_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    use std::process::Stdio;
    use std::thread;

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        thread::sleep(Duration::from_millis(20));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("command timed out after {:?}", timeout),
    ))
}

/// Probe available models by running `<bin> --list-models` if supported.
fn probe_models(_def: &AgentDef, _executable: &Path) -> Vec<String> {
    // Most CLI agents don't have a standard model listing interface yet.
    // We can probe with `<bin> --list-models` for agents that support it.
    Vec::new()
}

/// Probe agent auth status.
fn probe_auth(_def: &AgentDef, _executable: &Path) -> bool {
    // Most CLI agents manage auth internally via API keys.
    // For now, assume auth is OK if the binary exists.
    // Future: run `<bin> whoami` or equivalent.
    true
}

/// Helper: get all well-known toolchain bin directories
pub fn toolchain_bins() -> Vec<PathBuf> {
    well_known_user_toolchain_bins(&dirs_home())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_well_known_dirs_does_not_panic() {
        // Test with real home dir — the function should never panic
        let home = dirs_home();
        let dirs = well_known_user_toolchain_bins(&home);
        // We can't assert specific dirs exist (varies by machine),
        // but the function should always return without panicking
        let _ = dirs.len();
    }

    #[test]
    fn test_resolve_on_path() {
        // Test that common shell exists on PATH
        let path = std::env::var("PATH").unwrap_or_default();
        let home = dirs_home();
        // sh should always be resolvable
        let result = resolve_on_path("sh", &path, &home);
        assert!(result.is_some(), "sh should be on PATH");
    }

    #[test]
    fn test_resolve_nonexistent() {
        let path = std::env::var("PATH").unwrap_or_default();
        let home = dirs_home();
        let result = resolve_on_path("__nonexistent_binary_xyz__", &path, &home);
        assert!(result.is_none());
    }
}
