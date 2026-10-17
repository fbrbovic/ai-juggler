//! Claude Code environment scan + config import (spec §12, ADR-0006).
//!
//! The scan answers "what Claude setups already exist on this machine?"
//! for the onboarding wizard; the import turns one of them into a managed
//! account with a byte copy. Both are bound by ADR-0003: metadata-only
//! probing, zero writes outside managed dirs, sources never modified.

use std::path::{Path, PathBuf};

use juggler_adapter::environment::{DiscoveredInstall, ImportOutcome, InstallOrigin, LoginProbe};
use juggler_core::model::HarnessId;

use crate::claude_code::{CLAUDE_CODE_ID, is_managed_dir, prepare_managed_dir};

/// Directory/file names inside a Claude config dir that are rebuildable
/// cache or telemetry noise — skipped on import (`shell-snapshots/` alone
/// can dwarf the useful state).
const SKIP_ON_IMPORT: &[&str] = &[
    "shell-snapshots",
    "statsig",
    "telemetry",
    "cache",
    ".DS_Store",
];

/// Top-level names that make a directory look like a real Claude config —
/// a bare `.claude-something` dir with none of these is noise, not a setup.
const CONFIG_EVIDENCE: &[&str] = &[
    ".credentials.json",
    "settings.json",
    "settings.local.json",
    ".claude.json",
    "projects",
    "keybinds.json",
];

/// Default import ceiling: config dirs beyond this are almost entirely
/// transcript/cache bulk; the wizard offers a fresh account instead.
pub const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

/// Walk ceiling for size estimation (bounds pathological trees).
const MAX_WALK_ENTRIES: usize = 20_000;

/// Scans for existing Claude Code configurations under `home`, the
/// `$CLAUDE_CONFIG_DIR` override when set, and any other manager tool's
/// profile roots (`manager_roots` — e.g. t3.codes' `ClaudeCodeProfiles`).
/// The trait method passes the real locations and the metadata-only login
/// probe; tests pass sandboxes and a deterministic probe — the parameters
/// exist precisely so behavior is observable without touching real state.
pub fn scan_claude_environment(
    home: &Path,
    env_override: Option<&Path>,
    manager_roots: &[PathBuf],
    login_probe: &dyn Fn(&Path) -> LoginProbe,
) -> Vec<DiscoveredInstall> {
    let mut candidates: Vec<(PathBuf, InstallOrigin)> =
        vec![(home.join(".claude"), InstallOrigin::VendorDefault)];
    if let Some(override_path) = env_override {
        candidates.push((override_path.to_path_buf(), InstallOrigin::EnvOverride));
    }
    // Other manager tools keep whole profile roots; each direct child that
    // looks like a config is one of their profiles.
    for root in manager_roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                candidates.push((entry.path(), InstallOrigin::ManagerTool));
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(home) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            // Manual / other-tool / AI-created setups: anything dot-prefixed
            // starting with `.claude`, except the default dir itself and
            // the vendor's `.claude.json` state file.
            if name.starts_with(".claude") && name != ".claude" && name != ".claude.json" {
                candidates.push((entry.path(), InstallOrigin::HomePattern));
            }
        }
    }

    let mut seen: Vec<PathBuf> = Vec::new();
    let mut found: Vec<DiscoveredInstall> = Vec::new();
    for (path, origin) in candidates {
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        if seen.contains(&canonical) || !is_claude_config_dir(&canonical) {
            continue;
        }
        seen.push(canonical.clone());
        let login = login_probe(&canonical);
        let managed = is_managed_dir(&canonical);
        let bytes = estimate_bytes(&canonical);
        let last_used_secs = newest_mtime_secs(&canonical);
        let mut notes = origin_notes(origin, login);
        if managed {
            notes.push("Already managed by AI Juggler.".to_string());
        }
        found.push(DiscoveredInstall {
            harness: HarnessId::new(CLAUDE_CODE_ID),
            path: canonical,
            origin,
            login,
            bytes,
            last_used_secs,
            managed,
            notes,
        });
    }

    // Most useful first: the default location, then logged-in installs,
    // then most recently used.
    found.sort_by(|a, b| {
        origin_rank(a.origin)
            .cmp(&origin_rank(b.origin))
            .then(login_rank(a.login).cmp(&login_rank(b.login)))
            .then(b.last_used_secs.cmp(&a.last_used_secs))
    });
    found
}

