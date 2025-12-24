# HyprHue

A screen-syncing application for Philips Hue lights on Linux (Wayland/Hyprland).

## Prerequisites

1.  **Rust**: Install via [rustup.rs](https://rustup.rs).
2.  **Hue Bridge**: Must be on the same network.
3.  **Entertainment Area**: Create one in the official Hue app.

## How to Run

```bash
cargo run
```

## First Run Setup

1.  The app will automatically discover your Hue Bridge.
2.  When prompted, press the **Link Button** on your Bridge.
3.  Select the **Entertainment Group** you want to sync with.
4.  Configuration is saved to `hyprhue_config.json`.
