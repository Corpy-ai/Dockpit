#!/bin/bash

# Docker Manager v3.0 Cross-Platform Builder
# Builds binaries for multiple platforms
# Author: uniCommerce Team

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

APP_NAME="docker-manager"
VERSION="3.0.0"
DIST_DIR="dist"

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${CYAN}"
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║               Docker Manager v3.0 Cross-Platform Builder        ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Target platforms
declare -A TARGETS=(
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-x86_64-static"]="x86_64-unknown-linux-musl"
    ["linux-arm64"]="aarch64-unknown-linux-gnu"
    ["windows-x86_64"]="x86_64-pc-windows-gnu"
    ["macos-x86_64"]="x86_64-apple-darwin"
    ["macos-arm64"]="aarch64-apple-darwin"
)

# Check if cross is installed
check_cross() {
    if ! command -v cross &> /dev/null; then
        print_warning "Installing cross for cross-compilation..."
        cargo install cross --git https://github.com/cross-rs/cross || {
            print_error "Failed to install cross"
            exit 1
        }
    fi
    print_success "Cross-compilation tool ready"
}

# Install target
install_target() {
    local target=$1
    print_info "Installing target: $target"
    rustup target add "$target" || {
        print_warning "Could not add target $target via rustup, will try with cross"
    }
}

# Build for target
build_target() {
    local platform=$1
    local target=$2
    local output_dir="$DIST_DIR/$platform"
    
    print_info "Building for $platform ($target)..."
    
    mkdir -p "$output_dir"
    
    # Use cross for better compatibility, fallback to cargo
    if cross build --release --target "$target"; then
        print_success "Built with cross for $target"
    elif cargo build --release --target "$target" 2>/dev/null; then
        print_success "Built with cargo for $target"
    else
        print_error "Failed to build for $target"
        return 1
    fi
    
    # Copy binary to dist directory
    local binary_name="$APP_NAME"
    local source_path="target/$target/release/$binary_name"
    
    # Windows executables have .exe extension
    if [[ "$platform" == *"windows"* ]]; then
        binary_name="${APP_NAME}.exe"
        source_path="target/$target/release/${APP_NAME}.exe"
    fi
    
    if [[ -f "$source_path" ]]; then
        cp "$source_path" "$output_dir/$binary_name"
        chmod +x "$output_dir/$binary_name" 2>/dev/null || true
        
        # Get binary size
        local size=$(du -h "$output_dir/$binary_name" | cut -f1)
        print_success "Binary copied to $output_dir/$binary_name ($size)"
        
        # Create README for platform
        create_platform_readme "$platform" "$output_dir"
        
        # Create platform-specific installer
        create_platform_installer "$platform" "$output_dir"
        
    else
        print_error "Binary not found at $source_path"
        return 1
    fi
}

