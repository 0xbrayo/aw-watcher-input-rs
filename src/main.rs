use aw_client_rust::blocking::AwClient;
use aw_models::Event;
use chrono::{TimeDelta, Utc};
use clap::Parser;
use config::{Config, ConfigError, File};
use dirs::config_dir;
use hostname::get as get_hostname;
// Use the grab function on Linux when the unstable_grab feature is enabled
// This allows intercepting all input events before they are delivered to applications
#[cfg(all(target_os = "linux", feature = "unstable_grab"))]
use rdev::{grab, Event as RdevEvent, EventType};
// Use the standard listen function on all other platforms
#[cfg(not(all(target_os = "linux", feature = "unstable_grab")))]
use rdev::{listen, Event as RdevEvent, EventType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::{create_dir_all, write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep, JoinHandle};
use std::time::{Duration, Instant};

/// Configuration structure for aw-watcher-input
#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    /// Polling interval in seconds
    #[serde(default = "default_polling_interval")]
    polling_interval: u64,
}

fn default_polling_interval() -> u64 {
    1
}

impl AppConfig {
    fn new() -> Result<Self, ConfigError> {
        let default_config = Self {
            polling_interval: default_polling_interval(),
        };

        let config_path = if let Some(config_dir) = config_dir() {
            let aw_config_dir = config_dir.join("activitywatch").join("aw-watcher-input");

            create_dir_all(&aw_config_dir).ok();

            let config_file = aw_config_dir.join("config.toml");

            if !config_file.exists() {
                let default_config_str = toml::to_string_pretty(&default_config).unwrap();
                write(&config_file, default_config_str).ok();
            }

            Some(config_file)
        } else {
            None
        };

        let mut builder = Config::builder();

        if let Some(path) = config_path {
            if path.exists() {
                builder = builder.add_source(File::from(path));
            }
        }

        match builder.build()?.try_deserialize() {
            Ok(config) => Ok(config),
            Err(_) => Ok(default_config),
        }
    }
}

#[derive(Debug, Clone)]
struct InputState {
    presses: u64,
    clicks: u64,
    delta_x: u64,
    delta_y: u64,
    scroll_x: u64,
    scroll_y: u64,
    last_activity: Instant,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            presses: 0,
            clicks: 0,
            delta_x: 0,
            delta_y: 0,
            scroll_x: 0,
            scroll_y: 0,
            last_activity: Instant::now(),
        }
    }
}

// Global atomic for signaling threads to stop
static RUNNING: AtomicBool = AtomicBool::new(true);

