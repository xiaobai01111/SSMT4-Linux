use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

pub const MIGOTO_RUNTIME_LOG_MAX_BYTES: u64 = 32 * 1024 * 1024;
const MIGOTO_RUNTIME_LOG_KEEP_BYTES: u64 = 4 * 1024 * 1024;
const MIGOTO_RUNTIME_LOG_SCAN_MAX_DEPTH: usize = 4;

const EXACT_RUNTIME_LOG_NAMES: &[&str] = &[
    "bridge-output.log",
    "d3d9_log.txt",
    "d3d10_log.txt",
    "d3d10core_log.txt",
    "d3d11_log.txt",
    "d3d12_log.txt",
    "dxgi_log.txt",
    "loader.log",
    "loader.txt",
    "migoto.log",
    "xxmi.log",
];

const EXCLUDED_RUNTIME_LOG_NAMES: &[&str] =
    &["player.log", "player-prev.log", "launcher.log", "games.log"];

const MANAGED_LOG_SUBSTRINGS: &[&str] = &[
    "migoto",
    "xxmi",
    "shaderfix",
    "shader_fix",
    "shader-fix",
    "loader",
    "modmanager",
    "mod_manager",
    "mod-loader",
    "mod_loader",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedMigotoLogPaths {
    pub live_files: Vec<PathBuf>,
    pub scan_roots: Vec<PathBuf>,
}

impl ManagedMigotoLogPaths {
    pub fn new(
        game_folder: &Path,
        importer_folder: &Path,
        mod_folder: &Path,
        shader_fixes_folder: &Path,
        bridge_cache_dir: &Path,
    ) -> Self {
        let mut live_files = BTreeSet::new();
        let mut scan_roots = BTreeSet::new();

        let exact_root_logs = [
            game_folder.to_path_buf(),
            importer_folder.to_path_buf(),
            bridge_cache_dir.to_path_buf(),
        ];

        for root in &exact_root_logs {
            for name in EXACT_RUNTIME_LOG_NAMES {
                live_files.insert(root.join(name));
            }
        }

        for root in [
            bridge_cache_dir,
            importer_folder,
            mod_folder,
            shader_fixes_folder,
        ] {
            scan_roots.insert(root.to_path_buf());
        }

        Self {
            live_files: live_files.into_iter().collect(),
            scan_roots: scan_roots.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigotoLogPruneSummary {
    pub checked_files: usize,
    pub trimmed_files: usize,
    pub reclaimed_bytes: u64,
    pub failures: usize,
}

impl MigotoLogPruneSummary {
    pub fn is_noop(&self) -> bool {
        self.trimmed_files == 0 && self.failures == 0
    }

    pub fn describe(&self) -> String {
        format!(
            "checked={}, trimmed={}, reclaimed={} MiB, failures={}",
            self.checked_files,
            self.trimmed_files,
            self.reclaimed_bytes / 1024 / 1024,
            self.failures
        )
    }
}

pub fn prune_stale_logs(paths: &ManagedMigotoLogPaths) -> MigotoLogPruneSummary {
    let mut candidates = BTreeSet::new();

    for path in &paths.live_files {
        push_candidate(&mut candidates, path);
    }

    for root in &paths.scan_roots {
        collect_candidates(root, 0, &mut candidates);
    }

    prune_candidates(
        candidates,
        MIGOTO_RUNTIME_LOG_MAX_BYTES,
        MIGOTO_RUNTIME_LOG_KEEP_BYTES,
    )
}

pub fn prune_live_logs(paths: &ManagedMigotoLogPaths) -> MigotoLogPruneSummary {
    let mut candidates = BTreeSet::new();
    for path in &paths.live_files {
        push_candidate(&mut candidates, path);
    }
    prune_candidates(
        candidates,
        MIGOTO_RUNTIME_LOG_MAX_BYTES,
        MIGOTO_RUNTIME_LOG_KEEP_BYTES,
    )
}

fn prune_candidates(
    candidates: BTreeSet<PathBuf>,
    max_bytes: u64,
    keep_bytes: u64,
) -> MigotoLogPruneSummary {
    let mut summary = MigotoLogPruneSummary::default();

    for path in candidates {
        summary.checked_files += 1;
        match trim_log_file_if_needed(&path, max_bytes, keep_bytes) {
            Ok(reclaimed) if reclaimed > 0 => {
                summary.trimmed_files += 1;
                summary.reclaimed_bytes += reclaimed;
            }
            Ok(_) => {}
            Err(err) => {
                summary.failures += 1;
                warn!("3DMigoto 日志整理失败: {} ({})", path.display(), err);
            }
        }
    }

    summary
}

fn push_candidate(target: &mut BTreeSet<PathBuf>, path: &Path) {
    if !path.is_file() {
        return;
    }
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .map(is_managed_runtime_log_name)
        .unwrap_or(false)
    {
        return;
    }

    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    target.insert(canonical);
}

fn collect_candidates(root: &Path, depth: usize, out: &mut BTreeSet<PathBuf>) {
    if depth > MIGOTO_RUNTIME_LOG_SCAN_MAX_DEPTH || !root.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_candidates(&path, depth + 1, out);
            continue;
        }
        push_candidate(out, &path);
    }
}

fn is_managed_runtime_log_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if EXCLUDED_RUNTIME_LOG_NAMES.contains(&lower.as_str()) {
        return false;
    }
    if EXACT_RUNTIME_LOG_NAMES.contains(&lower.as_str()) {
        return true;
    }
    if !(lower.ends_with(".log") || lower.ends_with(".txt")) {
        return false;
    }
    if lower.ends_with("_log.txt") && (lower.contains("d3d") || lower.contains("dxgi")) {
        return true;
    }
    MANAGED_LOG_SUBSTRINGS
        .iter()
        .any(|needle| lower.contains(needle))
}

fn trim_log_file_if_needed(path: &Path, max_bytes: u64, keep_bytes: u64) -> Result<u64, String> {
    let metadata = fs::metadata(path).map_err(|err| format!("读取元数据失败: {}", err))?;
    let file_len = metadata.len();
    if file_len <= max_bytes {
        return Ok(0);
    }

    let keep_bytes = keep_bytes.min(max_bytes).max(1);
    let tail_start = file_len.saturating_sub(keep_bytes);

    let mut input = fs::File::open(path).map_err(|err| format!("打开日志失败: {}", err))?;
    input
        .seek(SeekFrom::Start(tail_start))
        .map_err(|err| format!("定位日志尾部失败: {}", err))?;
    let mut tail = Vec::new();
    input
        .read_to_end(&mut tail)
        .map_err(|err| format!("读取日志尾部失败: {}", err))?;

    let marker = format!(
        "[SSMT4] log trimmed automatically (previous_size={} bytes, keep_tail={} bytes)\n",
        file_len,
        tail.len()
    );

    let mut output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|err| format!("重写日志失败: {}", err))?;
    output
        .write_all(marker.as_bytes())
        .map_err(|err| format!("写入日志头失败: {}", err))?;
    output
        .write_all(&tail)
        .map_err(|err| format!("写入日志尾部失败: {}", err))?;
    output
        .flush()
        .map_err(|err| format!("刷新日志失败: {}", err))?;

    let new_len = marker.len() as u64 + tail.len() as u64;
    Ok(file_len.saturating_sub(new_len))
}

