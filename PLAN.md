# Plan: cargo-min-publish-age Cross-Platform Registry Proxy

## Goal

Intercept crates.io sparse registry requests transparently by running a lightweight HTTP proxy as a user-level background service. The proxy marks recently-published crate versions as `yanked` based on a configurable minimum age cutoff, protecting the supply chain from newly-published crates without manual vetting.

## Constraints & Decisions

- **No PATH wrapper.** Too fragile with rustup.
- **No rustup toolchain override.** Rejected by user.
- **No build.rs / proc-macro injection.** Runs too late in Cargo's lifecycle.
- **Persistent background daemon acceptable.** Auto-starts on user login.
- **Cross-platform.** Linux (systemd), macOS (LaunchAgent), Windows (Startup folder).
- **Minimal transitive dependencies.** Current count is 35. `toml_edit` will add ~4–6.
- **No markers in `~/.cargo/config.toml`.** Use proper TOML surgical editing via `toml_edit`.

## Architecture

### Overview

The project is a single binary `cargo-min-publish-age` with three roles:

1. **Proxy server** — serves filtered sparse registry index over HTTP
2. **Cargo subcommand** — `cargo min-publish-age install` / `uninstall`
3. **Background service target** — invoked by OS service manager as `cargo-min-publish-age proxy`

### Data Flow

```
User runs `cargo check`
  ↓
Cargo reads ~/.cargo/config.toml
  [source.crates-io]
  replace-with = "min-publish-age"
  [source.min-publish-age]
  registry = "sparse+http://127.0.0.1:2015/index/"
  ↓
Cargo makes HTTP GET to 127.0.0.1:2015/index/...
  ↓
Our daemon (already running) receives the request
  ↓
Daemon fetches upstream from https://index.crates.io/...
  ↓
Daemon parses JSON lines, marks young versions as yanked
  ↓
Daemon returns modified JSON to Cargo
  ↓
Cargo resolves dependencies, excluding yanked versions
```

### Service Lifecycle

- **Install** (`cargo min-publish-age install`):
  1. Accept optional `--port <N>` (default: 2015) and `--age "<duration>"` (default: "14 days") CLI args
  2. **Linux**: Verify systemd is available; write `~/.config/systemd/user/cargo-min-publish-age.service` with `--port` in `ExecStart`; run `systemctl --user daemon-reload && enable --now`
  3. **macOS**: Write `~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist` with `ProgramArguments` including `--port`; silently `launchctl bootout` any existing agent, then `launchctl bootstrap`
  4. Write `~/.cargo/config.toml` with `[registry]`, `[source.crates-io]`, and `[source.min-publish-age]` tables

- **Runtime:**
  - Service invokes `cargo-min-publish-age proxy`
  - Server reads `~/.cargo/config.toml` for port and min-age
  - Binds to 127.0.0.1:<port>
  - Serves filtered index until stopped

- **Uninstall** (`cargo min-publish-age uninstall`):
  1. Open `~/.cargo/config.toml` via `toml_edit`
  2. Remove `[source.min-publish-age]` table if present
  3. Remove `[source.crates-io]` table if present AND `replace-with == "min-publish-age"`
  4. Remove `[registry] global-min-publish-age` if present
  5. Best-effort stop and remove the OS service (ignores failures, exits 0):
     - **Linux**: `systemctl --user stop/disable` + remove `.service` file + `daemon-reload`
     - **macOS**: `launchctl bootout` + remove `.plist` file

- **Cargo install/uninstall:**
  - `cargo install cargo-min-publish-age` — installs binary only. No side effects.
  - `cargo uninstall cargo-min-publish-age` — removes binary only. Service and config remain.
  - **User must run `cargo min-publish-age uninstall` before `cargo uninstall`.**

## Configuration

### `~/.cargo/config.toml` (User-Editable)

```toml
[registry]
global-min-publish-age = "14 days"

[source.crates-io]
replace-with = "min-publish-age"

[source.min-publish-age]
registry = "sparse+http://127.0.0.1:2015/index/"
```

- `global-min-publish-age`: RFC 3923 format. Daemon reads this at startup.
- Port is derived from `[source.min-publish-age] registry` URL. No separate port setting.

### Daemon Behavior

The daemon re-reads `~/.cargo/config.toml` on startup. It does NOT hot-reload. To change port or min-age:

1. Edit `~/.cargo/config.toml`
2. Restart service (or run `cargo min-publish-age install` again, which restarts it)

## Platform-Specific Service Definitions

### Linux — systemd User Service

**File:** `~/.config/systemd/user/cargo-min-publish-age.service`

