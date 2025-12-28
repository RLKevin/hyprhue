# HyprHue

A high-performance screen-syncing application for Philips Hue lights on Wayland written in Rust (by the way). Bridge discovery uses `discovery.meethue.com`, but the app works entirely locally after that.

## Prerequisites

1.  **Rust**: Install via [rustup.rs](https://rustup.rs).
2.  **Hue Bridge**: Must be on the same network.
3.  **Entertainment Area**: Create one in the official Hue app.

## How to Run

### Option 1: Run with Cargo (Development)

```bash
cargo run
```

### Option 2: Build and Run Binary (Recommended)

You can build a standalone binary so you don't need to use `cargo` every time.

1.  **Build the release binary**:

    ```bash
    cargo build --release
    ```

2.  **Run the binary**:

    ```bash
    ./target/release/hyprhue
    ```

    You can also move this binary to somewhere in your PATH (like `/usr/local/bin`) to run it from anywhere:

    ```bash
    sudo cp target/release/hyprhue /usr/local/bin/
    ```

## Configuration

The configuration is saved to `~/.config/hypr/hyprhue.conf`. You can edit this file to change settings.

-   `brightness`: A multiplier for the brightness of the lights (default: 1.0). Increase this (e.g., 1.5) if the lights are too dim.

## First Run Setup

1.  The app will automatically discover your Hue Bridge.
2.  When prompted, press the **Link Button** on your Bridge.
3.  Select the **Entertainment Group** you want to sync with.
4.  Configuration is saved to `~/.config/hypr/hyprhue.conf`.
