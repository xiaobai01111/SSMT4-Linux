use crate::utils::data_parameters::{DATA_PARAMETERS_REPO_PAGES, DATA_PARAMETERS_VERSION_MIRRORS};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionCheckInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub update_log: String,
}

const APP_VERSION_REMOTE_SOURCES: &[(&str, &str, &str)] = &[
    (
        "GitHub",
        "https://raw.githubusercontent.com/peachycommit/ssmt4-linux/main/version",
        "https://raw.githubusercontent.com/peachycommit/ssmt4-linux/main/version-log",
    ),
    (
        "Gitee",
        "https://gitee.com/xiaobai01111/ssmt4-linux/raw/master/version",
        "https://gitee.com/xiaobai01111/ssmt4-linux/raw/master/version-log",
    ),
    (
        "Gitee (main)",
        "https://gitee.com/xiaobai01111/ssmt4-linux/raw/main/version",
        "https://gitee.com/xiaobai01111/ssmt4-linux/raw/main/version-log",
    ),
];

const APP_REPO_PAGES: &[(&str, &str)] = &[
    ("GitHub", "https://github.com/peachycommit/ssmt4-linux"),
    ("Gitee", "https://gitee.com/xiaobai01111/ssmt4-linux"),
];

fn read_trimmed_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_data_parameters_local_version() -> Option<(String, String)> {
    let path = crate::utils::data_parameters::resolve_data_path("version")?;
    let version = read_trimmed_file(&path)?;
    Some((version, path.to_string_lossy().to_string()))
}

fn read_raw_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|existing| existing == &candidate) {
        paths.push(candidate);
    }
}

fn build_version_file_search_bases(
    resource_dir: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut bases = Vec::<PathBuf>::new();

    if let Some(resource_dir) = resource_dir {
        push_unique_path(&mut bases, resource_dir);
    }

    if let Some(exe_dir) = exe_dir {
        push_unique_path(&mut bases, exe_dir);
    }

    #[cfg(debug_assertions)]
    if let Some(source_root) = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
    {
        push_unique_path(&mut bases, source_root);
    }

    bases
}

fn resolve_version_file_paths(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let resource_dir = app.path().resource_dir().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()));

    build_version_file_search_bases(resource_dir, exe_dir)
}

fn build_local_version_snapshot(app: &tauri::AppHandle) -> (String, String, Vec<String>) {
    let mut latest_version = String::new();
    let mut update_log = String::new();
    let mut notes = Vec::new();

    for base in resolve_version_file_paths(app) {
        if latest_version.is_empty() {
            let candidate = base.join("version");
            if let Some(v) = read_trimmed_file(&candidate) {
                notes.push(format!("本地 version 来源: {}", candidate.display()));
                latest_version = v;
            }
        }
        if update_log.is_empty() {
            let candidate = base.join("version-log");
            if let Some(v) = read_raw_file(&candidate) {
                notes.push(format!("本地 version-log 来源: {}", candidate.display()));
                update_log = v;
            }
        }
        if !latest_version.is_empty() && !update_log.is_empty() {
            break;
        }
    }

    (latest_version, update_log, notes)
}

fn prepend_update_log_notes(update_log: &str, notes: &[String]) -> String {
    let mut sections = Vec::new();
    if !notes.is_empty() {
        sections.push(notes.join("\n"));
    }
    if !update_log.trim().is_empty() {
        sections.push(update_log.trim().to_string());
    }
    sections.join("\n\n")
}

