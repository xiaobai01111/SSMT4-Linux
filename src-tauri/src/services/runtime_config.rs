use crate::configs::app_config::AppConfig;
use std::sync::Mutex;
use tauri::State;

pub fn snapshot(config: &Mutex<AppConfig>) -> Result<AppConfig, String> {
    config
        .lock()
        .map(|state| state.clone())
        .map_err(|e| e.to_string())
}

pub fn replace(config: &Mutex<AppConfig>, next: AppConfig) -> Result<(), String> {
    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = next;
    Ok(())
}

pub fn view<T>(config: &Mutex<AppConfig>, f: impl FnOnce(&AppConfig) -> T) -> Result<T, String> {
    let state = config.lock().map_err(|e| e.to_string())?;
    Ok(f(&state))
}

pub fn state_snapshot(state: &State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    snapshot(state.inner())
}

pub fn state_view<T>(
    state: &State<'_, Mutex<AppConfig>>,
    f: impl FnOnce(&AppConfig) -> T,
) -> Result<T, String> {
    view(state.inner(), f)
}
