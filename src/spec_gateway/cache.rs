//! Opt-in cross-run memo of the verification report (spike).
//!
//! A repeated verification on an unchanged tree redoes identical work; the
//! expensive part is the test layer, which spawns a test runner per scenario.
//! This module keys a serialized [`VerificationReport`] on every input that can
//! change the report:
//!
//! * the spec bytes,
//! * the resolved layer set,
//! * the `--code` path,
//! * the git state of the code tree (HEAD, `git status --porcelain`, and the
//!   content hash of every dirty path),
//! * the agent-spec version.
//!
//! Entries live as one JSON file per key under `<code>/.agent-spec-cache/`. The
//! cache never weakens a layer: a hit replays a report that the full pipeline
//! produced for exactly these inputs, and every run records whether it was
//! served from the cache. Any cache IO problem degrades to a miss with a note —
//! it can never change a verdict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::spec_core::{SpecResult, VerificationReport};

/// Directory under the code root that holds the cache entries.
pub const CACHE_DIR: &str = ".agent-spec-cache";

/// Bumped when the key composition or the entry format changes. The crate
/// version is mixed into the key separately, so a release also invalidates.
const CACHE_FORMAT: &str = "agent-spec-cache-v1";

/// Where a report came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    Hit,
    Miss,
}

impl CacheStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }
}

/// The result of a cache-aware verification.
pub struct CachedRun {
    /// The report — replayed from the cache on a hit, freshly computed on a miss.
    pub report: VerificationReport,
    /// Provenance of [`CachedRun::report`].
    pub status: CacheStatus,
    /// Set when the cache could not be used or written; never affects the
    /// report or the verdict.
    pub note: Option<String>,
}

/**
 * @cc [label:verification-cache] key-covers-every-report-input
 * Everything that determines a report, and therefore the cache key: spec
 * bytes, resolved layer set, code path, git state of the code tree (HEAD +
 * `git status --porcelain` + content hash of every dirty path) and crate
 * version. Any change to an input that can change the report must miss;
 * agent-spec's own tool state is excluded from git state.
 */
pub struct CacheInputs<'a> {
    /// Spec file the report was produced from.
    pub spec: &'a Path,
    /// Code root the verification ran against.
    pub code: &'a Path,
    /// Resolved layer filter, if the caller narrowed the layer set.
    pub layers: Option<&'a [&'a str]>,
    /// Change set handed to the boundary layer.
    pub change_paths: &'a [PathBuf],
    /// AI verifier mode as spelled on the command line.
    pub ai_mode: &'a str,
}

/**
 * @cc [label:verification-cache] hit-replays-a-pipeline-report
 * Verify, reusing a cached report when the inputs are unchanged. `verify` runs
 * the real pipeline and is not called on a hit: a replayed report was produced
 * by the full pipeline for exactly these inputs, so no layer is skipped or
 * weakened, and provenance is surfaced through `CachedRun::status`.
 */
/**
 * @cc [label:verification-cache] cache-io-never-gates
 * Any cache IO problem (unreadable spec, code root outside git, unwritable
 * cache directory, corrupt entry) degrades to a miss plus a note; it never
 * changes the report or the verdict, in either direction.
 */
