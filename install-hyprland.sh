#!/bin/sh
set -e

echo "Building release binary..."
nix-shell --run "cargo build --release"

echo "Stopping any running instances..."
killall hyprhue || true

echo "Installing binary to ~/dotfiles/Hyprland/.config/..."
sudo cp target/release/hyprhue ~/dotfiles/Hyprland/.config/

echo "Done! Hyprhue has been installed and will be available in your Hyprland configuration."
