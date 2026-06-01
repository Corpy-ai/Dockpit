#!/bin/bash

# Dockpit v3.0 - Launcher Script
# Ejecuta el Dockpit optimizado desde el directorio local

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$SCRIPT_DIR/target/release/dockpit"

# Verificar que el binario existe
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Error: Binary not found at $BINARY_PATH"
    echo "Run 'cargo build --release' first"
    exit 1
fi

# Verificar permisos de Docker
if ! docker info >/dev/null 2>&1; then
    echo "❌ Error: Docker is not running or you don't have permission to access Docker"
    echo "Try: sudo usermod -aG docker $USER && newgrp docker"
    exit 1
fi

# Ejecutar Dockpit
echo "🚀 Starting Dockpit v3.0..."
echo "📍 Binary: $BINARY_PATH"
echo ""

"$BINARY_PATH"