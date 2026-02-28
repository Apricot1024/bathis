# AGENTS.md — bathis

> **bathis** is a Linux TUI battery monitor built with Rust + ratatui.
> It reads battery stats from `/sys/class/power_supply/`, displays live dashboards
> with capacity/power charts, and tracks charge sessions to JSON history.

## Build & Run

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run
cargo run

# Check (type-check without building)
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Format check (CI)
cargo fmt -- --check
```

There are **no tests** yet. When adding tests, use `cargo test` and `cargo test <test_name>` for a single test.

## Project Structure

```
src/
├── main.rs       # Entry point, event loop (crossterm), key bindings
├── app.rs        # App state, View enum, ChartViewport (zoom/pan), data queries
├── battery.rs    # BatteryReader (Linux sysfs), BatterySample, BatteryStatus enum
├── history.rs    # History persistence (JSON via serde), ChargeSession tracking
└── ui.rs         # All ratatui rendering: dashboard, history chart, session detail
```

Data path: `~/.local/share/bathis/history.json` (via `dirs::data_dir()`).

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` 0.29 | TUI framework |
| `crossterm` 0.28 | Terminal backend / event handling |
| `serde` + `serde_json` | Serialization for history persistence |
| `chrono` 0.4 | Timestamps with local timezone |
| `dirs` 6 | XDG data directory resolution |

Rust edition: **2024**

## Code Style & Conventions

### Formatting

- Use `cargo fmt` defaults (no `rustfmt.toml` — standard rustfmt config).
- No line length override — standard rustfmt wrapping applies.
- Braces on same line for functions, `if`, `match`, `loop`.
- Longer builder chains (ratatui widgets) may be multi-line with `.method()` per line.

### Naming

- **Types/Structs/Enums**: PascalCase (`BatterySample`, `ChartViewport`, `View`).
- **Functions/Methods**: snake_case (`add_sample`, `fit_viewport`, `render_dashboard`).
- **Constants**: SCREAMING_SNAKE_CASE (`SAMPLE_INTERVAL`, `MAX_SAMPLES`).
- **Variables**: snake_case, descriptive (`last_sample_time`, `capacity_bar_width`).
- **Enum variants**: PascalCase (`Charging`, `Dashboard`, `SessionDetail(usize)`).

### Imports (ordered groups, separated by blank lines)

1. `std` imports
2. External crate imports (`chrono`, `crossterm`, `ratatui`, `serde`, etc.)
3. Internal crate imports (`crate::app`, `crate::battery`, etc.)

Within ratatui, nested imports are grouped in a single `use ratatui::{...}` block.

### Module Organization

- One module per file, declared in `main.rs` with `mod`.
- Public structs/enums/functions use `pub`. Fields are `pub` (flat, no getters).
- No `lib.rs` — this is a binary crate only.

### Error Handling

- `main()` returns `io::Result<()>`.
- Battery reader uses `Option<Self>` / `Option<BatterySample>` — returns `None` on failure.
- Sysfs reads: `.ok()` to convert `Result` → `Option`, then `?` to propagate.
- File I/O failures: silently ignore with `let _ = ...` (non-critical persistence).
- Terminal init/restore handled by `ratatui::init()` / `ratatui::restore()`.
- `.expect()` only at top-level for fatal missing-battery case.

### Patterns to Follow

- **State machine**: `App` holds all state; `View` enum drives which UI renders.
- **Immediate-mode UI**: `ui::render()` dispatches on `app.view`, each view function
  takes `&App` (read-only borrow). No UI state outside `App`.
- **serde for persistence**: `#[derive(Serialize, Deserialize)]` on all persisted types.
  Use `#[serde(skip)]` for transient fields (e.g., `active_session`).
- **Clone on samples**: `BatterySample` is `Clone`; samples are cloned into sessions
  and the main history vec.
- **Viewport pattern**: `ChartViewport` manages zoom/pan state. Both the global history
  and per-session views have independent viewports.

### Things to Avoid

- Don't suppress warnings with `#[allow(...)]` — fix them.
- Don't use `unwrap()` on fallible I/O — use `?`, `.ok()`, or `.unwrap_or()`.
- Don't add `unsafe` code.
- Don't introduce new dependencies without justification.
- Don't create `lib.rs` — this is a binary-only crate.

### Commit Messages

Format: `bathis: <description>` (lowercase, imperative mood).
Example: `bathis: add temperature tracking to dashboard`

## Platform Notes

- **Linux only** — reads from `/sys/class/power_supply/` sysfs interface.
- Battery values are in microwatts/microvolts in sysfs, converted to W/V.
- Power sign convention: positive = charging, negative = discharging.
- History is capped at 40,000 samples (~48h at 5s intervals).
- Auto-saves every 60 samples (~5 min). Always saves on quit (`q` / Ctrl+C).
