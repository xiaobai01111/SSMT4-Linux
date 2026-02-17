use crate::configs;
use crate::utils;

/// 应用启动初始化：数据目录、符号链接、固定目录创建
///
/// 关键目录创建失败会直接终止启动（返回 Err），避免进入半初始化状态。
pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 从 SQLite 读取 dataDir（优先），回退到 settings.json 兼容旧数据
    let config_dir_boot = configs::app_config::get_app_config_dir();
    utils::file_manager::ensure_dir(&config_dir_boot)
        .map_err(|e| format!("创建配置目录失败 {}: {}", config_dir_boot.display(), e))?;

    // 先尝试 SQLite
    let data_dir_value = configs::database::get_setting("data_dir");

    // 回退到 settings.json
    let data_dir_str = data_dir_value.unwrap_or_else(|| {
        let settings_path = config_dir_boot.join("settings.json");
        if settings_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&settings_path) {
                if let Ok(cfg) = serde_json::from_str::<configs::app_config::AppConfig>(&content) {
                    return cfg.data_dir;
                }
            }
        }
        String::new()
    });

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

    // 3. 符号链接和 Games 目录不在启动时创建
    //    仅在用户显式保存设置（save_settings）时才创建/更新符号链接
    //    这样新用户首次启动不会产生未经确认的符号链接

    let data_dir = configs::app_config::get_app_data_dir();
    tracing::info!("Config dir: {}", config_dir.display());
    tracing::info!("Data dir: {}", data_dir.display());
    tracing::info!("Cache dir: {}", cache_dir.display());

    Ok(())
}
