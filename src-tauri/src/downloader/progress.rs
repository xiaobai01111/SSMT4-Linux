use crate::events::{
    emit_game_download_progress, GameDownloadOperation, GameDownloadProgressEvent,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LauncherState {
    StartGame,
    GameRunning,
    NeedInstall,
    Downloading,
    Validating,
    NeedUpdate,
    Updating,
    Merging,
    NetworkError,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub phase: String,
    pub total_size: u64,
    pub finished_size: u64,
    pub total_count: usize,
    pub finished_count: usize,
    pub current_file: String,
    pub speed_bps: u64,
    pub eta_seconds: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DownloadProgressStats {
    pub total_size: u64,
    pub finished_size: u64,
    pub total_count: usize,
    pub finished_count: usize,
    pub speed_bps: u64,
    pub eta_seconds: u64,
}

impl DownloadProgress {
    pub fn phase(phase: impl Into<String>) -> Self {
        Self {
            phase: phase.into(),
            ..Self::default()
        }
    }

    pub fn with_sizes(mut self, finished_size: u64, total_size: u64) -> Self {
        self.finished_size = finished_size;
        self.total_size = total_size;
        self
    }

    pub fn with_counts(mut self, finished_count: usize, total_count: usize) -> Self {
        self.finished_count = finished_count;
        self.total_count = total_count;
        self
    }

    pub fn with_current_file(mut self, current_file: impl Into<String>) -> Self {
        self.current_file = current_file.into();
        self
    }

    pub fn with_transfer(mut self, speed_bps: u64, eta_seconds: u64) -> Self {
        self.speed_bps = speed_bps;
        self.eta_seconds = eta_seconds;
        self
    }
}

pub fn emit_game_download_snapshot(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    phase: &str,
    current_file: impl Into<String>,
    stats: DownloadProgressStats,
) {
    let progress = DownloadProgress::phase(phase)
        .with_sizes(stats.finished_size, stats.total_size)
        .with_counts(stats.finished_count, stats.total_count)
        .with_current_file(current_file)
        .with_transfer(stats.speed_bps, stats.eta_seconds);
    emit_game_download_progress(
        app,
        &GameDownloadProgressEvent::from_progress(task_id, operation, &progress),
    );
}

pub fn emit_game_download_state(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    progress: &DownloadProgress,
) {
    emit_game_download_progress(
        app,
        &GameDownloadProgressEvent::from_progress(task_id, operation, progress),
    );
}

/// 速度统计器：固定槽位环形缓冲区，O(1) record / O(1) speed_bps
///
/// 将 5 秒窗口分成 SLOT_COUNT 个槽（每槽 200ms），
/// 每个槽累计该时段的字节数。record 时只更新当前槽并推进指针，
/// speed_bps 直接用 window_bytes / window_elapsed 计算。
pub struct SpeedTracker {
    /// 环形槽：每槽存储该时段累计字节数
    slots: [u64; Self::SLOT_COUNT],
    /// 当前槽索引
    head: usize,
    /// 当前槽的起始时间
    slot_start: std::time::Instant,
    /// 窗口内字节总和（= slots 之和，增量维护）
    window_bytes: u64,
    /// 全局起始时间（兜底用）
    start_time: std::time::Instant,
    /// 全局累计字节
    total_bytes: u64,
}

impl SpeedTracker {
    /// 窗口总时长 5s，每槽 200ms → 25 槽
    const SLOT_COUNT: usize = 25;
    const SLOT_DURATION: std::time::Duration = std::time::Duration::from_millis(200);

    pub fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            slots: [0; Self::SLOT_COUNT],
            head: 0,
            slot_start: now,
            window_bytes: 0,
            start_time: now,
            total_bytes: 0,
        }
    }

    /// 记录新字节（O(1)）
    pub fn record(&mut self, bytes: u64) {
        self.total_bytes += bytes;
        self.advance_slots();
        self.slots[self.head] += bytes;
        self.window_bytes += bytes;
    }

    /// 当前速度（字节/秒，O(1)）
    pub fn speed_bps(&mut self) -> u64 {
        self.advance_slots();

        // 滑动窗口速度
        let window_secs = (Self::SLOT_COUNT as f64) * Self::SLOT_DURATION.as_secs_f64();
        if self.window_bytes > 0 {
            let speed = self.window_bytes as f64 / window_secs;
            if speed >= 1.0 {
                return speed as u64;
            }
        }

        // 兜底：全局累计速度
        let global_elapsed = self.start_time.elapsed().as_secs_f64();
        if global_elapsed >= 0.5 && self.total_bytes > 0 {
            return (self.total_bytes as f64 / global_elapsed) as u64;
        }

        0
    }

    pub fn eta_seconds(&mut self, remaining: u64) -> u64 {
        let speed = self.speed_bps();
        if speed == 0 {
            return 0;
        }
        remaining / speed
    }

    /// 推进槽位：过期的槽从 window_bytes 中减去并清零
    fn advance_slots(&mut self) {
        let elapsed = self.slot_start.elapsed();
        let slots_to_advance = (elapsed.as_millis() / Self::SLOT_DURATION.as_millis()) as usize;

        if slots_to_advance == 0 {
            return;
        }

        // 最多推进 SLOT_COUNT 个（超过即整个窗口已过期）
        let advance = slots_to_advance.min(Self::SLOT_COUNT);
        for _ in 0..advance {
            self.head = (self.head + 1) % Self::SLOT_COUNT;
            self.window_bytes = self.window_bytes.saturating_sub(self.slots[self.head]);
            self.slots[self.head] = 0;
        }
        self.slot_start += Self::SLOT_DURATION * advance as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn download_progress_builder_sets_expected_fields() {
        let progress = DownloadProgress::phase("verify")
            .with_sizes(128, 1024)
            .with_counts(2, 8)
            .with_current_file("data_01.zip")
            .with_transfer(512, 7);

        assert_eq!(progress.phase, "verify");
        assert_eq!(progress.finished_size, 128);
        assert_eq!(progress.total_size, 1024);
        assert_eq!(progress.finished_count, 2);
        assert_eq!(progress.total_count, 8);
        assert_eq!(progress.current_file, "data_01.zip");
        assert_eq!(progress.speed_bps, 512);
        assert_eq!(progress.eta_seconds, 7);
    }

    #[test]
    fn speed_tracker_reports_speed_and_eta_from_window_bytes() {
        let mut tracker = SpeedTracker::new();
        tracker.record(10_000);

        assert_eq!(tracker.speed_bps(), 2_000);
        assert_eq!(tracker.eta_seconds(4_000), 2);
    }

    #[test]
    fn speed_tracker_discards_stale_window_when_all_slots_expire() {
        let mut tracker = SpeedTracker::new();
        tracker.record(10_000);
        tracker.slot_start = Instant::now() - Duration::from_secs(10);
        tracker.start_time = Instant::now();

        assert_eq!(tracker.speed_bps(), 0);
        assert_eq!(tracker.window_bytes, 0);
    }

    #[test]
    fn speed_tracker_uses_global_fallback_when_window_speed_is_zero() {
        let mut tracker = SpeedTracker::new();
        tracker.start_time = Instant::now() - Duration::from_secs(2);
        tracker.total_bytes = 4_000;
        tracker.window_bytes = 0;

        let speed = tracker.speed_bps();
        assert!((1_900..=2_000).contains(&speed));
        assert_eq!(tracker.eta_seconds(4_000), 4_000 / speed);
    }

    #[test]
    fn speed_tracker_partial_slot_advance_keeps_recent_bytes() {
        let mut tracker = SpeedTracker::new();
        tracker.record(2_000);
        tracker.slot_start = Instant::now() - (SpeedTracker::SLOT_DURATION * 2);

        let speed = tracker.speed_bps();
        assert_eq!(speed, 400);
        assert_eq!(tracker.window_bytes, 2_000);
    }
}
