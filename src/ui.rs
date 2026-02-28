use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph, Wrap,
    },
    Frame,
};

use crate::app::{App, View};
use crate::battery::BatteryStatus;

/// Render the entire UI
pub fn render(f: &mut Frame, app: &App) {
    match app.view {
        View::Dashboard => render_dashboard(f, app),
        View::HistoryChart => render_history_chart(f, app),
        View::SessionDetail(idx) => render_session_detail(f, app, idx),
    }
}

/// Format seconds to a human-readable duration
fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

/// Format x-axis value (seconds since start) as time label
fn format_time_label(app: &App, x: f64) -> String {
    if let Some(dt) = app.x_to_time(x) {
        format!("{:02}:{:02}", dt.hour(), dt.minute())
    } else {
        String::new()
    }
}

/// Generate time axis labels for the visible range
fn time_axis_labels(app: &App, start: f64, end: f64) -> Vec<Span<'static>> {
    let n_labels = 5;
    let step = (end - start) / (n_labels as f64 - 1.0);
    (0..n_labels)
        .map(|i| {
            let x = start + step * i as f64;
            Span::raw(format_time_label(app, x))
        })
        .collect()
}

// --- Dashboard View ---

fn render_dashboard(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title bar
            Constraint::Min(8),   // status area
            Constraint::Length(3), // help bar
        ])
        .split(f.area());

    render_title_bar(f, chunks[0], app);
    render_status_panel(f, chunks[1], app);
    render_help_bar(f, chunks[2], app);
}

