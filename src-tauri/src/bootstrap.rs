use crate::configs;
use crate::utils;
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

    // 1. 运行时 AppConfig 已在 lib.rs 启动 Builder 前完成统一加载并注入 state。
    //    真相源是 SQLite 中的 AppConfig；旧 bootstrap/settings 文件只在缺少 SQLite 记录时参与迁移。
    let config_dir_boot = configs::app_config::get_app_config_dir();
    utils::file_manager::ensure_dir(&config_dir_boot)
        .map_err(|e| format!("创建配置目录失败 {}: {}", config_dir_boot.display(), e))?;

    // 2. 创建固定目录（config、cache、prefixes — 不受 dataDir 影响）
    //    这些是关键目录，失败则终止启动
    let config_dir = configs::app_config::get_app_config_dir();
    let cache_dir = configs::app_config::get_app_cache_dir();
    let prefixes_dir = utils::file_manager::get_prefixes_dir();

    for dir in [&config_dir, &cache_dir, &prefixes_dir] {
        utils::file_manager::ensure_dir(dir)
            .map_err(|e| format!("创建关键目录失败 {}: {}", dir.display(), e))?;
    }

    // 3. data-linux（旧名 Data-parameters）仓库同步改为显式任务（pull_resource_updates），
    //    启动阶段不再隐式 clone/pull 外部仓库。
    //
    // 4. 符号链接和 Games 目录不在启动时创建
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

#[cfg(test)]
mod tests {
    #[test]
    fn dependency_probe_functions_are_linked() {
        let _schedule: fn() = super::schedule_dependency_status_probe;
        let _log: fn() = super::log_dependency_status;
    }
}
