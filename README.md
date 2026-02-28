# bathis ⚡

A Linux TUI battery monitor built with Rust and ratatui. It tracks battery capacity, power usage, and charge sessions by reading from the sysfs interface.

## Features

- Live dashboard with real-time battery statistics and capacity bar
- Interactive history charts for battery percentage and power usage (W)
- Automatic charge session tracking (records sessions reaching 90%+)
- Zoom and pan functionality for all charts
- Headless recording mode for background data collection
- Persistent JSON history storage

## Requirements

- Linux (requires `/sys/class/power_supply/` interface)
- Rust toolchain (Edition 2024)

## Installation

### From Source

```bash
git clone https://github.com/user/bathis.git
cd bathis
cargo build --release
```

The binary will be located at `target/release/bathis`.

### Using Cargo

```bash
cargo install --path .
```

## Usage

### Interactive TUI

Run the interactive monitor:

```bash
bathis
```

### Headless Recording

Record battery samples to history without the TUI:

```bash
bathis --record
```

### Systemd User Service

To record battery history in the background, create a systemd user service at `~/.config/systemd/user/bathis-record.service`:

```ini
[Unit]
Description=bathis battery recorder
After=default.target

[Service]
ExecStart=%h/.cargo/bin/bathis --record
Restart=always

[Install]
WantedBy=default.target
```

Manage the service with these commands:

```bash
systemctl --user daemon-reload
systemctl --user enable bathis-record.service
systemctl --user start bathis-record.service
systemctl --user status bathis-record.service
journalctl --user -u bathis-record.service -f
```

### Key Bindings

| Key | Action |
|-----|--------|
| `d` | Switch to Dashboard view |
| `h` | Switch to History Chart view |
| `1` | View details for the first completed session |
| `2` | View details for the second completed session |
| `+` / `=` | Zoom in on the active chart |
| `-` | Zoom out on the active chart |
| `←` / `→` | Pan left or right on the active chart |
| `f` | Fit chart viewport to available data |
| `q` | Save and quit |
| `Ctrl+C` | Save and quit |

## How It Works

bathis reads battery data from the Linux kernel via `/sys/class/power_supply/`. It samples capacity (%), power (W), voltage (V), and energy (Wh) every 5 seconds. 

Charge sessions are automatically detected when the battery status changes to "Charging". A session is considered completed and saved to history if the battery level reaches 90% or higher before charging stops.

## Data Storage

History is stored in a JSON file at:
`~/.local/share/bathis/history.json`

- **Sampling Interval**: 5 seconds
- **Auto-save**: Every 60 samples (~5 minutes)
- **Capacity**: Capped at 40,000 samples (~48 hours of continuous monitoring)
- **Sessions**: Keeps the last 2 completed charge sessions

## Project Structure

- `src/main.rs`: Entry point, event loop, and headless recording logic
- `src/app.rs`: Application state management and chart viewport logic
- `src/battery.rs`: Linux sysfs battery reader and data structures
- `src/history.rs`: Persistent storage and charge session tracking
- `src/ui.rs`: Ratatui rendering for all views and charts

## License

MIT
