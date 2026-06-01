#!/bin/bash

# Docker Manager v3.0 - Local Installation Script
# Instala Docker Manager en el directorio local del usuario

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$SCRIPT_DIR/target/release/docker-manager"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_PATH="$INSTALL_DIR/docker-manager"

echo "🔧 Docker Manager v3.0 - Local Installation"
echo "============================================="

# Crear directorio si no existe
mkdir -p "$INSTALL_DIR"

# Verificar que el binario existe
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Error: Binary not found at $BINARY_PATH"
    echo "Run 'cargo build --release' first"
    exit 1
fi

# Copiar binario
echo "📁 Installing to: $INSTALL_PATH"
cp "$BINARY_PATH" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

# Verificar instalación
if [ -f "$INSTALL_PATH" ]; then
    echo "✅ Docker Manager v3.0 installed successfully!"
    echo ""
    echo "📝 Usage:"
    echo "   docker-manager           # Run from anywhere"
    echo "   ~/.local/bin/docker-manager  # Full path"
    echo ""
    echo "⚠️  Make sure ~/.local/bin is in your PATH:"
    echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
    echo "🎯 Features:"
    echo "   • Auto-scroll to latest logs (proportional to view size)"
    echo "   • Free navigation through entire log history"
    echo "   • Copy options: 50/100/400/1000 lines"
    echo "   • Dynamic viewport calculations"
else
    echo "❌ Installation failed"
    exit 1
fi