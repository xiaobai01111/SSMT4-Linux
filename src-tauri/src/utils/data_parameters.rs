use crate::configs::app_config;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DATA_PARAMETERS_REPO_URL: &str = "https://github.com/xiaobai01111/Data-parameters";

static RESOURCE_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// 在应用启动后注入 Tauri resource_dir，便于发布版定位资源目录。
pub fn set_resource_dir(resource_dir: PathBuf) {
    *RESOURCE_DIR.write().unwrap() = Some(resource_dir);
}

/// 外部 Data-parameters 仓库在本地的托管目录（随 dataDir 变化）。
pub fn managed_repo_dir() -> PathBuf {
    app_config::get_app_data_dir().join("Data-parameters")
}

fn normalize_relative(relative: &str) -> &str {
    relative.trim_start_matches(['/', '\\'])
}

fn push_unique_path(list: &mut Vec<PathBuf>, seen: &mut HashSet<String>, path: PathBuf) {
    let key = path.to_string_lossy().to_string();
    if seen.insert(key) {
        list.push(path);
    }
}

fn run_git(args: &[&str], action: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("执行 git {} 失败: {}", action, e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("git {} 失败，退出码: {}", action, output.status))
    } else {
        Err(format!("git {} 失败: {}", action, stderr))
    }
}

fn required_repo_files() -> &'static [&'static str] {
    &[
        "catalogs/game_catalog.seed.json",
        "catalogs/proton_catalog.seed.json",
        "games/GameIconConfig.json",
        "games/GenshinImpact/Config.json",
        "games/GenshinImpact/Icon.png",
    ]
}

fn is_readable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
        && std::fs::File::open(path).is_ok()
}

fn validate_repo_files(repo_dir: &Path) -> Result<(), String> {
    let mut missing = Vec::new();
    for rel in required_repo_files() {
        let path = repo_dir.join(rel);
        if !is_readable_file(&path) {
            missing.push(rel.to_string());
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Data-parameters 仓库不完整，缺少关键文件: {}",
            missing.join(", ")
        ))
    }
}

