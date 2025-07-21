# aw-watcher-input-rs

A Rust implementation of [aw-watcher-input](https://github.com/ActivityWatch/aw-watcher-input) for [ActivityWatch](https://activitywatch.net/) that follows the same bucket format as the original Python implementation.

## Features

- Uses the same data format as the Python aw-watcher-input
- Tracks keyboard and mouse inputs using rdev
- Monitors key presses, mouse clicks, mouse movement, and scrolling
- Configurable polling interval
- Command-line arguments for testing mode and server connection options

## Installation

### Prerequisites

- Rust and Cargo installed
- ActivityWatch server running

### Building from source

```bash
git clone https://github.com/yourusername/aw-watcher-input-rs.git
cd aw-watcher-input-rs
cargo build --release
```

To enable the unstable grab feature for Linux (which allows intercepting events):

```bash
cargo build --release --features="unstable_grab"
```

The compiled binary will be in `target/release/aw-watcher-input-rs`.

### Platform Support

This watcher uses the `rdev` crate to provide cross-platform input detection:
- macOS: Works via macOS accessibility APIs
- Windows: Uses Windows input hooks
- Linux: Works with X11 and Wayland (when using the unstable_grab feature)

The watcher tracks:
- Key presses (without logging specific keys for privacy)
- Mouse clicks
- Mouse movement
- Scroll wheel activity

## Usage

### Running the watcher

Basic usage:

```bash
cargo run
```

With command-line options:

```bash
cargo run -- --host localhost --port 5600 --testing
```

Or if you've built the release version:

```bash
./target/release/aw-watcher-input-rs
```

Available command-line options:
- `--host`: ActivityWatch server hostname (default: localhost)
- `--port`: ActivityWatch server port (default: 5600)
- `--testing`: Use testing mode (creates a separate bucket)
- `--poll-time`: Override the polling interval from config (in seconds)

### Using the Grab Feature on Linux

When built with the `unstable_grab` feature, the watcher will use rdev's grab functionality on Linux, which allows it to intercept all input events before they are delivered to applications.

For this to work properly:

1. Build with the grab feature enabled:
   ```bash
   cargo build --release --features="unstable_grab"
   ```

2. Make sure your user is in the `input` group (or run as root, not recommended):
   ```bash
   sudo usermod -a -G input $USER
   ```
   You'll need to log out and back in for this change to take effect.

3. Run the watcher as usual:
   ```bash
   ./target/release/aw-watcher-input-rs
   ```

The grab feature works with both X11 and Wayland on Linux, providing more reliable input detection.

### Configuration

The watcher will create a default configuration file at:

- Linux/macOS: `~/.config/activitywatch/aw-watcher-input/config.toml`
- Windows: `%APPDATA%\activitywatch\aw-watcher-input\config.toml`

You can edit this file to change settings:

```toml
# Polling interval in seconds
polling_interval = 1
```

## Data Structure

The watcher records the following data for each heartbeat:

- `presses`: Number of keypresses detected
- `clicks`: Number of mouse clicks
- `deltaX`: Horizontal mouse movement
- `deltaY`: Vertical mouse movement
- `scrollX`: Horizontal scroll distance
- `scrollY`: Vertical scroll distance

Data is stored in a bucket named `aw-watcher-input_{hostname}` with the event type `os.hid.input`, which is the same format used by the Python implementation of aw-watcher-input.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Implementation Details

This watcher follows the same pattern as the Python implementation:

1. It creates a bucket with the ID `aw-watcher-input_{hostname}`
2. It uses the event type `os.hid.input` for compatibility
3. It sets up the data structure for keypresses, mouse clicks, movement, and scrolling
4. It sends heartbeats with the collected data at regular intervals
5. It uses pulsetime to merge events with no input activity

The watcher effectively tracks input activity and integrates with ActivityWatch to provide comprehensive input monitoring.

## Acknowledgments

- Modeled after [aw-watcher-input](https://github.com/ActivityWatch/aw-watcher-input)
- Follows the data format used by [aw-watcher-afk](https://github.com/ActivityWatch/aw-watcher-afk)
- Uses the [ActivityWatch](https://activitywatch.net/) client library

## Current Status and Future Development

This project has implemented cross-platform input detection using the `rdev` crate, which provides monitoring of keyboard and mouse activity. On Linux, it can use the unstable_grab feature for more reliable event interception.

### Roadmap:

1. **Current Status:**
   - Cross-platform input detection using rdev
   - Keyboard and mouse tracking
   - Scroll wheel detection
   - Proper handling of interruption with Ctrl+C

2. **Future Enhancements:**
   - Add more accurate input tracking methods
   - Implement configuration options for sensitivity and thresholds
   - Add visualization tools similar to the Python implementation
   - Improve error handling and performance
   - Add optional detailed key logging (with privacy considerations)
   - Extend the grab feature to more platforms where possible

### Contributing

If you're interested in helping implement platform-specific input detection, your contributions would be especially valuable!
