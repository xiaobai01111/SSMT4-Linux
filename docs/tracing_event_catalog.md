# Tracing Event Catalog

## Event naming

- Default event (all logs): `<module_path>.info|warn|error`
- Explicit event (key flows): `domain.action[_result]`

## Explicit events in code

### Launch
- `launch.already_running`
- `launch.plan`
- `launch.spawned`
- `launch.child_exit`
- `launch.wait_failed`
- `launch.monitor_started`
- `launch.processes_exited`
- `launch.monitor_timeout`
- `launch.processes_still_running`

### Protection / Channel
- `protection.preset_fallback`
- `protection.applied`
- `channel.mode_applied`
- `channel.mode_already_target`
- `telemetry.file_removed`
- `telemetry.file_remove_failed`

### Runtime wrappers / performance
- `runtime.gamemode_enabled`
- `runtime.gamemode_missing`
- `runtime.cpu_limit_enabled`
- `runtime.cpu_limit_missing`
- `runtime.power_profile_tool_missing`
- `runtime.power_profile_switched`
- `runtime.power_profile_switch_failed`
- `runtime.power_profile_restored`
- `runtime.power_profile_restore_failed`

### Downloader (command layer)
- `state.fetch_launcher_info_failed`
- `installer.fetch_remote_failed`
- `state.snowbreak_api_failed`
- `state.hoyoverse_api_failed`
- `download.completed`
- `download.cancel_requested`
- `update.completed`
- `update.patch_completed`
- `verify.partial_failed`

### Downloader (HoYoverse core)
- `verify.started`
- `verify.unsafe_path_skipped`
- `verify.hash_mismatch`
- `verify.completed`
- `download.file_started`
- `download.file_retry`
- `download.file_retry_succeeded`
- `download.file_completed`
- `install.phase_started`
- `install.primary_commit_failed`
- `install.primary_failed`
- `install.language_commit_failed`
- `install.language_failed`
- `install.full_completed`
- `install.incremental_completed`