fn create_input_listener_thread(state: Arc<Mutex<InputState>>) -> JoinHandle<()> {
    thread::spawn(move || {
        // Set up the callback for input events
        let state_clone = Arc::clone(&state);

        // Standard input listening mode for non-Linux platforms or when unstable_grab is not enabled
        #[cfg(not(all(target_os = "linux", feature = "unstable_grab")))]
        {
            let callback = move |event: RdevEvent| {
                // Check if we should continue running
                if !RUNNING.load(Ordering::SeqCst) {
                    // Force exit from the listen loop by panicking the callback
                    // This is the only reliable way to exit rdev::listen
                    std::process::exit(0);
                }

                let now = Instant::now();
                let mut update_activity = false;

                // Lock the state to update
                if let Ok(mut state_guard) = state_clone.lock() {
                    match event.event_type {
                        EventType::KeyPress(_) => {
                            state_guard.presses += 1;
                            update_activity = true;
                        }
                        EventType::ButtonPress(_) => {
                            state_guard.clicks += 1;
                            update_activity = true;
                        }
                        EventType::MouseMove { x: _, y: _ } => {
                            state_guard.delta_x += 1;
                            state_guard.delta_y += 1;
                            update_activity = true;
                        }
                        EventType::Wheel { delta_x, delta_y } => {
                            state_guard.scroll_x += delta_x.unsigned_abs();
                            state_guard.scroll_y += delta_y.unsigned_abs();
                            update_activity = true;
                        }
                        _ => {}
                    }

                    if update_activity {
                        state_guard.last_activity = now;
                    }
                }
            };

            // Start listening for input events
            // Note: This is a blocking call that runs until the process exits
            if let Err(error) = listen(callback) {
                eprintln!("Error listening for input events: {:?}", error);
            }
        }

        // Use the grab feature on Linux when enabled
        // This intercepts events before they reach applications
        #[cfg(all(target_os = "linux", feature = "unstable_grab"))]
        {
            let callback = move |event: RdevEvent| -> Option<RdevEvent> {
                // Check if we should continue running
                if !RUNNING.load(Ordering::SeqCst) {
                    // Force exit from the grab loop
                    std::process::exit(0);
                }

                let now = Instant::now();
                let mut update_activity = false;

                // Lock the state to update
                if let Ok(mut state_guard) = state_clone.lock() {
                    match event.event_type {
                        EventType::KeyPress(_) => {
                            state_guard.presses += 1;
                            update_activity = true;
                        }
                        EventType::ButtonPress(_) => {
                            state_guard.clicks += 1;
                            update_activity = true;
                        }
                        EventType::MouseMove { x: _, y: _ } => {
                            state_guard.delta_x += 1;
                            state_guard.delta_y += 1;
                            update_activity = true;
                        }
                        EventType::Wheel { delta_x, delta_y } => {
                            state_guard.scroll_x += delta_x.unsigned_abs();
                            state_guard.scroll_y += delta_y.unsigned_abs();
                            update_activity = true;
                        }
                        _ => {}
                    }

                    if update_activity {
                        state_guard.last_activity = now;
                    }
                }

                // Return the event to pass it through without modification
                Some(event)
            };

            // Start grabbing input events
            // Note: This is a blocking call that runs until the process exits
            if let Err(error) = grab(callback) {
                eprintln!("Error grabbing input events: {:?}", error);
                eprintln!("Note: On Linux, this program must be run as root or by a user in the 'input' group");
                eprintln!("To add your user to the input group: sudo usermod -a -G input $USER");
                eprintln!("You may need to log out and back in for the changes to take effect");
                std::process::exit(1);
            }
        }
    })
}

/// Command line arguments for aw-watcher-input
#[derive(Parser, Debug)]
#[clap(author, version, about = "ActivityWatch Input Watcher")]
struct Args {
    /// ActivityWatch server hostname
    #[clap(long, default_value = "localhost")]
    host: String,

    /// ActivityWatch server port
    #[clap(long, default_value = "5600")]
    port: u16,

    /// Use testing mode (different bucket)
    #[clap(long)]
    testing: bool,

    /// Override the polling interval from config (in seconds)
    #[clap(long)]
    poll_time: Option<u64>,
}

