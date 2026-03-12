use crate::configs::app_config::{self, AppConfig};

/// 根据 AppConfig.data_dir 同步运行时 dataDir 镜像，并显式处理文件系统副作用。
/// 持久化真相源始终是 AppConfig.data_dir；这里只负责将其投影到当前进程和符号链接状态。
pub(super) fn sync_runtime_data_dir_and_filesystem(cfg: &AppConfig) {
    if cfg.data_dir.is_empty() {
        app_config::apply_runtime_data_dir_override("");
        crate::utils::file_manager::remove_data_dir_symlink();
    } else {
        let dir = app_config::expand_user_path(&cfg.data_dir);
        app_config::apply_runtime_data_dir_override(&cfg.data_dir);

        // 创建符号链接：~/.local/share/ssmt4 -> 自定义目录
        if let Err(e) = crate::utils::file_manager::setup_data_dir_symlink(&dir) {
            tracing::error!("设置数据目录符号链接失败: {}", e);
        }

        // 创建 Games 子目录
        let games_dir = crate::utils::file_manager::get_global_games_dir();
        crate::utils::file_manager::ensure_dir(&games_dir).ok();
    }
    tracing::info!("数据目录: {}", app_config::get_app_data_dir().display());
}
