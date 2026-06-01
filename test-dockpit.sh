#!/bin/bash

# Test script for Dockpit v3.0

echo "🧪 Testing Dockpit v3.0"
echo "=============================="
echo

# Check if binary exists
if [ ! -f "target/release/dockpit" ]; then
    echo "❌ Binary not found. Please run: cargo build --release"
    exit 1
fi

echo "✅ Binary found"

# Test help command
echo
echo "📋 Testing help command..."
./target/release/dockpit --help

# Test list command
echo
echo "📋 Testing list command..."
./target/release/dockpit list

# Instructions for interactive testing
echo
echo "🎮 Interactive Testing"
echo "====================="
echo
echo "Run the TUI mode with:"
echo "  ./target/release/dockpit"
echo
echo "Test these key combinations:"
echo "  1-9     : Jump to container N"
echo "  ↑/↓     : Navigate containers"
echo "  j/k     : Navigate (vim style)"
echo "  L       : Switch to Logs view"
echo "  S       : Switch to Stats view"
echo "  F       : Toggle fullscreen logs"
echo "  D       : Docker operations menu"
echo "  C       : Clipboard menu"
echo "  R       : Restart selected container"
echo "  Q       : Quit"
echo
echo "Docker Operations Menu (D):"
echo "  1 : Start container"
echo "  2 : Stop container"
echo "  3 : Restart container"
echo "  4 : Pause container"
echo "  5 : Unpause container"
echo "  6 : Remove container"
echo
echo "Clipboard Menu (C):"
echo "  1 : Copy last 100 lines"
echo "  2 : Copy last 500 lines"
echo "  3 : Copy all logs"
echo