fn main() {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match AppConfig::new() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            AppConfig {
                polling_interval: default_polling_interval(),
            }
        }
    };

    // Use poll_time from args if provided, otherwise from config
    let polling_interval = args.poll_time.unwrap_or(config.polling_interval);

    // Get hostname and create bucket ID with hostname appended
    let hostname = match get_hostname() {
        Ok(name) => name.to_string_lossy().into_owned(),
        Err(_) => "unknown-host".to_string(),
    };

    // Add testing suffix if in testing mode
    let bucket_id = if args.testing {
        format!("aw-watcher-input-testing_{}", hostname)
    } else {
        format!("aw-watcher-input_{}", hostname)
    };
    let event_type = "os.hid.input";

    println!(
        "Starting aw-watcher-input-rs with polling interval of {} seconds",
        polling_interval
    );
    println!("Using bucket ID: {}", bucket_id);
    println!("Connecting to aw-server at {}:{}", args.host, args.port);
    if args.testing {
        println!("Running in testing mode");
    }

    // Set up Ctrl+C handler
    RUNNING.store(true, Ordering::SeqCst);
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C, shutting down gracefully...");
        RUNNING.store(false, Ordering::SeqCst);

        // Give the application a moment to clean up
        sleep(Duration::from_millis(100));

        // Force exit if graceful shutdown doesn't work
        // This is necessary because rdev::listen cannot be interrupted gracefully
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let client = AwClient::new(&args.host, args.port, "aw-watcher-input").unwrap();

    // Create or get bucket
    client
        .create_bucket_simple(&bucket_id, event_type)
        .expect("Failed to create input bucket");

    // Setup shared state for input monitoring
    let input_state = Arc::new(Mutex::new(InputState::default()));

    // Start the input monitoring thread
    let _listener_thread = create_input_listener_thread(Arc::clone(&input_state));

    println!("Input monitoring thread started");

    #[cfg(not(all(target_os = "linux", feature = "unstable_grab")))]
    println!("Input detection is now active using rdev listen mode");

    #[cfg(all(target_os = "linux", feature = "unstable_grab"))]
    {
        println!("Input detection is now active using rdev grab mode (Linux)");
        println!("NOTE: This requires your user to be in the 'input' group or to run as root");
        println!("To add your user to the input group: sudo usermod -a -G input $USER");
        println!("On some distributions, you may need to use the 'plugdev' group instead");
        println!("You must log out and back in for group changes to take effect");
    }

    println!("Press Ctrl+C to exit");

    // Main polling loop
    while RUNNING.load(Ordering::SeqCst) {
        // Record the start time of this iteration
        let loop_start = Instant::now();
        let timestamp = Utc::now();

        // Get current input state and reset counters
        let data = {
            if let Ok(mut state_guard) = input_state.lock() {
                let data = InputState {
                    presses: state_guard.presses,
                    clicks: state_guard.clicks,
                    delta_x: state_guard.delta_x,
                    delta_y: state_guard.delta_y,
                    scroll_x: state_guard.scroll_x,
                    scroll_y: state_guard.scroll_y,
                    last_activity: state_guard.last_activity,
                };

                // Reset counters for the next period, but keep the last_activity time
                let last_activity = state_guard.last_activity;
                *state_guard = InputState {
                    last_activity,
                    ..Default::default()
                };
                data
            } else {
                // If we can't lock the state, use default values
                InputState::default()
            }
        };

        // Create event data
        let mut data_map = Map::new();
        data_map.insert("presses".to_string(), Value::Number(data.presses.into()));
        data_map.insert("clicks".to_string(), Value::Number(data.clicks.into()));
        data_map.insert("deltaX".to_string(), Value::Number(data.delta_x.into()));
        data_map.insert("deltaY".to_string(), Value::Number(data.delta_y.into()));
        data_map.insert("scrollX".to_string(), Value::Number(data.scroll_x.into()));
        data_map.insert("scrollY".to_string(), Value::Number(data.scroll_y.into()));

        let event = Event {
            id: None,
            timestamp,
            duration: TimeDelta::seconds(polling_interval as i64),
            data: data_map.clone(),
        };

        // Calculate pulsetime based on whether input occurred during this polling interval
        let pulsetime = polling_interval as f64 + 0.1;

        // Debug output
        println!(
            "Heartbeat: presses={}, clicks={}, deltaX={}, deltaY={}, scrollX={}, scrollY={}",
            data.presses, data.clicks, data.delta_x, data.delta_y, data.scroll_x, data.scroll_y
        );

        // Send the heartbeat
        match client.heartbeat(&bucket_id, &event, pulsetime) {
            Ok(_) => (),
            Err(e) => eprintln!("Error sending heartbeat: {}", e),
        }

        // Calculate how much time has elapsed in this iteration
        let elapsed = loop_start.elapsed();

        // Calculate the time to sleep to maintain consistent intervals
        if elapsed < Duration::from_secs(polling_interval) {
            let sleep_time = Duration::from_secs(polling_interval) - elapsed;

            // Sleep in smaller intervals to be more responsive to shutdown signals
            let sleep_interval = Duration::from_millis(100);
            let mut remaining = sleep_time;

            while remaining > Duration::from_millis(0) && RUNNING.load(Ordering::SeqCst) {
                let current_sleep = if remaining > sleep_interval {
                    sleep_interval
                } else {
                    remaining
                };
                sleep(current_sleep);
                remaining = remaining.saturating_sub(current_sleep);
            }
        } else {
            // If operations took longer than polling_interval, don't sleep
            // but log a warning about the missed interval
            eprintln!(
                "Warning: Operations took longer than polling interval ({:?} > {}s)",
                elapsed, polling_interval
            );
        }
    }

    println!("Graceful shutdown complete.");
}