/// Copy-imports an existing Claude config dir into the fresh managed dir
/// `dest` (which must not exist). Read-only toward `source`.
///
/// # Errors
///
/// - [`juggler_adapter::AdapterError::AccountDir`] when the source is not a
///   plausible config dir, is itself AI-Juggler-managed, when `dest`
///   exists, when the import ceiling is exceeded, or on any copy failure
///   (the message carries the plain-language reason).
pub fn import_claude_config(
    source: &Path,
    dest: &Path,
    login_probe: &dyn Fn(&Path) -> LoginProbe,
) -> Result<ImportOutcome, juggler_adapter::AdapterError> {
    import_with_cap(source, dest, login_probe, MAX_IMPORT_BYTES)
}

/// Copy-imports with an explicit byte ceiling — the testable core of
/// [`import_claude_config`].
///
/// # Errors
///
/// Same conditions as [`import_claude_config`], with the ceiling taken
/// from `max_bytes` instead of [`MAX_IMPORT_BYTES`].
pub fn import_with_cap(
    source: &Path,
    dest: &Path,
    login_probe: &dyn Fn(&Path) -> LoginProbe,
    max_bytes: u64,
) -> Result<ImportOutcome, juggler_adapter::AdapterError> {
    use juggler_adapter::AdapterError;

    let dir_error = |message: String| AdapterError::AccountDir {
        path: source.to_path_buf(),
        message,
    };

    if !source.is_dir() {
        return Err(dir_error(format!(
            "{} is not a directory",
            source.display()
        )));
    }
    let source = source
        .canonicalize()
        .map_err(|e| dir_error(e.to_string()))?;
    if !is_claude_config_dir(&source) {
        return Err(dir_error(
            "this folder does not look like a Claude Code configuration (no settings, credentials, or session history found)".to_string(),
        ));
    }
    if is_managed_dir(&source) {
        return Err(dir_error(
            "this folder is already managed by AI Juggler — launch it from Profiles instead of importing".to_string(),
        ));
    }
    if dest.exists() {
        return Err(AdapterError::AccountDir {
            path: dest.to_path_buf(),
            message: "destination already exists".to_string(),
        });
    }

    // Collect first, copy second: fail before writing anything when the
    // tree exceeds the ceiling.
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut total = 0u64;
    collect_files(
        &source,
        &source,
        &mut files,
        &mut skipped,
        &mut total,
        max_bytes,
    )
    .map_err(dir_error)?;

    let source_login = login_probe(&source);

    // Marker first: even a partially imported dir is identifiable as ours.
    prepare_managed_dir(dest)?;

    let mut files_copied: u64 = 0;
    let mut bytes_copied: u64 = 0;
    for (relative, _) in &files {
        let from = source.join(relative);
        let to = dest.join(relative);
        if let Some(parent) = to.parent() {
            crate::claude_code::create_private_dir(parent).map_err(|e| {
                AdapterError::AccountDir {
                    path: to.clone(),
                    message: format!("cannot create {}: {e}", parent.display()),
                }
            })?;
        }
        std::fs::copy(&from, &to).map_err(|e| AdapterError::AccountDir {
            path: to.clone(),
            message: format!("cannot copy {}: {e}", relative.display()),
        })?;
        if relative
            .file_name()
            .is_some_and(|n| n == ".credentials.json")
        {
            // Defense in depth: credentials stay owner-only in the copy.
            let _ = crate::claude_code::restrict_to_owner(&to);
        }
        files_copied += 1;
        bytes_copied += std::fs::metadata(&to).map(|m| m.len()).unwrap_or_default();
    }

    let login = login_probe(dest);
    let login_note = match (source_login, login) {
        (LoginProbe::LoggedIn, LoginProbe::LoggedOut) => Some(
            "Your Claude login lives in the macOS Keychain, locked to the original folder — it can't (and shouldn't) be moved. Sign in once inside the imported account; it takes a few seconds.".to_string(),
        ),
        (LoginProbe::LoggedIn, LoginProbe::LoggedIn) => {
            Some("Login carried over with the import.".to_string())
        }
        _ => None,
    };

    Ok(ImportOutcome {
        files_copied,
        bytes_copied,
        skipped,
        login,
        login_note,
    })
}

