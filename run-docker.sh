#!/bin/bash

# Dockpit v3.0 Container Runner
# Easy way to run Dockpit in a container

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

IMAGE_NAME="corpy-ai/dockpit:3.3.0"
CONTAINER_NAME="dockpit"

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
    echo "║                   Dockpit Container Runner                ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Check if Docker is running
check_docker() {
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker first."
        exit 1
    fi
    print_success "Docker is running"
}

# Build the image
build_image() {
    print_info "Building Dockpit image..."
    
    if docker build -t "$IMAGE_NAME" .; then
        print_success "Image built successfully: $IMAGE_NAME"
    else
        print_error "Failed to build image"
        exit 1
    fi
}

# Run the container
run_container() {
    print_info "Starting Dockpit container..."
    
    # Stop and remove existing container if it exists
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        print_info "Stopping existing container..."
        docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
        docker rm "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi
    
    # Run new container
    docker run -it --rm \
        --name "$CONTAINER_NAME" \
        -v /var/run/docker.sock:/var/run/docker.sock:ro \
        -v /usr/bin/docker:/usr/bin/docker:ro \
        -e TERM=xterm-256color \
        "$IMAGE_NAME"
}

# Run with docker-compose
run_compose() {
    print_info "Starting Dockpit with docker-compose..."
    
    if [[ ! -f "docker-compose.yml" ]]; then
        print_error "docker-compose.yml not found"
        exit 1
    fi
    
    docker-compose up --build -d dockpit
    
    print_success "Container started in detached mode"
    print_info "To connect: docker exec -it dockpit dockpit"
    print_info "To stop: docker-compose down"
}

# Show usage
show_usage() {
    echo "Dockpit Container Runner"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  run       Build and run interactively (default)"
    echo "  build     Build the Docker image only"
    echo "  compose   Run using docker-compose"
    echo "  stop      Stop running container"
    echo "  logs      Show container logs"
    echo "  shell     Open shell in running container"
    echo "  clean     Remove image and containers"
    echo "  --help    Show this help"
    echo ""
    echo "Examples:"
    echo "  $0                  # Build and run interactively"
    echo "  $0 run              # Same as above"
    echo "  $0 build            # Build image only"
    echo "  $0 compose          # Run with docker-compose"
    echo ""
}

# Stop container
stop_container() {
    print_info "Stopping Dockpit container..."
    
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker stop "$CONTAINER_NAME"
        print_success "Container stopped"
    else
        print_warning "Container is not running"
    fi
}

# Show logs
show_logs() {
    print_info "Showing Dockpit logs..."
    
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker logs -f "$CONTAINER_NAME"
    else
        print_warning "Container does not exist"
    fi
}

# Open shell
open_shell() {
    print_info "Opening shell in Dockpit container..."
    
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker exec -it "$CONTAINER_NAME" /bin/sh
    else
        print_error "Container is not running"
        exit 1
    fi
}

# Clean up
clean_up() {
    print_info "Cleaning up Dockpit containers and images..."
    
    # Stop and remove container
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
        docker rm "$CONTAINER_NAME" >/dev/null 2>&1 || true
        print_success "Container removed"
    fi
    
    # Remove image
    if docker images --format '{{.Repository}}:{{.Tag}}' | grep -q "^${IMAGE_NAME}$"; then
        docker rmi "$IMAGE_NAME" >/dev/null 2>&1 || true
        print_success "Image removed"
    fi
    
    # Clean up dangling images
    docker image prune -f >/dev/null 2>&1 || true
    print_success "Cleanup completed"
}

# Main function
main() {
    print_header
    check_docker
    
    case "${1:-run}" in
        "run"|"")
            build_image
            run_container
            ;;
        "build")
            build_image
            ;;
        "compose")
            run_compose
            ;;
        "stop")
            stop_container
            ;;
        "logs")
            show_logs
            ;;
        "shell")
            open_shell
            ;;
        "clean")
            clean_up
            ;;
        "--help"|"-h"|"help")
            show_usage
            ;;
        *)
            print_error "Unknown command: $1"
            show_usage
            exit 1
            ;;
    esac
}

main "$@"