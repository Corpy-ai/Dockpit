#!/bin/bash

# Docker Manager v3.0 - Installation Script
# Author: uniCommerce Team

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Variables
BINARY_NAME="docker-manager"
INSTALL_DIR="/usr/local/bin"
SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$SOURCE_DIR/target/release/$BINARY_NAME"

# Functions
print_header() {
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}       Docker Manager v3.0 - Rust Edition Installer${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

check_docker() {
    if command -v docker &> /dev/null; then
        print_success "Docker está instalado"
        
        if docker ps &> /dev/null; then
            print_success "Docker daemon está funcionando"
        else
            print_warning "Docker daemon no está funcionando o no tienes permisos"
            echo "  Intenta: sudo systemctl start docker"
            echo "  O añádete al grupo docker: sudo usermod -aG docker \$USER"
        fi
    else
        print_error "Docker no está instalado"
        echo "  Instala Docker primero: https://docs.docker.com/get-docker/"
        exit 1
    fi
}

check_rust() {
    if command -v cargo &> /dev/null; then
        print_success "Rust/Cargo está instalado"
        return 0
    else
        print_warning "Rust/Cargo no está instalado"
        return 1
    fi
}

install_rust() {
    print_warning "¿Deseas instalar Rust ahora? (s/n)"
    read -r response
    if [[ "$response" =~ ^[Ss]$ ]]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        source "$HOME/.cargo/env"
        print_success "Rust instalado correctamente"
    else
        print_error "Rust es necesario para compilar. Abortando."
        exit 1
    fi
}

build_binary() {
    print_header
    echo "Compilando Docker Manager v3.0..."
    echo
    
    cd "$SOURCE_DIR"
    
    if [ ! -f "Cargo.toml" ]; then
        print_error "No se encontró Cargo.toml en el directorio actual"
        exit 1
    fi
    
    # Compile in release mode
    if cargo build --release; then
        print_success "Compilación exitosa"
    else
        print_error "Error durante la compilación"
        exit 1
    fi
    
    if [ ! -f "$BINARY_PATH" ]; then
        print_error "No se encontró el binario compilado en $BINARY_PATH"
        exit 1
    fi
}

install_binary() {
    print_header
    echo "Instalando Docker Manager v3.0..."
    echo
    
    # Check if we need sudo
    if [ -w "$INSTALL_DIR" ]; then
        SUDO=""
    else
        SUDO="sudo"
        print_warning "Se requieren permisos de administrador para instalar en $INSTALL_DIR"
    fi
    
    # Copy binary
    if $SUDO cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"; then
        print_success "Binario copiado a $INSTALL_DIR/$BINARY_NAME"
    else
        print_error "Error al copiar el binario"
        exit 1
    fi
    
    # Make executable
    if $SUDO chmod +x "$INSTALL_DIR/$BINARY_NAME"; then
        print_success "Permisos de ejecución establecidos"
    else
        print_error "Error al establecer permisos"
        exit 1
    fi
    
    # Verify installation
    if command -v $BINARY_NAME &> /dev/null; then
        print_success "Docker Manager v3.0 instalado correctamente"
    else
        print_warning "Instalación completa pero $BINARY_NAME no está en PATH"
        echo "  Añade $INSTALL_DIR a tu PATH si es necesario"
    fi
}

show_usage() {
    echo
    print_header
    echo -e "${GREEN}¡Instalación completa!${NC}"
    echo
    echo "Uso:"
    echo "  ${BLUE}docker-manager${NC}              # Iniciar modo TUI interactivo"
    echo "  ${BLUE}docker-manager list${NC}        # Listar contenedores"
    echo "  ${BLUE}docker-manager logs <name>${NC} # Ver logs de un contenedor"
    echo "  ${BLUE}docker-manager --help${NC}      # Ver todas las opciones"
    echo
    echo "Atajos de teclado en modo TUI:"
    echo "  ${YELLOW}↑↓${NC} Navegar    ${YELLOW}L${NC} Logs    ${YELLOW}S${NC} Stats    ${YELLOW}F${NC} Fullscreen"
    echo "  ${YELLOW}D${NC} Docker Ops  ${YELLOW}C${NC} Clipboard  ${YELLOW}R${NC} Restart  ${YELLOW}Q${NC} Quit"
    echo
}

main() {
    print_header
    echo "Verificando requisitos del sistema..."
    echo
    
    # Check Docker
    check_docker
    
    # Check if binary exists
    if [ -f "$BINARY_PATH" ]; then
        print_success "Binario precompilado encontrado"
        echo
        print_warning "¿Deseas usar el binario existente o recompilar? (u/r)"
        echo "  u = Usar existente"
        echo "  r = Recompilar"
        read -r response
        if [[ "$response" =~ ^[Rr]$ ]]; then
            if check_rust; then
                build_binary
            else
                install_rust
                build_binary
            fi
        fi
    else
        print_warning "No se encontró binario precompilado"
        if check_rust; then
            build_binary
        else
            install_rust
            build_binary
        fi
    fi
    
    # Install
    install_binary
    
    # Show usage
    show_usage
}

# Parse arguments
case "${1:-}" in
    --help|-h)
        echo "Uso: $0 [opciones]"
        echo
        echo "Opciones:"
        echo "  --help, -h     Mostrar esta ayuda"
        echo "  --build-only   Solo compilar, no instalar"
        echo "  --install-only Solo instalar binario existente"
        echo
        exit 0
        ;;
    --build-only)
        if check_rust; then
            build_binary
        else
            install_rust
            build_binary
        fi
        print_success "Compilación completa. Binario en: $BINARY_PATH"
        exit 0
        ;;
    --install-only)
        if [ ! -f "$BINARY_PATH" ]; then
            print_error "No se encontró el binario en $BINARY_PATH"
            echo "Ejecuta primero: $0 --build-only"
            exit 1
        fi
        install_binary
        show_usage
        exit 0
        ;;
esac

# Run main installation
main