async fn fetch_remote_text(
    client: &reqwest::Client,
    label: &str,
    url: &str,
    kind: &str,
) -> Result<String, String> {
    let resp = client
        .get(url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("{} {} 请求失败: {}", label, kind, e))?;

    if !resp.status().is_success() {
        return Err(format!("{} {} 返回 HTTP {}", label, kind, resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("{} {} 读取内容失败: {}", label, kind, e))?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(format!("{} {} 返回内容为空", label, kind));
    }
    Ok(text)
}

async fn fetch_remote_data_parameters_version() -> Result<(String, String), String> {
    let client = reqwest::Client::new();
    let mut errors = Vec::new();

    for (label, url) in DATA_PARAMETERS_VERSION_MIRRORS {
        match fetch_remote_text(&client, label, url, "version").await {
            Ok(version) => return Ok((version, (*label).to_string())),
            Err(e) => errors.push(e),
        }
    }

    Err(format!("远程资源版本检查全部失败: {}", errors.join(" | ")))
}

async fn fetch_remote_app_version_info() -> Result<(String, String, String), String> {
    let client = reqwest::Client::new();
    let mut errors = Vec::new();

    for (label, version_url, log_url) in APP_VERSION_REMOTE_SOURCES {
        let version = match fetch_remote_text(&client, label, version_url, "version").await {
            Ok(version) => version,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        let update_log = match fetch_remote_text(&client, label, log_url, "version-log").await {
            Ok(update_log) => update_log,
            Err(err) => {
                errors.push(err);
                String::new()
            }
        };

        return Ok((version, update_log, (*label).to_string()));
    }

    Err(format!("远程系统版本检查全部失败: {}", errors.join(" | ")))
}

pub(super) async fn get_version_check_info(
    app: tauri::AppHandle,
) -> Result<VersionCheckInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let (local_latest_version, local_update_log, local_notes) = build_local_version_snapshot(&app);

    let (latest_version, update_log) = match fetch_remote_app_version_info().await {
        Ok((remote_version, remote_update_log, source)) => {
            let mut notes = vec![format!("远程版本源: {}", source)];
            for (label, url) in APP_REPO_PAGES {
                notes.push(format!("项目仓库 {}: {}", label, url));
            }
            (
                remote_version,
                prepend_update_log_notes(&remote_update_log, &notes),
            )
        }
        Err(err) => {
            let fallback_version = if local_latest_version.is_empty() {
                current_version.clone()
            } else {
                local_latest_version
            };
            let mut notes = vec![format!("远程检查失败: {}", err)];
            notes.extend(local_notes);
            (
                fallback_version,
                prepend_update_log_notes(&local_update_log, &notes),
            )
        }
    };

    let has_update = latest_version != current_version;

    Ok(VersionCheckInfo {
        current_version,
        latest_version,
        has_update,
        update_log,
    })
}

pub(super) async fn get_resource_version_info() -> Result<VersionCheckInfo, String> {
    let mut notes = Vec::new();
    for (label, url) in DATA_PARAMETERS_REPO_PAGES {
        notes.push(format!("资源仓库 {}: {}", label, url));
    }

    let (current_version, local_path_note) = match read_data_parameters_local_version() {
        Some((version, path)) => (version, Some(path)),
        None => ("unknown".to_string(), None),
    };

    if let Some(path) = local_path_note {
        notes.push(format!("本地版本文件: {}", path));
    } else {
        notes.push("本地版本文件: 未找到 Data-parameters/version".to_string());
    }

    let latest_version = match fetch_remote_data_parameters_version().await {
        Ok((v, source)) => {
            notes.push(format!("远程版本源: {}", source));
            v
        }
        Err(e) => {
            notes.push(format!("远程检查失败: {}", e));
            current_version.clone()
        }
    };

    let has_update = current_version != "unknown"
        && latest_version != "unknown"
        && latest_version != current_version;

    Ok(VersionCheckInfo {
        current_version,
        latest_version,
        has_update,
        update_log: notes.join("\n"),
    })
}

pub(super) fn pull_resource_updates() -> Result<String, String> {
    crate::utils::data_parameters::sync_managed_repo()?;
    let version = read_data_parameters_local_version()
        .map(|(v, _)| v)
        .unwrap_or_else(|| "unknown".to_string());
    Ok(format!("资源更新完成，本地版本: {}", version))
}

#[cfg(test)]
mod tests {
    use super::{build_version_file_search_bases, prepend_update_log_notes};
    use std::path::PathBuf;

    #[test]
    fn version_file_paths_prefer_packaged_locations() {
        let resource_dir = PathBuf::from("/tmp/app/resources");
        let exe_dir = PathBuf::from("/tmp/app/bin");
        let bases =
            build_version_file_search_bases(Some(resource_dir.clone()), Some(exe_dir.clone()));

        assert_eq!(bases[0], resource_dir);
        assert_eq!(bases[1], exe_dir);

        #[cfg(debug_assertions)]
        {
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("source root")
                .to_path_buf();
            assert_eq!(bases[2], source_root);
        }
    }

    #[test]
    fn version_file_paths_deduplicate_packaged_locations() {
        let shared = PathBuf::from("/tmp/app/shared");
        let bases = build_version_file_search_bases(Some(shared.clone()), Some(shared.clone()));

        assert_eq!(bases[0], shared);
        assert_eq!(bases.iter().filter(|path| *path == &shared).count(), 1);
    }

    #[test]
    fn prepend_update_log_notes_joins_sections_without_extra_blank_blocks() {
        let combined = prepend_update_log_notes(
            "  line1\nline2  ",
            &["source: GitHub".to_string(), "repo: mirror".to_string()],
        );
        assert_eq!(combined, "source: GitHub\nrepo: mirror\n\nline1\nline2");

        assert_eq!(
            prepend_update_log_notes("", &["only notes".to_string()]),
            "only notes"
        );
        assert_eq!(prepend_update_log_notes("only log", &[]), "only log");
    }
}
