use chrono::{DateTime, Local};

use crate::battery::BatterySample;
use crate::history::History;

/// Which view the app is showing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Live dashboard with current stats
    Dashboard,
    /// History chart for all recorded data
    HistoryChart,
    /// Charge session detail view
    SessionDetail(usize), // index into charge_sessions
}

/// Chart viewport for zoom/pan
#[derive(Debug, Clone)]
pub struct ChartViewport {
    /// Start time of visible window (seconds since first sample)
    pub time_start: f64,
    /// End time of visible window (seconds since first sample)
    pub time_end: f64,
    /// Total time range of all data
    pub time_total: f64,
    /// Zoom level (1.0 = show all, 0.1 = show 10%)
    pub zoom: f64,
}

impl ChartViewport {
    pub fn new() -> Self {
        ChartViewport {
            time_start: 0.0,
            time_end: 1.0,
            time_total: 1.0,
            zoom: 1.0,
        }
    }

    /// Update viewport to fit data range
    pub fn fit_data(&mut self, total_seconds: f64) {
        self.time_total = total_seconds.max(1.0);
        self.time_end = self.time_total;
        self.time_start = 0.0;
        self.zoom = 1.0;
    }

    /// Zoom in (show less time range)
    pub fn zoom_in(&mut self) {
        let center = (self.time_start + self.time_end) / 2.0;
        let half_range = (self.time_end - self.time_start) / 2.0 * 0.7;
        self.time_start = (center - half_range).max(0.0);
        self.time_end = (center + half_range).min(self.time_total);
        self.zoom = (self.time_end - self.time_start) / self.time_total;
    }

    /// Zoom out (show more time range)
    pub fn zoom_out(&mut self) {
        let center = (self.time_start + self.time_end) / 2.0;
        let half_range = (self.time_end - self.time_start) / 2.0 / 0.7;
        self.time_start = (center - half_range).max(0.0);
        self.time_end = (center + half_range).min(self.time_total);
        self.zoom = (self.time_end - self.time_start) / self.time_total;
        if self.zoom > 0.99 {
            self.zoom = 1.0;
            self.time_start = 0.0;
            self.time_end = self.time_total;
        }
    }

    /// Pan left (earlier in time)
    pub fn pan_left(&mut self) {
        let range = self.time_end - self.time_start;
        let shift = range * 0.2;
        if self.time_start > 0.0 {
            self.time_start = (self.time_start - shift).max(0.0);
            self.time_end = self.time_start + range;
        }
    }

    /// Pan right (later in time)
    pub fn pan_right(&mut self) {
        let range = self.time_end - self.time_start;
        let shift = range * 0.2;
        if self.time_end < self.time_total {
            self.time_end = (self.time_end + shift).min(self.time_total);
            self.time_start = self.time_end - range;
        }
    }

    /// Visible time range in seconds
    pub fn visible_range(&self) -> (f64, f64) {
        (self.time_start, self.time_end)
    }
}

/// Main application state
pub struct App {
    pub view: View,
    pub history: History,
    pub viewport: ChartViewport,
    pub session_viewport: ChartViewport,
    pub running: bool,
    pub battery_name: String,
    pub last_sample: Option<BatterySample>,
    pub tick_count: u64,
    /// Reference time for converting DateTime to chart x-axis
    pub ref_time: Option<DateTime<Local>>,
}

impl App {
    pub fn new(battery_name: String) -> Self {
        let history = History::load();
        let ref_time = history.samples.first().map(|s| s.timestamp);

        App {
            view: View::Dashboard,
            history,
            viewport: ChartViewport::new(),
            session_viewport: ChartViewport::new(),
            running: true,
            battery_name,
            last_sample: None,
            tick_count: 0,
            ref_time,
        }
    }

    /// Add a new battery sample
    pub fn add_sample(&mut self, sample: BatterySample) {
        if self.ref_time.is_none() {
            self.ref_time = Some(sample.timestamp);
        }
        self.last_sample = Some(sample.clone());
        self.history.add_sample(sample);
        self.tick_count += 1;

        // Auto-save every 60 ticks (~5 min at 5s interval)
        if self.tick_count % 60 == 0 {
            self.history.save();
        }
    }

    /// Convert a DateTime to seconds since ref_time (for chart x-axis)
    pub fn time_to_x(&self, ts: &DateTime<Local>) -> f64 {
        match self.ref_time {
            Some(ref rt) => (*ts - *rt).num_milliseconds() as f64 / 1000.0,
            None => 0.0,
        }
    }

    /// Convert seconds since ref_time back to DateTime
    pub fn x_to_time(&self, x: f64) -> Option<DateTime<Local>> {
        self.ref_time
            .map(|rt| rt + chrono::Duration::milliseconds((x * 1000.0) as i64))
    }

    /// Get chart data points for capacity (filtered by viewport)
    pub fn capacity_chart_data(&self, samples: &[BatterySample]) -> Vec<(f64, f64)> {
        let (t_start, t_end) = self.viewport.visible_range();
        samples
            .iter()
            .map(|s| (self.time_to_x(&s.timestamp), s.capacity))
            .filter(|(x, _)| *x >= t_start && *x <= t_end)
            .collect()
    }

    /// Get chart data points for power (filtered by viewport)
    pub fn power_chart_data(&self, samples: &[BatterySample]) -> Vec<(f64, f64)> {
        let (t_start, t_end) = self.viewport.visible_range();
        samples
            .iter()
            .map(|s| (self.time_to_x(&s.timestamp), s.power_watts))
            .filter(|(x, _)| *x >= t_start && *x <= t_end)
            .collect()
    }

    /// Update viewport to fit current data
    pub fn fit_viewport(&mut self) {
        if let (Some(first), Some(last)) = (
            self.history.samples.first(),
            self.history.samples.last(),
        ) {
            let total = self.time_to_x(&last.timestamp) - self.time_to_x(&first.timestamp);
            self.viewport.fit_data(total);
        }
    }

    /// Update session viewport to fit session data
    pub fn fit_session_viewport(&mut self, session_idx: usize) {
        if let Some(session) = self.history.completed_sessions().get(session_idx) {
            if let (Some(first), Some(last)) =
                (session.samples.first(), session.samples.last())
            {
                let total = self.time_to_x(&last.timestamp) - self.time_to_x(&first.timestamp);
                self.session_viewport.fit_data(total);
            }
        }
    }

    /// Get the active viewport for current view
    pub fn active_viewport_mut(&mut self) -> &mut ChartViewport {
        match self.view {
            View::SessionDetail(_) => &mut self.session_viewport,
            _ => &mut self.viewport,
        }
    }

    pub fn switch_to_dashboard(&mut self) {
        self.view = View::Dashboard;
    }

    pub fn switch_to_history(&mut self) {
        self.view = View::HistoryChart;
        self.fit_viewport();
    }

    pub fn switch_to_session(&mut self, idx: usize) {
        if idx < self.history.completed_sessions().len() {
            self.view = View::SessionDetail(idx);
            self.fit_session_viewport(idx);
        }
    }
}
