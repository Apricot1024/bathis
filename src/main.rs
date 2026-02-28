mod app;
mod battery;
mod history;
mod ui;

use std::env;
use std::io;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use app::App;
use battery::BatteryReader;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: bathis [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --record    Run headless, sampling battery to history without TUI");
        println!("  -h, --help  Show this help");
        return Ok(());
    }

    let reader = BatteryReader::new().expect("No battery found in /sys/class/power_supply/");

    if args.iter().any(|a| a == "--record") {
        return run_headless(reader);
    }

    let battery_name = reader.battery_name();
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, reader, battery_name);
    ratatui::restore();
    result
}

fn run(
    terminal: &mut DefaultTerminal,
    reader: BatteryReader,
    battery_name: String,
) -> io::Result<()> {
    let mut app = App::new(battery_name);

    // Take initial sample
    if let Some(sample) = reader.sample() {
        app.add_sample(sample);
    }

    let mut last_sample_time = Instant::now();

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        // Poll for events with short timeout so we stay responsive
        let timeout = Duration::from_millis(100);
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                // Quit
                KeyCode::Char('q') => {
                    app.history.save();
                    app.running = false;
                    return Ok(());
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.history.save();
                    return Ok(());
                }

                // View switching
                KeyCode::Char('d') => app.switch_to_dashboard(),
                KeyCode::Char('h') => app.switch_to_history(),
                KeyCode::Char('1') => app.switch_to_session(0),
                KeyCode::Char('2') => app.switch_to_session(1),

                // Zoom
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    app.active_viewport_mut().zoom_in();
                }
                KeyCode::Char('-') => {
                    app.active_viewport_mut().zoom_out();
                }

                // Pan
                KeyCode::Left => {
                    app.active_viewport_mut().pan_left();
                }
                KeyCode::Right => {
                    app.active_viewport_mut().pan_right();
                }

                // Fit to data
                KeyCode::Char('f') => match app.view {
                    app::View::HistoryChart => app.fit_viewport(),
                    app::View::SessionDetail(idx) => app.fit_session_viewport(idx),
                    _ => {}
                },

                _ => {}
            }
        }

        // Sample battery at interval
        if last_sample_time.elapsed() >= SAMPLE_INTERVAL {
            if let Some(sample) = reader.sample() {
                app.add_sample(sample);
            }
            last_sample_time = Instant::now();
        }
    }
}

fn run_headless(reader: BatteryReader) -> io::Result<()> {
    let mut history = history::History::load();
    let mut tick_count: u64 = 0;

    eprintln!(
        "bathis: recording battery samples every {}s (Ctrl+C to stop)",
        SAMPLE_INTERVAL.as_secs()
    );

    // Take initial sample
    if let Some(sample) = reader.sample() {
        history.add_sample(sample);
        tick_count += 1;
    }

    loop {
        thread::sleep(SAMPLE_INTERVAL);

        if let Some(sample) = reader.sample() {
            history.add_sample(sample);
            tick_count += 1;

            // Auto-save every 60 ticks (~5 min at 5s interval)
            if tick_count.is_multiple_of(60) {
                history.save();
            }
        }
    }
}
