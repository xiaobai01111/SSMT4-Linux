use crate::commands;

const COMMANDS: &[&str] = &[
    "load_settings",
    "save_settings",
    "get_version_check_info",
    "get_resource_version_info",
    "pull_resource_updates",
    "get_xxmi_package_sources",
    "scan_local_xxmi_packages",
    "fetch_xxmi_remote_versions",
    "download_xxmi_package",
    "deploy_xxmi_package",
    "delete_local_xxmi_package",
];

pub fn matches(command: &str) -> bool {
    COMMANDS.contains(&command)
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::settings::load_settings,
        commands::settings::save_settings,
        commands::settings::get_version_check_info,
        commands::settings::get_resource_version_info,
        commands::settings::pull_resource_updates,
        commands::settings::get_xxmi_package_sources,
        commands::settings::scan_local_xxmi_packages,
        commands::settings::fetch_xxmi_remote_versions,
        commands::settings::download_xxmi_package,
        commands::settings::deploy_xxmi_package,
        commands::settings::delete_local_xxmi_package,
    ]
}
