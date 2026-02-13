use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            phase: String::new(),
            total_size: 0,
            finished_size: 0,
            total_count: 0,
            finished_count: 0,
            current_file: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        }
    }
}

pub struct SpeedTracker {
    samples: Vec<(std::time::Instant, u64)>,
    window: std::time::Duration,
}

impl SpeedTracker {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            window: std::time::Duration::from_secs(5),
        }
    }

    pub fn record(&mut self, bytes: u64) {
        let now = std::time::Instant::now();
        self.samples.push((now, bytes));
        self.samples.retain(|&(t, _)| now.duration_since(t) < self.window);
    }

    pub fn speed_bps(&self) -> u64 {
        if self.samples.len() < 2 {
            return 0;
        }
        let first = self.samples.first().unwrap();
        let last = self.samples.last().unwrap();
        let elapsed = last.0.duration_since(first.0).as_secs_f64();
        if elapsed < 0.001 {
            return 0;
        }
        let total_bytes: u64 = self.samples.iter().map(|(_, b)| b).sum();
        (total_bytes as f64 / elapsed) as u64
    }

    pub fn eta_seconds(&self, remaining: u64) -> u64 {
        let speed = self.speed_bps();
        if speed == 0 {
            return 0;
        }
        remaining / speed
    }
}