```ini
[Unit]
Description=cargo-min-publish-age registry proxy

[Service]
Type=simple
ExecStart=%h/.cargo/bin/cargo-min-publish-age proxy --port 2015
Restart=on-failure

[Install]
WantedBy=default.target
```

*(Port is baked into `ExecStart` from `--port` CLI arg at install time.)*

**Registration:**
```sh
systemctl --user daemon-reload
systemctl --user enable --now cargo-min-publish-age
```

### macOS — LaunchAgent

**File:** `~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.github.cargo-min-publish-age</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/$USER/.cargo/bin/cargo-min-publish-age</string>
        <string>proxy</string>
        <string>--port</string>
        <string>2015</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

**Registration:**
```sh
# Idempotent: silently unload existing agent first
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist 2>/dev/null
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist
```

### Windows — Startup Folder + Immediate Spawn

**File:** `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\cargo-min-publish-age.lnk`

- Shortcut pointing to `cargo-min-publish-age.exe proxy`
- Also spawn the process immediately via `Command::new(...).spawn()` so it works before next logout/login

**Why Startup folder instead of Task Scheduler:**
- No admin rights needed
- Works on all Windows editions (Home, Pro, Enterprise)
- Trivial to create from Rust (COM `IShellLink` or PowerShell one-liner)
- Can be deleted directly by the user

## Command-Line Interface

### `cargo-min-publish-age proxy`

Runs the HTTP proxy. Intended to be invoked by the OS service manager, not directly by users.

**Behavior:**
1. Read `~/.cargo/config.toml` for `[source.min-publish-age] registry` URL, extract port
2. Read `[registry] global-min-publish-age` for cutoff duration
3. Bind `tiny_http::Server::http("127.0.0.1:<port>")`
4. Serve filtered sparse index

### `cargo min-publish-age install`

One-time setup. Idempotent (re-running restarts service with current config).

**Behavior:**
1. Parse optional `--port <N>` and `--age "<duration>"` CLI args
2. Determine binary path via `current_exe()`
3. **Linux**: Verify systemd; write `.service` file; `systemctl --user daemon-reload && enable --now`
4. **macOS**: Write `.plist` to `~/Library/LaunchAgents/`; `launchctl bootout` (silent) then `launchctl bootstrap`
5. If service starts successfully, write `~/.cargo/config.toml` with source replacement tables

### `cargo min-publish-age uninstall`

Full cleanup.

**Behavior:**
1. Open `~/.cargo/config.toml` via `toml_edit`
2. Remove `[source.min-publish-age]`
3. Remove `[source.crates-io]` only if `replace-with == "min-publish-age"`
4. Remove `[registry] global-min-publish-age`
5. Best-effort stop and remove the OS service (ignore failures):
   - **Linux**: `systemctl --user stop/disable`, remove `.service`, `daemon-reload`
   - **macOS**: `launchctl bootout`, remove `.plist`
6. Exit 0 even if service was already gone

### `cargo min-publish-age` (no subcommand)

Prints basic usage message. (Status query deferred to future work.)

## Dependencies

### New Additions

- **`toml_edit`** (~4–6 transitive deps) — surgical TOML editing, same library Cargo uses

### Existing (no changes)

- `tiny_http` — HTTP server for proxy
- `ureq` — HTTP client for upstream fetches
- `serde_json` — JSON manipulation of index entries
- `time` — date parsing and cutoff calculation

## Implementation Phases

### Phase 1: Extract Config Reading

- Add `toml_edit` to `Cargo.toml`
- Create `src/config.rs`:
  - `Config::load()` — reads `~/.cargo/config.toml`
  - Parses `[registry] global-min-publish-age = "14 days"` into `time::Duration`
  - Derives port from `[source.min-publish-age] registry` URL (`sparse+http://127.0.0.1:<port>/index/`)
  - Returns defaults if absent
  - `write_install_config(port: u16, age: &str)` — writes source replacement tables via `toml_edit`
  - `remove_install_config()` — removes `[source.min-publish-age]`, `[source.crates-io]`, and `[registry] global-min-publish-age`

### Phase 2: Refactor Proxy into `proxy` Subcommand

- Extract proxy logic from `src/main.rs` into `src/proxy.rs`
- `pub fn run(port_override: Option<u16>)` — reads config from `~/.cargo/config.toml`, binds to port
- Add CLI dispatch in `src/main.rs`:
  - `proxy` → run proxy server (used by service manager)
  - `install` → install systemd service + write config
  - `uninstall` → remove config + stop service
  - (no args / `help`) → print usage

### Phase 3: Cross-Platform Service Install

- Create `src/platform/mod.rs` with `#[cfg(target_os = ...)]` dispatch:
  - Linux: systemd via `std::fs` + `Command::new("systemctl")`
  - macOS: LaunchAgent via `std::fs` + `Command::new("launchctl")`
  - Windows: Startup shortcut via PowerShell one-liner + immediate spawn (deferred)

