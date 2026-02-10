#!/usr/bin/env bash
set -euo pipefail

# Logium startup script
# Builds and runs both the backend server and frontend dev server.
# Usage:
#   ./run.sh          - Build and run everything (dev mode)
#   ./run.sh build    - Build for production only (no dev servers)
#   ./run.sh prod     - Build for production and serve from backend

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

ROOT="$(cd "$(dirname "$0")" && pwd)"

log() { echo -e "${CYAN}[logium]${NC} $1"; }
ok()  { echo -e "${GREEN}[logium]${NC} $1"; }
err() { echo -e "${RED}[logium]${NC} $1" >&2; }

cleanup() {
    log "Shutting down..."
    [[ -n "${BACKEND_PID:-}" ]] && kill "$BACKEND_PID" 2>/dev/null || true
    [[ -n "${FRONTEND_PID:-}" ]] && kill "$FRONTEND_PID" 2>/dev/null || true
    wait 2>/dev/null
}
trap cleanup EXIT

# Check prerequisites
check_prereqs() {
    if ! command -v cargo &>/dev/null; then
        err "cargo not found. Install Rust: https://rustup.rs"
        exit 1
    fi
    if ! command -v node &>/dev/null; then
        err "node not found. Install Node.js: https://nodejs.org"
        exit 1
    fi
    if ! command -v npm &>/dev/null; then
        err "npm not found. Install Node.js: https://nodejs.org"
        exit 1
    fi
}

# Install frontend dependencies if needed
install_frontend_deps() {
    if [ ! -d "$ROOT/ui/node_modules" ]; then
        log "Installing frontend dependencies..."
        (cd "$ROOT/ui" && npm install)
    fi
}

# Build the Rust backend
build_backend() {
    log "Building backend..."
    cargo build -p logium-server --release 2>&1
    ok "Backend built."
}

# Build the frontend for production
build_frontend() {
    log "Building frontend..."
    (cd "$ROOT/ui" && npm run build)
    ok "Frontend built to ui/dist/"
}

# Run tests
run_tests() {
    log "Running tests..."
    cargo test --workspace 2>&1
    ok "All tests passed."
}

case "${1:-dev}" in
    build)
        check_prereqs
        install_frontend_deps
        build_backend
        build_frontend
        run_tests
        ok "Build complete. Run with: ./run.sh prod"
        ;;

    prod)
        check_prereqs
        install_frontend_deps
        build_backend
        build_frontend
        ok "Starting Logium in production mode..."
        log "Server: http://localhost:3000"
        echo ""
        (cd "$ROOT/crates/logium-server" && "$ROOT/target/release/logium-server")
        ;;

    dev|"")
        check_prereqs
        install_frontend_deps

        log "Building backend (debug)..."
        cargo build -p logium-server 2>&1
        ok "Backend built."

        log "Starting backend on http://localhost:3000..."
        (cd "$ROOT/crates/logium-server" && exec "$ROOT/target/debug/logium-server") &
        BACKEND_PID=$!

        # Wait for backend to start
        sleep 1
        if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
            err "Backend failed to start."
            exit 1
        fi
        ok "Backend running (PID $BACKEND_PID)"

        log "Starting frontend dev server on http://localhost:5173..."
        (cd "$ROOT/ui" && npm run dev -- --open) &
        FRONTEND_PID=$!
        ok "Frontend dev server running (PID $FRONTEND_PID)"

        echo ""
        ok "Logium is running!"
        log "Frontend (dev):  http://localhost:5173"
        log "Backend API:     http://localhost:3000/api"
        log "Press Ctrl+C to stop."
        echo ""

        wait
        ;;

    *)
        echo "Usage: $0 [dev|build|prod]"
        exit 1
        ;;
esac