# Create README for each platform
create_platform_readme() {
    local platform=$1
    local output_dir=$2
    
    cat > "$output_dir/README.md" << EOF
# Docker Manager v${VERSION} - ${platform}

Fast, efficient Docker container management with modern TUI interface.

## Features
- ✨ Modern terminal interface with perfect rendering
- 🚀 Real-time container monitoring and logs
- 📊 CPU, memory, and network statistics
- 🔄 Container lifecycle management (start, stop, restart, pause)
- 📋 Smart clipboard integration with multiple copy options
- 🌐 Network information and port mapping display
- ⌨️ Intuitive keyboard navigation and shortcuts

## Quick Start

### Installation
Run the included installer:
EOF

    if [[ "$platform" == *"windows"* ]]; then
        echo "- \`install.bat\` - Windows installer" >> "$output_dir/README.md"
        echo "" >> "$output_dir/README.md"
        echo "### Manual Installation" >> "$output_dir/README.md"
        echo "1. Copy \`docker-manager.exe\` to a directory in your PATH" >> "$output_dir/README.md"
        echo "2. Run from Command Prompt or PowerShell: \`docker-manager\`" >> "$output_dir/README.md"
    else
        echo "- \`./install.sh\` - Automatic installation" >> "$output_dir/README.md"
        echo "" >> "$output_dir/README.md"
        echo "### Manual Installation" >> "$output_dir/README.md"
        echo "1. Copy \`docker-manager\` to \`/usr/local/bin/\` or \`~/.local/bin/\`" >> "$output_dir/README.md"
        echo "2. Make executable: \`chmod +x docker-manager\`" >> "$output_dir/README.md"
        echo "3. Run: \`docker-manager\`" >> "$output_dir/README.md"
    fi

    cat >> "$output_dir/README.md" << EOF

## Usage
- **Navigate**: Use ↑↓ arrow keys or click containers
- **Jump to container**: Type any number (1-999) + Enter
- **View logs**: Press 'L' or right arrow
- **View stats**: Press 'S' 
- **Clipboard**: Press 'C' to copy logs (50, 100, 300, 500, 1000 lines or all)
- **Docker operations**: Press 'D' for start/stop/restart/pause/remove
- **Full screen logs**: Press 'F' to toggle expanded view
- **Quick restart**: Press 'R'
- **Quit**: Press 'Q'

## Requirements
- Docker installed and running
- Terminal with color support (recommended)
- Unix-like system (Linux, macOS, WSL)

## Support
For issues and updates, visit: https://github.com/your-repo/docker-manager

Built with ❤️ using Rust and Ratatui
EOF
}

# Create platform-specific installer
create_platform_installer() {
    local platform=$1
    local output_dir=$2
    
    if [[ "$platform" == *"windows"* ]]; then
        # Windows batch installer
        cat > "$output_dir/install.bat" << 'EOF'
@echo off
echo Docker Manager v3.0 Windows Installer
echo.

REM Check if running as administrator
net session >nul 2>&1
if %errorLevel% == 0 (
    echo Installing system-wide...
    set INSTALL_DIR=%ProgramFiles%\DockerManager
    mkdir "%INSTALL_DIR%" 2>nul
    copy docker-manager.exe "%INSTALL_DIR%\" >nul
    
    REM Add to PATH if not already there
    echo %PATH% | find /i "%INSTALL_DIR%" >nul || (
        setx PATH "%PATH%;%INSTALL_DIR%" /M
        echo Added to system PATH
    )
    
    echo Installation completed successfully!
    echo Run 'docker-manager' from any command prompt
) else (
    echo Installing for current user...
    set INSTALL_DIR=%LOCALAPPDATA%\Programs\DockerManager
    mkdir "%INSTALL_DIR%" 2>nul
    copy docker-manager.exe "%INSTALL_DIR%\" >nul
    
    REM Add to user PATH
    for /f "tokens=2*" %%a in ('reg query HKCU\Environment /v PATH 2^>nul') do set USER_PATH=%%b
    echo %USER_PATH% | find /i "%INSTALL_DIR%" >nul || (
        setx PATH "%USER_PATH%;%INSTALL_DIR%"
        echo Added to user PATH
    )
    
    echo Installation completed successfully!
    echo Restart your command prompt and run 'docker-manager'
)

pause
EOF
    else
        # Unix installer (Linux/macOS)
        cp install-universal.sh "$output_dir/install.sh"
        chmod +x "$output_dir/install.sh"
        
        # Create a simple installer that copies the binary
        cat > "$output_dir/install-simple.sh" << EOF
#!/bin/bash
# Simple installer for Docker Manager v${VERSION}

set -e

APP_NAME="docker-manager"
INSTALL_DIR="\${1:-/usr/local/bin}"

echo "Installing Docker Manager to \$INSTALL_DIR..."

if [[ \$EUID -ne 0 ]] && [[ "\$INSTALL_DIR" == "/usr/local/bin" ]]; then
    echo "Note: Installing to system directory requires sudo"
    sudo cp "\$APP_NAME" "\$INSTALL_DIR/"
    sudo chmod +x "\$INSTALL_DIR/\$APP_NAME"
else
    mkdir -p "\$INSTALL_DIR"
    cp "\$APP_NAME" "\$INSTALL_DIR/"
    chmod +x "\$INSTALL_DIR/\$APP_NAME"
fi

echo "Installation completed successfully!"
echo "Run: \$APP_NAME"
EOF
        chmod +x "$output_dir/install-simple.sh"
    fi
}