#[cfg(test)]
mod tests {
    use super::{
        collect_candidates, is_managed_runtime_log_name, prune_candidates, ManagedMigotoLogPaths,
    };
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssmt4-migoto-logs-{label}-{nonce}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_bytes(path: &PathBuf, len: usize, fill: u8) {
        let parent = path.parent().expect("path parent");
        fs::create_dir_all(parent).expect("create parent");
        fs::write(path, vec![fill; len]).expect("write bytes");
    }

    #[test]
    fn managed_log_name_matches_expected_patterns_and_excludes_game_logs() {
        assert!(is_managed_runtime_log_name("d3d11_log.txt"));
        assert!(is_managed_runtime_log_name("bridge-output.log"));
        assert!(is_managed_runtime_log_name("xxmi-runtime.log"));
        assert!(is_managed_runtime_log_name("shaderfix_debug.txt"));
        assert!(!is_managed_runtime_log_name("Player.log"));
        assert!(!is_managed_runtime_log_name("launcher.log"));
        assert!(!is_managed_runtime_log_name("readme.txt"));
    }

    #[test]
    fn managed_paths_include_exact_live_files_and_scan_roots() {
        let dir = unique_temp_dir("paths");
        let paths = ManagedMigotoLogPaths::new(
            &dir.join("game"),
            &dir.join("importer"),
            &dir.join("importer").join("Mods"),
            &dir.join("importer").join("ShaderFixes"),
            &dir.join("Cache").join("bridge"),
        );

        assert!(paths
            .live_files
            .iter()
            .any(|path| path.ends_with("game/d3d11_log.txt")));
        assert!(paths
            .live_files
            .iter()
            .any(|path| path.ends_with("Cache/bridge/bridge-output.log")));
        assert!(paths
            .scan_roots
            .iter()
            .any(|path| path.ends_with("importer/Mods")));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_candidates_trims_large_logs_and_skips_small_or_excluded_files() {
        let dir = unique_temp_dir("trim");
        let managed = dir.join("d3d11_log.txt");
        let excluded = dir.join("Player.log");
        let small = dir.join("loader.log");

        write_bytes(&managed, 128, b'a');
        write_bytes(&excluded, 128, b'b');
        write_bytes(&small, 16, b'c');

        let mut candidates = BTreeSet::new();
        super::push_candidate(&mut candidates, &managed);
        super::push_candidate(&mut candidates, &excluded);
        super::push_candidate(&mut candidates, &small);

        let summary = prune_candidates(candidates, 64, 16);

        assert_eq!(summary.checked_files, 2);
        assert_eq!(summary.trimmed_files, 1);
        assert_eq!(summary.failures, 0);
        assert!(summary.reclaimed_bytes > 0);
        let managed_content = fs::read(&managed).expect("read managed log");
        assert!(managed_content.starts_with(b"[SSMT4]"));
        assert!(fs::metadata(&small).expect("small metadata").len() <= 16);
        assert_eq!(
            fs::metadata(&excluded).expect("excluded metadata").len(),
            128
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_stale_logs_scans_nested_mod_directories() {
        let dir = unique_temp_dir("scan");
        let paths = ManagedMigotoLogPaths::new(
            &dir.join("game"),
            &dir.join("importer"),
            &dir.join("importer").join("Mods"),
            &dir.join("importer").join("ShaderFixes"),
            &dir.join("Cache").join("bridge"),
        );
        let nested_log = dir
            .join("importer")
            .join("Mods")
            .join("SomeMod")
            .join("xxmi.log");
        write_bytes(&nested_log, 128, b'x');

        let mut candidates = BTreeSet::new();
        for root in &paths.scan_roots {
            collect_candidates(root, 0, &mut candidates);
        }
        let summary = prune_candidates(candidates, 64, 16);

        assert_eq!(summary.trimmed_files, 1);
        assert!(fs::read(&nested_log)
            .expect("read nested log")
            .starts_with(b"[SSMT4]"));

        let _ = fs::remove_dir_all(dir);
    }
}
