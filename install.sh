#!/bin/bash
set -e

echo "Building release binary..."
cargo build --release

echo "Installing binary to /usr/local/bin/..."
sudo cp target/release/hyprhue /usr/local/bin/

echo "Done! You can now run 'hyprhue' from anywhere."