fn render_title_bar(f: &mut Frame, area: Rect, app: &App) {
    let title = format!(" ⚡ bathis — {} ", app.battery_name);
    let block = Paragraph::new(Line::from(vec![
        Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(block, area);
}

fn render_status_panel(f: &mut Frame, area: Rect, app: &App) {
    let sample = match &app.last_sample {
        Some(s) => s,
        None => {
            let msg = Paragraph::new("Waiting for first battery sample...")
                .block(Block::default().borders(Borders::ALL).title(" Status "));
            f.render_widget(msg, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: battery info
    let status_color = match sample.status {
        BatteryStatus::Charging => Color::Green,
        BatteryStatus::Discharging => Color::Yellow,
        BatteryStatus::Full => Color::Cyan,
        _ => Color::Gray,
    };

    let capacity_bar_width = 20;
    let filled = (sample.capacity / 100.0 * capacity_bar_width as f64) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(capacity_bar_width - filled);

    let power_display = if sample.power_watts.abs() < 0.01 {
        "0.00 W".to_string()
    } else if sample.power_watts > 0.0 {
        format!("+{:.2} W (charging)", sample.power_watts)
    } else {
        format!("{:.2} W (discharging)", sample.power_watts)
    };

    let info_lines = vec![
        Line::from(vec![
            Span::raw("  Status:   "),
            Span::styled(format!("{}", sample.status), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Battery:  "),
            Span::styled(format!("{:.1}%", sample.capacity), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("            "),
            Span::styled(&bar, Style::default().fg(if sample.capacity > 20.0 { Color::Green } else { Color::Red })),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Power:    "),
            Span::styled(power_display, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Voltage:  "),
            Span::styled(format!("{:.3} V", sample.voltage_now_v), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Energy:   "),
            Span::styled(
                format!("{:.2} / {:.2} Wh", sample.energy_now_wh, sample.energy_full_wh),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let info = Paragraph::new(info_lines)
        .block(Block::default().borders(Borders::ALL).title(" Battery Info "))
        .wrap(Wrap { trim: false });
    f.render_widget(info, chunks[0]);

    // Right: session history
    let sessions = app.history.completed_sessions();
    let mut session_items: Vec<ListItem> = Vec::new();

    if sessions.is_empty() {
        session_items.push(ListItem::new(Line::from(
            Span::styled("  No completed charge sessions yet", Style::default().fg(Color::DarkGray)),
        )));
    } else {
        for (i, session) in sessions.iter().enumerate().rev() {
            let duration = session
                .end_time
                .map(|e| (e - session.start_time).num_seconds() as f64)
                .unwrap_or(0.0);
            let line = format!(
                "  [{}] {:.0}% → {:.0}%  ({})  {}",
                i + 1,
                session.start_capacity,
                session.end_capacity,
                format_duration(duration),
                session.start_time.format("%m/%d %H:%M"),
            );
            session_items.push(ListItem::new(Line::from(
                Span::styled(line, Style::default().fg(Color::White)),
            )));
        }
    }

    let sample_count = app.history.all_samples().len();
    let session_list = List::new(session_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Charge Sessions (90%+)  |  {} samples ", sample_count)),
    );
    f.render_widget(session_list, chunks[1]);
}

fn render_help_bar(f: &mut Frame, area: Rect, app: &App) {
    let help_text = match app.view {
        View::Dashboard => " [h] History Chart  [1/2] Session Detail  [q] Quit ",
        View::HistoryChart => " [d] Dashboard  [←/→] Pan  [+/-] Zoom  [f] Fit  [1/2] Session  [q] Quit ",
        View::SessionDetail(_) => " [d] Dashboard  [h] History  [←/→] Pan  [+/-] Zoom  [f] Fit  [q] Quit ",
    };

    let help = Paragraph::new(Line::from(
        Span::styled(help_text, Style::default().fg(Color::DarkGray)),
    ))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(help, area);
}

// --- History Chart View ---

fn render_history_chart(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title
            Constraint::Percentage(50), // capacity chart
            Constraint::Percentage(50), // power chart
            Constraint::Length(3),  // help
        ])
        .split(f.area());

    render_title_bar(f, chunks[0], app);
    render_capacity_chart(f, chunks[1], app, app.history.all_samples());
    render_power_chart(f, chunks[2], app, app.history.all_samples());
    render_help_bar(f, chunks[3], app);
}

fn render_capacity_chart(f: &mut Frame, area: Rect, app: &App, samples: &[crate::battery::BatterySample]) {
    if samples.is_empty() {
        let msg = Paragraph::new("No data yet")
            .block(Block::default().borders(Borders::ALL).title(" Battery % "));
        f.render_widget(msg, area);
        return;
    }

    let data: Vec<(f64, f64)> = match app.view {
        View::SessionDetail(_idx) => {
            let (t_start, t_end) = app.session_viewport.visible_range();
            let session_start = samples.first().map(|s| app.time_to_x(&s.timestamp)).unwrap_or(0.0);
            samples
                .iter()
                .map(|s| (app.time_to_x(&s.timestamp) - session_start, s.capacity))
                .filter(|(x, _)| *x >= t_start && *x <= t_end)
                .collect()
        }
        _ => app.capacity_chart_data(samples),
    };

    if data.is_empty() {
        let msg = Paragraph::new("No data in visible range (try [f] to fit)")
            .block(Block::default().borders(Borders::ALL).title(" Battery % "));
        f.render_widget(msg, area);
        return;
    }

    let (vp_start, vp_end) = match app.view {
        View::SessionDetail(_) => app.session_viewport.visible_range(),
        _ => app.viewport.visible_range(),
    };

    let x_labels = time_axis_labels_for_range(app, vp_start, vp_end, samples);

    let datasets = vec![Dataset::default()
        .name("Battery %")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Green))
        .data(&data)];

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::ALL).title(" Battery % "))
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default().fg(Color::Gray))
                .bounds([vp_start, vp_end])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("%")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::raw("0"),
                    Span::raw("25"),
                    Span::raw("50"),
                    Span::raw("75"),
                    Span::raw("100"),
                ]),
        );

    f.render_widget(chart, area);
}

fn render_power_chart(f: &mut Frame, area: Rect, app: &App, samples: &[crate::battery::BatterySample]) {
    if samples.is_empty() {
        let msg = Paragraph::new("No data yet")
            .block(Block::default().borders(Borders::ALL).title(" Power (W) "));
        f.render_widget(msg, area);
        return;
    }

    let data: Vec<(f64, f64)> = match app.view {
        View::SessionDetail(_idx) => {
            let (t_start, t_end) = app.session_viewport.visible_range();
            let session_start = samples.first().map(|s| app.time_to_x(&s.timestamp)).unwrap_or(0.0);
            samples
                .iter()
                .map(|s| (app.time_to_x(&s.timestamp) - session_start, s.power_watts))
                .filter(|(x, _)| *x >= t_start && *x <= t_end)
                .collect()
        }
        _ => app.power_chart_data(samples),
    };

    if data.is_empty() {
        let msg = Paragraph::new("No data in visible range (try [f] to fit)")
            .block(Block::default().borders(Borders::ALL).title(" Power (W) "));
        f.render_widget(msg, area);
        return;
    }

    let (vp_start, vp_end) = match app.view {
        View::SessionDetail(_) => app.session_viewport.visible_range(),
        _ => app.viewport.visible_range(),
    };

    // Dynamic y-axis bounds based on visible data
    let min_power = data.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let max_power = data.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);
    let y_margin = (max_power - min_power).abs() * 0.1 + 0.5;
    let y_min = (min_power - y_margin).min(-0.5);
    let y_max = (max_power + y_margin).max(0.5);

    let x_labels = time_axis_labels_for_range(app, vp_start, vp_end, samples);

    let datasets = vec![Dataset::default()
        .name("Power")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Yellow))
        .data(&data)];

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::ALL).title(" Power (W) — +charge / -discharge "))
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default().fg(Color::Gray))
                .bounds([vp_start, vp_end])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("W")
                .style(Style::default().fg(Color::Gray))
                .bounds([y_min, y_max])
                .labels(vec![
                    Span::raw(format!("{:.1}", y_min)),
                    Span::raw("0"),
                    Span::raw(format!("{:.1}", y_max)),
                ]),
        );

    f.render_widget(chart, area);
}

