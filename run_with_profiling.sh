#!/bin/bash
# Convenient script to run pprof_http example with proper heap profiling configuration

echo "========================================="
echo "Starting pprof HTTP server with heap profiling"
echo "========================================="
echo ""
echo "Configuration:"
echo "  ✓ Jemalloc profiling enabled (via jemalloc_pprof)"
echo "  ✓ Frame pointers enabled (via .cargo/config.toml)"
echo ""
echo "Server will start at http://localhost:8080"
echo ""
echo "To get heap profile with proper stack traces:"
echo "  1. Allocate memory: curl -X POST 'http://localhost:8080/allocate?mb=100'"
echo "  2. Get profile: curl -X POST http://localhost:8080/profile/memory > heap_profile.pb"
echo "  3. Analyze: go tool pprof -http=:9001 heap_profile.pb"
echo ""
echo "========================================="
echo ""

# Clean and rebuild to ensure frame pointers are applied
echo "Cleaning previous build..."
cargo clean -p tokio-console-demo --release 2>/dev/null || true

echo "Building with frame pointers (from .cargo/config.toml)..."
cargo build --example pprof_http --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo ""
echo "Starting server..."
echo ""

# Run the server
# Jemalloc profiling is activated in the code via jemalloc_pprof::PROF_CTL
exec ./target/release/examples/pprof_http