fn backup_broken_repo(repo_dir: &Path) -> Result<(), String> {
    if !repo_dir.exists() {
        return Ok(());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_name = format!("Data-parameters.broken.{}", ts);
    let backup_path = repo_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(backup_name);
    std::fs::rename(repo_dir, &backup_path).map_err(|e| {
        format!(
            "重命名损坏的 Data-parameters 目录失败 {} -> {}: {}",
            repo_dir.display(),
            backup_path.display(),
            e
        )
    })
}

fn clone_repo(repo_dir: &Path) -> Result<(), String> {
    let repo_dir_str = repo_dir.to_string_lossy().to_string();
    run_git(
        &[
            "clone",
            "--depth",
            "1",
            DATA_PARAMETERS_REPO_URL,
            &repo_dir_str,
        ],
        "clone",
    )
}

fn pull_repo(repo_dir: &Path) -> Result<(), String> {
    let repo_dir_str = repo_dir.to_string_lossy().to_string();
    let _ = run_git(
        &[
            "-C",
            &repo_dir_str,
            "remote",
            "set-url",
            "origin",
            DATA_PARAMETERS_REPO_URL,
        ],
        "remote set-url",
    );
    run_git(&["-C", &repo_dir_str, "pull", "--ff-only"], "pull")
}

fn restore_worktree(repo_dir: &Path) -> Result<(), String> {
    let repo_dir_str = repo_dir.to_string_lossy().to_string();
    run_git(
        &[
            "-C",
            &repo_dir_str,
            "checkout",
            "--force",
            "HEAD",
            "--",
            ".",
        ],
        "checkout --force",
    )
}

/// 拉取（或首次克隆）Data-parameters 外部仓库。
///
/// 说明：
/// - 首启/损坏场景会自动自愈（重命名旧目录后重新克隆）。
/// - 失败时返回错误，由调用方决定是否降级继续运行。
pub fn sync_managed_repo() -> Result<(), String> {
    let repo_dir = managed_repo_dir();
    let parent = repo_dir
        .parent()
        .ok_or_else(|| format!("Data-parameters 目录非法: {}", repo_dir.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("创建 Data-parameters 上级目录失败: {}", e))?;

    let git_dir = repo_dir.join(".git");
    if git_dir.exists() {
        if let Err(e) = pull_repo(&repo_dir) {
            tracing::warn!("Data-parameters pull 失败，尝试修复工作区: {}", e);
        }

        if validate_repo_files(&repo_dir).is_ok() {
            return Ok(());
        }

        if let Err(e) = restore_worktree(&repo_dir) {
            tracing::warn!("修复 Data-parameters 工作区失败: {}", e);
        }
        if validate_repo_files(&repo_dir).is_ok() {
            return Ok(());
        }

        tracing::warn!("Data-parameters 仓库仍不完整，准备重新克隆");
        backup_broken_repo(&repo_dir)?;
        clone_repo(&repo_dir)?;
        return validate_repo_files(&repo_dir);
    }

    if repo_dir.exists() {
        tracing::warn!(
            "Data-parameters 路径已存在但不是 git 仓库，尝试重建: {}",
            repo_dir.display()
        );
        backup_broken_repo(&repo_dir)?;
        clone_repo(&repo_dir)?;
        return validate_repo_files(&repo_dir);
    }

    clone_repo(&repo_dir)?;
    validate_repo_files(&repo_dir)
}

fn data_parameter_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(raw) = std::env::var("SSMT4_DATA_PARAMETERS_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            push_unique_path(&mut roots, &mut seen, PathBuf::from(trimmed));
        }
    }

    // 优先使用托管目录（由 sync_managed_repo 获取）
    push_unique_path(&mut roots, &mut seen, managed_repo_dir());

    if let Ok(guard) = RESOURCE_DIR.read() {
        if let Some(resource_dir) = guard.as_ref() {
            // 兼容某些手工部署布局
            push_unique_path(&mut roots, &mut seen, resource_dir.join("Data-parameters"));
            push_unique_path(
                &mut roots,
                &mut seen,
                resource_dir.join("resources").join("Data-parameters"),
            );
            push_unique_path(&mut roots, &mut seen, resource_dir.clone());
            push_unique_path(&mut roots, &mut seen, resource_dir.join("resources"));
        }
    }

    // 开发模式回退：工作区同级目录
    if let Ok(cwd) = std::env::current_dir() {
        push_unique_path(&mut roots, &mut seen, cwd.join("Data-parameters"));
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push_unique_path(
        &mut roots,
        &mut seen,
        manifest_dir.join("..").join("Data-parameters"),
    );
    push_unique_path(
        &mut roots,
        &mut seen,
        manifest_dir.join("..").join("..").join("Data-parameters"),
    );

    roots
}

fn legacy_resource_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(guard) = RESOURCE_DIR.read() {
        if let Some(resource_dir) = guard.as_ref() {
            push_unique_path(&mut roots, &mut seen, resource_dir.join("resources"));
            push_unique_path(&mut roots, &mut seen, resource_dir.clone());
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push_unique_path(&mut roots, &mut seen, manifest_dir.join("resources"));

    roots
}

fn resolve_from_roots(roots: &[PathBuf], relative: &str) -> Option<PathBuf> {
    let rel = normalize_relative(relative);
    if rel.is_empty() {
        return None;
    }

    roots
        .iter()
        .map(|root| root.join(rel))
        .find(|path| path.exists())
}

pub fn resolve_data_path(relative: &str) -> Option<PathBuf> {
    resolve_from_roots(&data_parameter_roots(), relative)
}

pub fn read_data_json(relative: &str) -> Result<String, String> {
    let path = resolve_data_path(relative).ok_or_else(|| {
        format!(
            "未找到 Data-parameters 文件: {}（请检查仓库结构）",
            relative
        )
    })?;
    std::fs::read_to_string(&path)
        .map_err(|e| format!("读取 Data-parameters 文件失败 {}: {}", path.display(), e))
}

pub fn resolve_catalog_path(file_name: &str) -> Option<PathBuf> {
    let name = file_name.trim();
    if name.is_empty() {
        return None;
    }

    let data_roots = data_parameter_roots();
    let catalog_rel = format!("catalogs/{}", name);
    if let Some(path) = resolve_from_roots(&data_roots, &catalog_rel) {
        return Some(path);
    }

    // 兼容旧内置路径：resources/bootstrap/*.seed.json
    let legacy_roots = legacy_resource_roots();
    let legacy_rel = format!("bootstrap/{}", name);
    resolve_from_roots(&legacy_roots, &legacy_rel)
}

pub fn read_catalog_json(file_name: &str) -> Result<String, String> {
    let path = resolve_catalog_path(file_name).ok_or_else(|| {
        format!(
            "未找到 catalog 文件: {}（请检查 Data-parameters 仓库是否已拉取）",
            file_name
        )
    })?;
    std::fs::read_to_string(&path)
        .map_err(|e| format!("读取 catalog 文件失败 {}: {}", path.display(), e))
}

pub fn resolve_games_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    for root in data_parameter_roots() {
        let games_dir = root.join("games");
        if games_dir.exists() {
            push_unique_path(&mut dirs, &mut seen, games_dir);
        }
    }

    // 只要 Data-parameters/games 可用，就将其作为唯一权威来源；
    // 避免旧版内置 resources/Games 混入过期游戏目录（如已下线条目）。
    if !dirs.is_empty() {
        return dirs;
    }

    // 兼容旧内置路径：resources/Games
    for root in legacy_resource_roots() {
        let games_dir = root.join("Games");
        if games_dir.exists() {
            push_unique_path(&mut dirs, &mut seen, games_dir);
        }
    }

    dirs
}