### Phase 4: Config.toml Surgical Editing

- `write_install_config(port: u16, age: &str)`:
  - Open `~/.cargo/config.toml` with `toml_edit`
  - Write `[registry] global-min-publish-age = "..."`
  - Write `[source.crates-io] replace-with = "min-publish-age"`
  - Write `[source.min-publish-age] registry = "sparse+http://127.0.0.1:<port>/index/"`
  - Use `toml_edit::table()` + `set_implicit(true)` for `[source]` to avoid inline table syntax
- `remove_install_config()`:
  - Open `~/.cargo/config.toml` with `toml_edit`
  - Remove `[source.min-publish-age]` and `[source.crates-io]` if ours
  - Remove `[registry] global-min-publish-age`
  - Serialize back preserving formatting

### Phase 5: Integration & Testing

- Test `install` → `cargo check` → `uninstall` on Linux
- Test `install` → `cargo check` → `uninstall` on macOS
- Windows startup shortcut deferred to future work

## Edge Cases & Decisions

### `+nightly` and `rust-toolchain.toml`

When the user runs `cargo +nightly check`, rustup bypasses our default toolchain entirely and uses the real nightly cargo. Since our proxy is configured in `~/.cargo/config.toml` (which all Cargo versions read), the source replacement still applies. The proxy daemon is independent of which cargo binary runs. **This is fine.**

### Port Already in Use

No explicit port check. `systemctl --user enable --now` will fail if the port is already bound, and we exit without touching `~/.cargo/config.toml`. User can re-run with `--port <N>`.

### Config File Missing

If `~/.cargo/config.toml` doesn't exist, `install` creates it. `proxy` uses defaults (port 2015, 14 days) if the file is missing.

### Uninstall When Already Uninstalled

Idempotent: exits 0 even if service is already gone or config tables were manually removed. No warnings printed.

### Windows: Symlink vs Copy

On Windows, `cargo install` may create the binary in a path with spaces. The shortcut `.lnk` handles this natively. No need to copy or symlink.

## File Structure

```
src/
  main.rs          # CLI dispatch
  proxy.rs         # HTTP proxy logic
  config.rs        # Config read/write via toml_edit
  platform/
    mod.rs         # #[cfg] dispatch (Linux/macOS/Windows)
    linux.rs       # systemd implementation
    macos.rs       # LaunchAgent
    windows.rs     # Startup shortcut (deferred)
Cargo.toml
main.go            # Original Go reference (keep)
PLAN.md            # This document
```

## macOS Testing Checklist

Run these on your Macbook to validate the LaunchAgent integration.

### 1. Build

```sh
cd /path/to/cargo-min-publish-age
cargo build
```

### 2. Install

```sh
cargo run -- install --port 2015 --age "14 days"
```

**Expected:**
- `~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist` exists
- `launchctl list | grep cargo-min-publish-age` shows the agent
- `~/.cargo/config.toml` has `[source.min-publish-age]` and `[source.crates-io]` tables

### 3. Verify proxy is running

```sh
curl http://127.0.0.1:2015/index/
# Should return 404 or an index response (not "connection refused")
```

### 4. Test with cargo

```sh
mkdir -p /tmp/test-project && cd /tmp/test-project
cargo init
cargo add serde  # or any crate — should resolve through the proxy
cargo check
```

**Expected:** `cargo check` succeeds. The proxy daemon is filtering the index in the background.

### 5. Inspect what the proxy sees

Tail the service output. With `KeepAlive` there is no stdout by default; for debugging you can run the proxy manually:

```sh
~/.cargo/bin/cargo-min-publish-age proxy --port 2015
```

Then in another terminal run `cargo check` in a test project. The manual proxy will log requests to stderr.

### 6. Uninstall

```sh
cargo run -- uninstall
```

**Expected:**
- `launchctl list | grep cargo-min-publish-age` returns nothing
- `~/Library/LaunchAgents/io.github.cargo-min-publish-age.plist` is gone
- `~/.cargo/config.toml` no longer has `[source.min-publish-age]` or `[source.crates-io]`
- Port 2015 is free (`lsof -i :2015` returns nothing)

### 7. Reinstall with different port

```sh
cargo run -- install --port 3000
cargo run -- uninstall
```

Verify the plist contains `--port 3000` and the proxy binds to 3000.

## Future Work (Out of Scope)

- Hot-reload of config (currently requires service restart)
- Local project `.cargo/config.toml` support (currently global only)
- Replacing `serde_json` with a lighter JSON crate to reduce dep count
- `cargo min-publish-age status` as explicit subcommand instead of default no-args behavior