// ---- internals -----------------------------------------------------------

fn origin_rank(origin: InstallOrigin) -> u8 {
    match origin {
        InstallOrigin::VendorDefault => 0,
        InstallOrigin::EnvOverride => 1,
        InstallOrigin::HomePattern => 2,
        InstallOrigin::ManagerTool | InstallOrigin::ShellAlias => 3,
    }
}

fn login_rank(login: LoginProbe) -> u8 {
    match login {
        LoginProbe::LoggedIn => 0,
        LoginProbe::LoggedOut => 1,
    }
}

fn origin_notes(origin: InstallOrigin, login: LoginProbe) -> Vec<String> {
    let mut notes = vec![match origin {
        InstallOrigin::VendorDefault => {
            "Claude Code's standard location — what a plain `claude` command uses.".to_string()
        }
        InstallOrigin::EnvOverride => {
            "Your environment points CLAUDE_CONFIG_DIR here — likely another manager tool or a manual setup.".to_string()
        }
        InstallOrigin::HomePattern => {
            "A separate Claude configuration — created manually, by another tool, or by an AI assistant.".to_string()
        }
        InstallOrigin::ManagerTool => {
            "Managed by another tool (t3.codes) — importing copies it; the tool and its sessions are never touched.".to_string()
        }
        InstallOrigin::ShellAlias => {
            "Your shell points CLAUDE_CONFIG_DIR here — a setup you may have forgotten about.".to_string()
        }
    }];
    if matches!(login, LoginProbe::LoggedIn) {
        notes.push("Signed in — importing keeps your settings and session history.".to_string());
    }
    notes
}

fn is_claude_config_dir(dir: &Path) -> bool {
    CONFIG_EVIDENCE.iter().any(|name| dir.join(name).exists())
}

/// Sums file sizes to depth 2 (config dirs keep bulk under `projects/`),
/// bounded by [`MAX_WALK_ENTRIES`]; skips the same noise the import skips.
fn estimate_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut entries_seen = 0usize;
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((current, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in read.flatten() {
            entries_seen += 1;
            if entries_seen > MAX_WALK_ENTRIES {
                return total;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy().into_owned();
            if SKIP_ON_IMPORT.contains(&name.as_str()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() && depth < 2 {
                stack.push((entry.path(), depth + 1));
            } else {
                total += meta.len();
            }
        }
    }
    total
}

fn newest_mtime_secs(dir: &Path) -> Option<u64> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter_map(|meta| meta.modified().ok())
        .map(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default()
        })
        .max()
}

/// Depth-first file collection relative to `root`; noise entries are
/// skipped and recorded. Fails (as a string) once the running total
/// exceeds `max_bytes` — before any copying happens.
fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(PathBuf, u64)>,
    skipped: &mut Vec<String>,
    total: &mut u64,
    max_bytes: u64,
) -> Result<(), String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Ok(()); // unreadable subtree: skip it, don't fail the import
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let relative = match entry.path().strip_prefix(root) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => entry.path(),
        };
        if meta.is_dir() {
            if SKIP_ON_IMPORT.contains(&name.as_str()) {
                skipped.push(format!("{}/ (rebuildable cache)", relative.display()));
                continue;
            }
            collect_files(root, &entry.path(), files, skipped, total, max_bytes)?;
        } else {
            if SKIP_ON_IMPORT.contains(&name.as_str()) {
                skipped.push(format!("{} (rebuildable)", relative.display()));
                continue;
            }
            *total += meta.len();
            if *total > max_bytes {
                return Err(format!(
                    "configuration is larger than the {} GiB import ceiling — add a fresh account instead",
                    max_bytes / (1024 * 1024 * 1024)
                ));
            }
            files.push((relative, meta.len()));
        }
    }
    Ok(())
}
