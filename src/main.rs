mod hue;
mod capturer;
mod processor;

use anyhow::Result;
use simplelog::*;
use log::{info, error};
use notify_rust::Notification;

fn is_network_not_ready_error(e: &anyhow::Error) -> bool {
    let has_unreachable_io = e.chain().any(|cause| {
        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
            matches!(
                io_err.kind(),
                std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
                    | std::io::ErrorKind::AddrNotAvailable
                    | std::io::ErrorKind::NotConnected
            )
        } else {
            false
        }
    });

    if has_unreachable_io {
        return true;
    }

    // Fallback for wrapped transport errors that do not preserve io::ErrorKind.
    let msg = format!("{:#}", e).to_lowercase();
    msg.contains("network is unreachable")
        || msg.contains("no route to host")
        || msg.contains("host is unreachable")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let _ = TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto);

    // Panic hook to log panics
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        error!("Panic occurred: {:?}", panic_info);
        let _ = Notification::new()
            .summary("HyprHue Panic")
            .body("Application panicked.")
            .show();
        hook(panic_info);
    }));

    loop {
        match run().await {
            Ok(_) => break Ok(()),
            Err(e) => {
                if is_network_not_ready_error(&e) {
                    error!("Network is not ready yet. Retrying in 2 seconds...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }

                let is_conn_refused = e.chain().any(|cause| {
                    if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
                        io_err.kind() == std::io::ErrorKind::ConnectionRefused
                    } else {
                        false
                    }
                });

                if is_conn_refused {
                    error!("Connection refused. Retrying in 5 seconds...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }

                let err_string = e.to_string();
                if err_string.contains("unauthorized user") || (err_string.contains("resource") && err_string.contains("not available")) {
                    error!("Authentication failed or Group missing. Resetting configuration and restarting setup...");
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                    let config_path = std::path::Path::new(&home).join(".config/hypr/hyprhue.conf");
                    if let Err(del_err) = std::fs::remove_file(&config_path) {
                        error!("Failed to delete config file: {}", del_err);
                    }
                    continue;
                }

                error!("Fatal error: {:?}", e);

                let _ = Notification::new()
                    .summary("HyprHue Error")
                    .body(&format!("Fatal error: {}", e))
                    .show();

                return Err(e);
            }
        }
    }
}

async fn run() -> Result<()> {
    info!("Starting HyprHue...");

    // 1. Load or Setup Config
    let config = load_or_setup_config().await?;
    let bridge = hue::Bridge::new(config.clone())?;

    // 2. Setup Screen Capture
    let mut capturer = capturer::setup()?;

    // 3. Start Stream
    info!("Activating Entertainment Stream on Bridge...");
    bridge.start_stream().await?;
    
    info!("Waiting for Bridge to switch modes...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    info!("Connecting to DTLS Stream...");
    let mut stream = hue::HueStream::connect(&config.ip, &config.username, &config.clientkey).await?;
    info!("Stream Connected! Syncing at 50 FPS...");

    let _ = Notification::new()
        .summary("HyprHue")
        .body("Light syncing enabled")
        .show();

    // 4. Main Loop
    info!("Press Ctrl+C to stop.");
    loop {
        let (frame, width, height, stride) = capturer.capture_frame().await?;
        let (left_color, right_color) = processor::calculate_colors(&frame, width, height, stride);
        
        let (lr, lg, lb) = left_color;
        let (rr, rg, rb) = right_color;

        // Apply brightness modifier
        let b_mod = config.brightness;
        let left_color = (
            (lr as f32 * b_mod).min(255.0) as u8,
            (lg as f32 * b_mod).min(255.0) as u8,
            (lb as f32 * b_mod).min(255.0) as u8,
        );
        let right_color = (
            (rr as f32 * b_mod).min(255.0) as u8,
            (rg as f32 * b_mod).min(255.0) as u8,
            (rb as f32 * b_mod).min(255.0) as u8,
        );
        
        // Log the color
        // print!("\rLeft: \x1b[48;2;{};{};{}m   \x1b[0m Right: \x1b[48;2;{};{};{}m   \x1b[0m", lr, lg, lb, rr, rg, rb);
        // std::io::stdout().flush()?;
        
        // Send to Hue via DTLS
        if let Err(e) = stream.send_colors(&config.light_ids, left_color, right_color).await {
            error!("Error sending stream: {}", e);
            // Try to reconnect or just break?
            // For now, just log.
        }
        
        // 50 FPS = 20ms
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
}

async fn load_or_setup_config() -> Result<hue::BridgeConfig> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_dir = std::path::Path::new(&home).join(".config/hypr");
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    let config_path = config_dir.join("hyprhue.conf");
    
    if let Ok(file) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<hue::BridgeConfig>(&file) {
            // Check if it has the new fields (clientkey)
            if !config.clientkey.is_empty() {
                return Ok(config);
            }
        }
    }

    info!("No valid config found. Starting setup...");
    
    println!("Discovering Hue Bridge...");
    let ip = hue::Bridge::discover().await.or_else(|_| {
        println!("Discovery failed. Please enter Bridge IP manually:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        Ok::<String, anyhow::Error>(input.trim().to_string())
    })?;
    
    println!("Found Bridge at {}. Registering...", ip);
    let (username, clientkey) = hue::Bridge::register(&ip).await?;
    println!("Registered! Username: {}", username);

    println!("Fetching Entertainment Groups...");
    let groups = hue::Bridge::get_entertainment_groups(&ip, &username).await?;
    
    if groups.is_empty() {
        return Err(anyhow::anyhow!("No Entertainment Groups found! Please create one in the Hue App first."));
    }
    
    println!("Available Entertainment Groups:");
    for (i, group) in groups.iter().enumerate() {
        println!("{}: {} (Lights: {:?})", i, group.name, group.lights);
    }
    
    println!("Enter the number of the group you want to sync:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let index: usize = input.trim().parse()?;
    
    let selected_group = groups.get(index).ok_or_else(|| anyhow::anyhow!("Invalid selection"))?;
    
    // Parse light IDs to u16
    let light_ids: Vec<u16> = selected_group.lights.iter()
        .filter_map(|id| id.parse().ok())
        .collect();

    let config = hue::BridgeConfig {
        ip,
        username,
        clientkey,
        group_id: selected_group.id.clone(),
        light_ids,
        brightness: 1.0,
    };

    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    info!("Config saved to {}", config_path.display());

    Ok(config)
}
