use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as AsyncMutex;

/// 按前端 task_id 管理取消令牌，避免并行任务互相干扰。
static CANCEL_TOKENS: once_cell::sync::Lazy<StdMutex<HashMap<String, Arc<AsyncMutex<bool>>>>> =
    once_cell::sync::Lazy::new(|| StdMutex::new(HashMap::new()));

/// 获取指定任务的取消令牌（每次创建全新令牌，避免旧状态污染）。
pub(crate) fn get_cancel_token(task_id: &str) -> Arc<AsyncMutex<bool>> {
    let mut tokens = CANCEL_TOKENS.lock().unwrap();
    let token = Arc::new(AsyncMutex::new(false));
    tokens.insert(task_id.to_string(), token.clone());
    token
}

/// 清理已完成任务的令牌。
pub(crate) fn cleanup_cancel_token(task_id: &str) {
    if let Ok(mut tokens) = CANCEL_TOKENS.lock() {
        tokens.remove(task_id);
    }
}

pub(crate) async fn request_cancel(task_id: Option<&str>) -> Vec<String> {
    let targets: Vec<(String, Arc<AsyncMutex<bool>>)> = {
        let tokens = CANCEL_TOKENS.lock().unwrap();
        if let Some(id) = task_id {
            tokens
                .get(id)
                .map(|token| vec![(id.to_string(), token.clone())])
                .unwrap_or_default()
        } else {
            tokens
                .iter()
                .map(|(id, token)| (id.clone(), token.clone()))
                .collect()
        }
    };

    let mut cancelled = Vec::with_capacity(targets.len());
    for (id, token) in targets {
        *token.lock().await = true;
        cancelled.push(id);
    }

    cancelled
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_task_id(label: &str) -> String {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        format!("cancel-{label}-{nonce}")
    }

    #[tokio::test]
    async fn get_cancel_token_replaces_existing_token_state() {
        let _guard = TEST_GUARD.lock().unwrap();
        let task_id = unique_task_id("replace");

        let first = get_cancel_token(&task_id);
        *first.lock().await = true;

        let second = get_cancel_token(&task_id);
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(!(*second.lock().await));

        cleanup_cancel_token(&task_id);
    }

    #[tokio::test]
    async fn request_cancel_targets_specific_task_only() {
        let _guard = TEST_GUARD.lock().unwrap();
        let task_a = unique_task_id("specific-a");
        let task_b = unique_task_id("specific-b");

        let token_a = get_cancel_token(&task_a);
        let token_b = get_cancel_token(&task_b);

        let cancelled = request_cancel(Some(&task_a)).await;
        assert_eq!(cancelled, vec![task_a.clone()]);
        assert!(*token_a.lock().await);
        assert!(!(*token_b.lock().await));

        cleanup_cancel_token(&task_a);
        cleanup_cancel_token(&task_b);
    }

    #[tokio::test]
    async fn request_cancel_without_task_id_cancels_all_tasks() {
        let _guard = TEST_GUARD.lock().unwrap();
        let task_a = unique_task_id("all-a");
        let task_b = unique_task_id("all-b");

        let token_a = get_cancel_token(&task_a);
        let token_b = get_cancel_token(&task_b);

        let mut cancelled = request_cancel(None).await;
        cancelled.sort();
        let mut expected = vec![task_a.clone(), task_b.clone()];
        expected.sort();
        assert_eq!(cancelled, expected);
        assert!(*token_a.lock().await);
        assert!(*token_b.lock().await);

        cleanup_cancel_token(&task_a);
        cleanup_cancel_token(&task_b);
    }
}
