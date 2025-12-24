mod hue;
mod capturer;
mod processor;

use anyhow::Result;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting HyprHue...");

    // 1. Load or Setup Config
    let config = load_or_setup_config().await?;
    let bridge = hue::Bridge::new(config.clone())?;

    // 2. Setup Screen Capture
    let mut capturer = capturer::setup()?;

    // 3. Start Stream
    println!("Activating Entertainment Stream on Bridge...");
    bridge.start_stream().await?;
    
    println!("Waiting for Bridge to switch modes...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("Connecting to DTLS Stream...");
    let mut stream = hue::HueStream::connect(&config.ip, &config.username, &config.clientkey).await?;
    println!("Stream Connected! Syncing at 50 FPS...");

    // 4. Main Loop
    println!("Press Ctrl+C to stop.");
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
        
        let (lr, lg, lb) = left_color;
        let (rr, rg, rb) = right_color;

        // Log the color
        print!("\rLeft: \x1b[48;2;{};{};{}m   \x1b[0m Right: \x1b[48;2;{};{};{}m   \x1b[0m", lr, lg, lb, rr, rg, rb);
        std::io::stdout().flush()?;
        
        // Send to Hue via DTLS
        if let Err(e) = stream.send_colors(&config.light_ids, left_color, right_color).await {
            eprintln!("\nError sending stream: {}", e);
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

    println!("No valid config found. Starting setup...");
    
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
    println!("Config saved to {}", config_path.display());

    Ok(config)
}
