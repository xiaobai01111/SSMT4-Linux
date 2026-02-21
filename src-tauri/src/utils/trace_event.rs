/// 统一日志分级约定：
/// - info：正常流程（开始、完成、状态变更）
/// - warn：可恢复异常（回退、跳过、兼容处理）
/// - error：失败流程（操作中断、需要用户处理）
///
/// 所有事件统一包含 `scope` 与 `event` 字段，便于日志检索与聚合分析。
/// 事件命名建议：
/// - 默认：`<module_path>.info|warn|error`（由宏自动生成）
/// - 显式：`domain.action[_result]`，例如 `launch.spawned` / `download.cancel_requested`
#[inline]
pub fn info(scope: &'static str, event: &'static str, message: impl AsRef<str>) {
    tracing::info!(
        scope = scope,
        event = event,
        message = %message.as_ref(),
        "{}",
        message.as_ref()
    );
}

#[inline]
pub fn warn(scope: &'static str, event: &'static str, message: impl AsRef<str>) {
    tracing::warn!(
        scope = scope,
        event = event,
        message = %message.as_ref(),
        "{}",
        message.as_ref()
    );
}

#[inline]
pub fn error(scope: &'static str, event: &'static str, message: impl AsRef<str>) {
    tracing::error!(
        scope = scope,
        event = event,
        message = %message.as_ref(),
        "{}",
        message.as_ref()
    );
}

#[macro_export]
macro_rules! log_info {
    (event: $event:expr, $($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::info!(
            scope = module_path!(),
            event = $event,
            message = %__msg,
            "{}",
            __msg
        );
    }};
    ($($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::info!(
            scope = module_path!(),
            event = concat!(module_path!(), ".info"),
            message = %__msg,
            "{}",
            __msg
        );
    }};
}

#[macro_export]
macro_rules! log_warn {
    (event: $event:expr, $($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::warn!(
            scope = module_path!(),
            event = $event,
            message = %__msg,
            "{}",
            __msg
        );
    }};
    ($($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::warn!(
            scope = module_path!(),
            event = concat!(module_path!(), ".warn"),
            message = %__msg,
            "{}",
            __msg
        );
    }};
}

#[macro_export]
macro_rules! log_error {
    (event: $event:expr, $($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::error!(
            scope = module_path!(),
            event = $event,
            message = %__msg,
            "{}",
            __msg
        );
    }};
    ($($arg:tt)+) => {{
        let __msg = format!($($arg)+);
        tracing::error!(
            scope = module_path!(),
            event = concat!(module_path!(), ".error"),
            message = %__msg,
            "{}",
            __msg
        );
    }};
}