# Build all targets
build_all() {
    print_header
    print_info "Starting cross-platform build..."
    
    # Clean previous builds
    if [[ -d "$DIST_DIR" ]]; then
        rm -rf "$DIST_DIR"
    fi
    mkdir -p "$DIST_DIR"
    
    # Check tools
    check_cross
    
    local success_count=0
    local total_count=${#TARGETS[@]}
    
    for platform in "${!TARGETS[@]}"; do
        local target="${TARGETS[$platform]}"
        
        print_info "Processing $platform..."
        install_target "$target"
        
        if build_target "$platform" "$target"; then
            ((success_count++))
        fi
        
        echo "" # Separator
    done
    
    # Summary
    print_info "Build Summary:"
    print_success "$success_count/$total_count targets built successfully"
    
    if [[ -d "$DIST_DIR" ]]; then
        print_info "Distribution files created in: $DIST_DIR"
        ls -la "$DIST_DIR"
    fi
    
    # Create release archive
    create_release_archive
}

# Create release archive
create_release_archive() {
    print_info "Creating release archive..."
    
    local archive_name="docker-manager-v${VERSION}-multi-platform"
    
    # Create main README
    cat > "$DIST_DIR/README.md" << EOF
# Docker Manager v${VERSION} - Multi-Platform Release

This package contains Docker Manager binaries for multiple platforms.

## Available Platforms

EOF
    
    for platform in "${!TARGETS[@]}"; do
        if [[ -d "$DIST_DIR/$platform" ]]; then
            echo "- **$platform**: \`$platform/\`" >> "$DIST_DIR/README.md"
        fi
    done
    
    cat >> "$DIST_DIR/README.md" << EOF

## Installation

Choose your platform directory and follow the included README.md and installer.

## Universal Features
- Fast Docker container management
- Real-time monitoring and logs  
- Cross-platform compatibility
- Modern terminal interface
- Smart clipboard integration

Built with Rust for maximum performance and reliability.
EOF
    
    # Create archive
    if command -v tar &> /dev/null; then
        tar -czf "${archive_name}.tar.gz" -C "$DIST_DIR" .
        print_success "Release archive created: ${archive_name}.tar.gz"
    fi
    
    if command -v zip &> /dev/null; then
        (cd "$DIST_DIR" && zip -r "../${archive_name}.zip" .)
        print_success "Release archive created: ${archive_name}.zip"
    fi
}

# Main function
case "${1:-all}" in
    "all")
        build_all
        ;;
    "linux")
        build_target "linux-x86_64" "${TARGETS[linux-x86_64]}"
        build_target "linux-x86_64-static" "${TARGETS[linux-x86_64-static]}"
        ;;
    "windows")
        build_target "windows-x86_64" "${TARGETS[windows-x86_64]}"
        ;;
    "macos")
        build_target "macos-x86_64" "${TARGETS[macos-x86_64]}"
        build_target "macos-arm64" "${TARGETS[macos-arm64]}"
        ;;
    "--help"|"-h"|"help")
        echo "Docker Manager Cross-Platform Builder"
        echo "Usage: $0 [target]"
        echo ""
        echo "Targets:"
        echo "  all      Build for all platforms (default)"
        echo "  linux    Build Linux variants"
        echo "  windows  Build Windows variant"
        echo "  macos    Build macOS variants" 
        echo "  help     Show this help"
        ;;
    *)
        print_error "Unknown target: $1"
        exit 1
        ;;
esac