fn time_axis_labels_for_range(
    app: &App,
    start: f64,
    end: f64,
    samples: &[crate::battery::BatterySample],
) -> Vec<Span<'static>> {
    // For session detail, offset from session start
    match app.view {
        View::SessionDetail(_) => {
            let session_ref = samples.first().map(|s| s.timestamp);
            let n_labels = 5;
            let step = (end - start) / (n_labels as f64 - 1.0);
            (0..n_labels)
                .map(|i| {
                    let x = start + step * i as f64;
                    if let Some(rt) = session_ref {
                        let dt = rt + chrono::Duration::milliseconds((x * 1000.0) as i64);
                        Span::raw(format!("{:02}:{:02}", dt.hour(), dt.minute()))
                    } else {
                        Span::raw(format_duration(x))
                    }
                })
                .collect()
        }
        _ => time_axis_labels(app, start, end),
    }
}

// --- Session Detail View ---

fn render_session_detail(f: &mut Frame, app: &App, idx: usize) {
    let sessions = app.history.completed_sessions();
    if idx >= sessions.len() {
        let msg = Paragraph::new(format!("Session {} not found", idx + 1))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(msg, f.area());
        return;
    }

    let session = &sessions[idx];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title
            Constraint::Length(4),  // session info
            Constraint::Percentage(50), // capacity chart
            Constraint::Percentage(50), // power chart
            Constraint::Length(3),  // help
        ])
        .split(f.area());

    render_title_bar(f, chunks[0], app);

    // Session info
    let duration = session
        .end_time
        .map(|e| (e - session.start_time).num_seconds() as f64)
        .unwrap_or(0.0);
    let info_text = format!(
        "  Session {}  |  {:.0}% → {:.0}%  |  {}  |  Started: {}",
        idx + 1,
        session.start_capacity,
        session.end_capacity,
        format_duration(duration),
        session.start_time.format("%Y-%m-%d %H:%M"),
    );
    let info = Paragraph::new(Line::from(Span::styled(
        info_text,
        Style::default().fg(Color::Cyan),
    )))
    .block(Block::default().borders(Borders::ALL).title(format!(" Charge Session {} ", idx + 1)));
    f.render_widget(info, chunks[1]);

    render_capacity_chart(f, chunks[2], app, &session.samples);
    render_power_chart(f, chunks[3], app, &session.samples);
    render_help_bar(f, chunks[4], app);
}