pub fn verify_cached<F>(inputs: &CacheInputs<'_>, verify: F) -> SpecResult<CachedRun>
where
    F: FnOnce() -> SpecResult<VerificationReport>,
{
    let key = match key_for(inputs) {
        Ok(key) => key,
        Err(reason) => {
            return Ok(CachedRun {
                report: verify()?,
                status: CacheStatus::Miss,
                note: Some(format!("cache disabled: {reason}")),
            });
        }
    };

    let store = CacheStore {
        dir: inputs.code.join(CACHE_DIR),
    };
    if let Some(report) = store.read(&key) {
        return Ok(CachedRun {
            report,
            status: CacheStatus::Hit,
            note: None,
        });
    }

    let report = verify()?;
    let note = store
        .write(&key, &report)
        .err()
        .map(|err| format!("failed to write {}: {err}", store.path(&key).display()));
    Ok(CachedRun {
        report,
        status: CacheStatus::Miss,
        note,
    })
}

// ── key composition ─────────────────────────────────────────────

/// Compute the cache key for `inputs`.
fn key_for(inputs: &CacheInputs<'_>) -> Result<String, String> {
    let spec_bytes = fs::read(inputs.spec)
        .map_err(|err| format!("cannot read spec {}: {err}", inputs.spec.display()))?;
    let code_path = fs::canonicalize(inputs.code)
        .unwrap_or_else(|_| inputs.code.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let git_state = git_state(inputs.code)?;

    let mut key = Fnv1a::new();
    key.write(CACHE_FORMAT.as_bytes());
    key.write(env!("CARGO_PKG_VERSION").as_bytes());
    key.write(&spec_bytes);
    key.write(code_path.as_bytes());
    key.write(normalized_layers(inputs.layers).as_bytes());
    key.write(inputs.ai_mode.as_bytes());
    for change in inputs.change_paths {
        key.write(change.as_os_str().as_encoded_bytes());
    }
    key.write(git_state.as_bytes());
    Ok(key.finish())
}

/// The layer set as one canonical string, so `lint, test` and `lint,test` are
/// the same layer set and "no filter" is never confused with a filter.
fn normalized_layers(layers: Option<&[&str]>) -> String {
    match layers {
        None => "layers:none".to_owned(),
        Some(list) => {
            let joined = list
                .iter()
                .map(|layer| layer.trim())
                .filter(|layer| !layer.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            format!("layers:{joined}")
        }
    }
}

/// Digest of the code tree's git state: HEAD, the porcelain status, and the
/// content of every dirty path.
///
/// The cache directory and agent-spec's own run logs are excluded: they are
/// tool state written *by* verification, and folding them in would make every
/// run invalidate the previous one.
fn git_state(code: &Path) -> Result<String, String> {
    let root = git_output(code, &["rev-parse", "--show-toplevel"])?;
    let root = root.trim();
    if root.is_empty() {
        return Err("code root is not inside a git work tree".to_owned());
    }
    let head = git_output(code, &["rev-parse", "HEAD"])
        .map(|head| head.trim().to_owned())
        .unwrap_or_else(|_| "no-head".to_owned());
    let status = git_output(code, &["status", "--porcelain", "-uall"])?;

    let mut state = Fnv1a::new();
    state.write(b"git-state-v1");
    state.write(root.as_bytes());
    state.write(head.as_bytes());
    for line in status.lines() {
        let Some(path) = porcelain_path(line) else {
            continue;
        };
        if is_tool_state(path) {
            continue;
        }
        state.write(line.as_bytes());
        hash_path(&mut state, &Path::new(root).join(path))?;
    }
    Ok(state.finish())
}

/// Extract the path from a `git status --porcelain` line (`XY <path>`, with
/// `old -> new` for renames).
fn porcelain_path(line: &str) -> Option<&str> {
    if line.len() <= 3 {
        return None;
    }
    let rest = line.get(3..)?.trim();
    Some(rest.rsplit(" -> ").next().unwrap_or(rest))
}

/// True for paths agent-spec itself owns (cache entries, run logs, pending AI
/// requests) — never part of the verified content.
fn is_tool_state(path: &str) -> bool {
    path.split('/').any(|part| part.starts_with(".agent-spec"))
}

/// Mix one path's content into `state`; directories are walked in sorted order
/// so the digest is stable across filesystems.
fn hash_path(state: &mut Fnv1a, path: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        // A deleted tracked path has no content to hash; its status line still
        // carries the deletion.
        Err(_) => {
            state.write(b"<absent>");
            return Ok(());
        }
    };

    if meta.is_dir() {
        let mut children: Vec<PathBuf> = fs::read_dir(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        children.sort();
        for child in children {
            hash_path(state, &child)?;
        }
    } else if meta.is_file() {
        let bytes =
            fs::read(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        state.write(&bytes);
    } else if let Ok(target) = fs::read_link(path) {
        state.write(target.as_os_str().as_encoded_bytes());
    }
    Ok(())
}

fn git_output(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|err| format!("cannot run git: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// FNV-1a over a length-prefixed byte stream (no dependency, and the length
/// prefix keeps field boundaries unambiguous).
struct Fnv1a {
    hash: u64,
}

impl Fnv1a {
    fn new() -> Self {
        Self {
            hash: 0xcbf2_9ce4_8422_2325,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64).to_le_bytes() {
            self.write_byte(byte);
        }
        for byte in bytes {
            self.write_byte(*byte);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        self.hash ^= u64::from(byte);
        self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn finish(&self) -> String {
        format!("{:016x}", self.hash)
    }
}

// ── entry storage ──────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    version: String,
    report: VerificationReport,
}

struct CacheStore {
    dir: PathBuf,
}

impl CacheStore {
    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Read a replayed report. A missing, unreadable, stale-version, or corrupt
    /// entry is simply a miss.
    fn read(&self, key: &str) -> Option<VerificationReport> {
        let bytes = fs::read(self.path(key)).ok()?;
        let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
        if entry.version != env!("CARGO_PKG_VERSION") {
            return None;
        }
        Some(entry.report)
    }

    fn write(&self, key: &str, report: &VerificationReport) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let entry = CacheEntry {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            report: report.clone(),
        };
        let body = serde_json::to_vec(&entry).map_err(std::io::Error::other)?;
        fs::write(self.path(key), body)
    }
}
