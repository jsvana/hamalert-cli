# hamalert-cli

A command-line interface for managing HamAlert triggers. This tool allows ham radio operators to programmatically add and manage callsign alerts through the HamAlert API.

## Description

hamalert-cli provides a simple CLI interface for interacting with HamAlert.org, allowing you to create triggers that notify you when specific callsigns appear. The tool handles authentication and trigger management through a convenient command-line interface.

## Installation

### Prerequisites

- Rust and Cargo (install from [rustup.rs](https://rustup.rs))
- A HamAlert.org account

### Building from Source

```bash
git clone <repository-url>
cd hamalert-sub
cargo build --release
```

The compiled binary will be available at `target/release/hamalert-cli`.

Optionally, install it to your PATH:

```bash
cargo install --path .
```

## Configuration

Create a configuration file at `~/.config/hamalert/config.toml` with your HamAlert credentials:

```toml
username = "your_username"
password = "your_password"
```

### Custom Configuration Path

You can specify a different configuration file location using the `--config-file` flag:

```bash
hamalert-cli --config-file /path/to/config.toml <command>
```

## Usage

### Add a Trigger

Add callsign triggers with specified actions. You can add multiple callsigns in a single command, and each will receive its own trigger via a separate API call:

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --comment "Monitor W1AW activity" \
  --actions url \
  --actions app
```

#### Available Actions

- `url` - HTTP/webhook notification
- `app` - Mobile app notification
- `threema` - Threema messenger notification
- `telnet` - Telnet cluster notification

You can specify multiple actions by repeating the `--actions` flag.

#### Available Modes

You can optionally specify a mode to filter alerts by transmission mode:

- `cw` - CW (Morse code)
- `ft8` - FT8 digital mode
- `ssb` - SSB (Single Side Band)

The `--mode` parameter is optional. If not specified, alerts will trigger for all modes.

### Examples

Monitor a specific callsign with app notifications:

```bash
hamalert-cli add-trigger --callsign K3LR --comment "K3LR spotted" --actions app
```

Add multiple callsigns at once (each gets its own trigger):

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --callsign K3LR \
  --callsign DX1DX \
  --comment "Multiple DX stations" \
  --actions app
```

Monitor a callsign only for FT8 activity:

```bash
hamalert-cli add-trigger \
  --callsign VP8LP \
  --comment "FT8 only" \
  --actions app \
  --mode ft8
```

Monitor multiple callsigns for CW mode with multiple notification methods:

```bash
hamalert-cli add-trigger \
  --callsign W1AW \
  --callsign K3LR \
  --comment "CW operators" \
  --actions url \
  --actions app \
  --mode cw
```

## License

MIT License - see [LICENSE](LICENSE) file for details.
