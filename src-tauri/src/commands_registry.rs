mod common;
mod diagnostics;
mod games;
mod platform;
mod settings;

/// 生成 Tauri invoke_handler，集中注册所有前端可调用的命令
pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    let common_handler = common::handler();
    let settings_handler = settings::handler();
    let games_handler = games::handler();
    let platform_handler = platform::handler();
    let diagnostics_handler = diagnostics::handler();

    move |invoke| {
        let command = invoke.message.command().to_string();

        if common::matches(&command) {
            common_handler(invoke)
        } else if settings::matches(&command) {
            settings_handler(invoke)
        } else if games::matches(&command) {
            games_handler(invoke)
        } else if platform::matches(&command) {
            platform_handler(invoke)
        } else if diagnostics::matches(&command) {
            diagnostics_handler(invoke)
        } else {
            false
        }
    }
}
