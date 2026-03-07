use crate::configs;
use crate::utils;
use std::path::Path;
use tauri::Manager;

/// 应用启动初始化：数据目录、符号链接、固定目录创建
///
/// 关键目录创建失败会直接终止启动（返回 Err），避免进入半初始化状态。
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        tracing::info!("资源目录: {}", resource_dir.display());
        utils::data_parameters::set_resource_dir(resource_dir);
    } else {
        tracing::warn!("无法获取 resource_dir，后续将使用回退路径解析资源");
    }

    // 1. 从轻量引导缓存恢复 dataDir；缺失时轻量读取 SQLite，最后回退到旧 settings.json。
    let config_dir_boot = configs::app_config::get_app_config_dir();
    utils::file_manager::ensure_dir(&config_dir_boot)
        .map_err(|e| format!("创建配置目录失败 {}: {}", config_dir_boot.display(), e))?;

    let data_dir_str = resolve_bootstrap_data_dir(&config_dir_boot).unwrap_or_default();

    if !data_dir_str.is_empty() {
        let expanded = configs::app_config::expand_user_path(&data_dir_str);
        configs::app_config::set_custom_data_dir(expanded);
    }

    // 2. 创建固定目录（config、cache、prefixes — 不受 dataDir 影响）
    //    这些是关键目录，失败则终止启动
    let config_dir = configs::app_config::get_app_config_dir();
    let cache_dir = configs::app_config::get_app_cache_dir();
    let prefixes_dir = utils::file_manager::get_prefixes_dir();

    for dir in [&config_dir, &cache_dir, &prefixes_dir] {
        utils::file_manager::ensure_dir(dir)
            .map_err(|e| format!("创建关键目录失败 {}: {}", dir.display(), e))?;
    }

    utils::data_parameters::schedule_managed_repo_sync();

    // 3. 符号链接和 Games 目录不在启动时创建
    //    仅在用户显式保存设置（save_settings）时才创建/更新符号链接
    //    这样新用户首次启动不会产生未经确认的符号链接

    let data_dir = configs::app_config::get_app_data_dir();
    tracing::info!(
        "启动目录已就绪: 配置={}, 数据={}, 缓存={}, 前缀={}",
        config_dir.display(),
        data_dir.display(),
        cache_dir.display(),
        prefixes_dir.display()
    );
    schedule_dependency_status_probe();

    Ok(())
}

fn resolve_bootstrap_data_dir(config_dir_boot: &Path) -> Option<String> {
    match configs::app_config::read_bootstrap_data_dir() {
        configs::app_config::BootstrapDataDir::Value(value) => return Some(value),
        configs::app_config::BootstrapDataDir::Missing => {}
    }

    if let Some(value) = configs::database::peek_setting("data_dir") {
        return Some(value);
    }

    read_legacy_data_dir(config_dir_boot)
}

fn read_legacy_data_dir(config_dir_boot: &Path) -> Option<String> {
    let settings_path = config_dir_boot.join("settings.json");
    let content = std::fs::read_to_string(settings_path).ok()?;
    let cfg = serde_json::from_str::<configs::app_config::AppConfig>(&content).ok()?;
    Some(cfg.data_dir.trim().to_string())
}

fn schedule_dependency_status_probe() {
    if let Err(err) = std::thread::Builder::new()
        .name("dependency-probe".to_string())
        .spawn(log_dependency_status)
    {
        tracing::warn!("后台依赖探测线程启动失败: {}", err);
    }
}

fn log_dependency_status() {
    // 后台输出关键外部依赖探测结果，避免启动阶段串行 which 检查叠加到首屏路径。
    for dep in ["umu-run", "bwrap", "wine", "wine64", "vulkaninfo"] {
        match which::which(dep) {
            Ok(path) => tracing::info!("依赖可用: {} -> {}", dep, path.display()),
            Err(_) => tracing::warn!("依赖缺失: {}（PATH 未找到）", dep),
        }
    }
}
