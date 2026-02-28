use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::battery::{BatterySample, BatteryStatus};

/// A single charge session: from start of charging to reaching 90%+
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargeSession {
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub start_capacity: f64,
    pub end_capacity: f64,
    pub samples: Vec<BatterySample>,
    pub completed: bool, // reached 90%+
}

/// Persistent history storage
#[derive(Debug, Serialize, Deserialize)]
pub struct History {
    /// All samples in current monitoring session
    pub samples: Vec<BatterySample>,
    /// Last 2 completed charge sessions (reached 90%+)
    pub charge_sessions: Vec<ChargeSession>,
    /// Currently active charge session (if charging)
    #[serde(skip)]
    pub active_session: Option<ChargeSession>,
}

impl History {
    pub fn new() -> Self {
        History {
            samples: Vec::new(),
            charge_sessions: Vec::new(),
            active_session: None,
        }
    }

    /// Load history from disk, or create new if not found
    pub fn load() -> Self {
        let path = Self::data_path();
        if path.exists()
            && let Ok(data) = fs::read_to_string(&path)
            && let Ok(history) = serde_json::from_str::<History>(&data)
        {
            return history;
        }
        Self::new()
    }

    /// Save history to disk
    pub fn save(&self) {
        let path = Self::data_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, data);
        }
    }

    fn data_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bathis")
            .join("history.json")
    }

    /// Add a new sample and update charge session tracking
    pub fn add_sample(&mut self, sample: BatterySample) {
        // Track charge sessions
        match sample.status {
            BatteryStatus::Charging => {
                if self.active_session.is_none() {
                    // Start a new charge session
                    self.active_session = Some(ChargeSession {
                        start_time: sample.timestamp,
                        end_time: None,
                        start_capacity: sample.capacity,
                        end_capacity: sample.capacity,
                        samples: vec![sample.clone()],
                        completed: false,
                    });
                } else if let Some(ref mut session) = self.active_session {
                    session.end_capacity = sample.capacity;
                    session.end_time = Some(sample.timestamp);
                    session.samples.push(sample.clone());

                    // Check if reached 90%+
                    if sample.capacity >= 90.0 && !session.completed {
                        session.completed = true;
                    }
                }
            }
            _ => {
                // Not charging — close active session if exists
                if let Some(mut session) = self.active_session.take() {
                    session.end_time = Some(sample.timestamp);
                    if session.completed {
                        self.charge_sessions.push(session);
                        // Keep only last 2 completed sessions
                        while self.charge_sessions.len() > 2 {
                            self.charge_sessions.remove(0);
                        }
                    }
                    // If not completed (didn't reach 90%), just discard
                }
            }
        }

        self.samples.push(sample);

        // Limit total sample count to avoid unbounded growth
        // Keep last ~48h at 5s intervals = ~34560 samples
        const MAX_SAMPLES: usize = 40000;
        if self.samples.len() > MAX_SAMPLES {
            let drain_count = self.samples.len() - MAX_SAMPLES;
            self.samples.drain(..drain_count);
        }
    }

    /// Get all samples for display (including current + loaded history)
    pub fn all_samples(&self) -> &[BatterySample] {
        &self.samples
    }

    /// Get charge sessions for display
    pub fn completed_sessions(&self) -> &[ChargeSession] {
        &self.charge_sessions
    }
}
