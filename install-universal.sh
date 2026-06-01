#!/bin/bash

# Dockpit v3.0 Universal Installer
# Compatible with Linux, macOS, and WSL
# Author: Corpy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="dockpit"
APP_VERSION="3.0.0"
INSTALL_DIR="/usr/local/bin"
BINARY_PATH="./target/release/dockpit"

# Print colored messages
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
    echo "║                    Dockpit v3.0 Installer                ║"
    echo "║                     Universal Installation                       ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Check if running as root for system-wide installation
check_permissions() {
    if [[ $EUID -eq 0 ]]; then
        print_info "Running as root - installing system-wide"
        INSTALL_DIR="/usr/local/bin"
    else
        print_info "Running as user - installing to user directory"
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
        
        # Add to PATH if not already there
        if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
            print_warning "Adding $INSTALL_DIR to PATH in your shell profile"
            case "$SHELL" in
                */zsh)
                    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
                    print_info "Added to ~/.zshrc - please restart your shell or run: source ~/.zshrc"
                    ;;
                */bash)
                    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
                    print_info "Added to ~/.bashrc - please restart your shell or run: source ~/.bashrc"
                    ;;
                *)
                    print_warning "Please manually add $INSTALL_DIR to your PATH"
                    ;;
            esac
        fi
    fi
}

# Detect operating system
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS="linux"
        ARCH=$(uname -m)
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
        ARCH=$(uname -m)
    elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]]; then
        OS="windows"
        ARCH="x86_64"
    else
        OS="unknown"
        ARCH="unknown"
    fi
    
    print_info "Detected OS: $OS ($ARCH)"
}

# Check dependencies
check_dependencies() {
    print_info "Checking dependencies..."
    
    # Check if Docker is installed
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed. Please install Docker first."
        echo "Visit: https://docs.docker.com/get-docker/"
        exit 1
    fi
    
    # Check if Docker is running
    if ! docker info &> /dev/null; then
        print_warning "Docker daemon is not running. Please start Docker."
        print_info "The manager will still install, but won't work until Docker is running."
    else
        print_success "Docker is installed and running"
    fi
    
    # Check terminal capabilities
    if [[ -t 1 ]] && [[ "$(tput colors 2>/dev/null)" -ge 8 ]]; then
        print_success "Terminal supports colors"
    else
        print_warning "Terminal has limited color support - some features may look different"
    fi
}

# Install the binary
install_binary() {
    print_info "Installing Dockpit..."
    
    if [[ ! -f "$BINARY_PATH" ]]; then
        print_error "Binary not found at $BINARY_PATH"
        print_info "Please compile first with: cargo build --release"
        exit 1
    fi
    
    # Copy binary
    cp "$BINARY_PATH" "$INSTALL_DIR/$APP_NAME"
    chmod +x "$INSTALL_DIR/$APP_NAME"
    
    print_success "Binary installed to $INSTALL_DIR/$APP_NAME"
}

# Create desktop entry (Linux only)
create_desktop_entry() {
    if [[ "$OS" == "linux" ]] && command -v xdg-desktop-menu &> /dev/null; then
        print_info "Creating desktop entry..."
        
        DESKTOP_DIR="$HOME/.local/share/applications"
        mkdir -p "$DESKTOP_DIR"
        
        cat > "$DESKTOP_DIR/dockpit.desktop" << EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Dockpit
Comment=Fast and efficient Docker container management
Icon=application-x-executable
Exec=gnome-terminal -- dockpit
Categories=Development;System;
Terminal=true
StartupNotify=false
EOF
        
        if command -v update-desktop-database &> /dev/null; then
            update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
        fi
        
        print_success "Desktop entry created"
    fi
}

# Test installation
test_installation() {
    print_info "Testing installation..."
    
    if command -v "$APP_NAME" &> /dev/null; then
        VERSION_OUTPUT=$("$APP_NAME" --version 2>/dev/null || echo "Dockpit v3.0.0")
        print_success "Installation successful!"
        print_info "Version: $VERSION_OUTPUT"
        print_info "Run '$APP_NAME' to start the Dockpit"
    else
        print_error "Installation failed - command not found in PATH"
        print_info "Try running: $INSTALL_DIR/$APP_NAME"
        exit 1
    fi
}

# Uninstall function
uninstall() {
    print_info "Uninstalling Dockpit..."
    
    # Remove binary
    if [[ -f "$INSTALL_DIR/$APP_NAME" ]]; then
        rm "$INSTALL_DIR/$APP_NAME"
        print_success "Binary removed from $INSTALL_DIR"
    fi
    
    # Remove desktop entry
    if [[ -f "$HOME/.local/share/applications/dockpit.desktop" ]]; then
        rm "$HOME/.local/share/applications/dockpit.desktop"
        print_success "Desktop entry removed"
    fi
    
    print_success "Dockpit uninstalled successfully"
}

# Show usage information
show_usage() {
    echo "Dockpit v3.0 Universal Installer"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  install     Install Dockpit (default)"
    echo "  uninstall   Remove Dockpit"
    echo "  test        Test installation"
    echo "  --help      Show this help message"
    echo ""
    echo "Examples:"
    echo "  sudo $0                    # System-wide installation"
    echo "  $0                         # User installation"
    echo "  $0 uninstall               # Remove installation"
    echo ""
}

# Main installation function
main_install() {
    print_header
    detect_os
    check_permissions
    check_dependencies
    install_binary
    create_desktop_entry
    test_installation
    
    echo ""
    print_success "Installation completed successfully!"
    echo ""
    print_info "Quick Start:"
    echo "  1. Run: $APP_NAME"
    echo "  2. Use arrow keys or 1-9 to navigate containers"
    echo "  3. Press 'L' for logs, 'S' for stats, 'C' for clipboard"
    echo "  4. Press 'Q' to quit"
    echo ""
    print_info "For help and documentation:"
    echo "  $APP_NAME --help"
    echo ""
}

# Handle command line arguments
case "${1:-install}" in
    "install"|"")
        main_install
        ;;
    "uninstall")
        uninstall
        ;;
    "test")
        test_installation
        ;;
    "--help"|"-h"|"help")
        show_usage
        ;;
    *)
        print_error "Unknown option: $1"
        show_usage
        exit 1
        ;;
esac