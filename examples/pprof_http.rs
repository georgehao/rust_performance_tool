//! Example of using pprof-rs with HTTP endpoints for profiling
//!
//! This example demonstrates how to integrate pprof-rs with an HTTP server
//! to provide profiling endpoints that can be accessed via HTTP requests.
//!
//! Run this with:
//! ```
//! cargo run --example pprof_http --release
//! ```
//!
//! Available HTTP endpoints:
//! - GET  http://localhost:8080/                      - Status page
//! - GET  http://localhost:8080/work                  - Trigger CPU-intensive work
//! - POST http://localhost:8080/allocate?mb=<n>       - Allocate persistent memory
//! - POST http://localhost:8080/profile/cpu           - Get CPU profile (protobuf format)
//! - POST http://localhost:8080/profile/memory        - Get heap memory profile (jemalloc, protobuf)
//!
//! Example usage:
//! ```bash
//! # CPU Profiling
//! curl -X POST http://localhost:8080/profile/cpu > cpu_profile.pb
//! curl -X POST "http://localhost:8080/profile/cpu?seconds=30" > cpu_profile.pb
//! go tool pprof -http=:9000 cpu_profile.pb
//!
//! # Memory Profiling (requires jemalloc profiling enabled)
//! # First, allocate some persistent memory
//! curl -X POST "http://localhost:8080/allocate?mb=100"
//!
//! # Then get a heap profile
//! curl -X POST http://localhost:8080/profile/memory > heap_profile.pb
//! go tool pprof -http=:9001 heap_profile.pb
//!
//! # To enable jemalloc profiling with proper stack traces, run with:
//! _RJEM_MALLOC_CONF=prof:true,lg_prof_sample:0,prof_final:false \
//! RUSTFLAGS="-C force-frame-pointers=yes" \
//! cargo run --example pprof_http --release
//!
//! Note: Memory profiling uses jemalloc (not available on MSVC/Windows)
//! ```

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder;
use pprof::protos::Message;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// Use jemalloc as the global allocator (not on MSVC/Windows)
#[cfg(all(not(target_env = "msvc"), not(target_os = "windows")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Shared application state
struct AppState {
    request_count: Arc<Mutex<u64>>,
    // Persistent memory allocations for demonstration
    memory_pool: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            request_count: Arc::new(Mutex::new(0)),
            memory_pool: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Activate jemalloc profiling at startup
    #[cfg(all(not(target_env = "msvc"), not(target_os = "windows")))]
    {
        if let Some(prof_ctl) = jemalloc_pprof::PROF_CTL.as_ref() {
            let mut guard = prof_ctl.lock().await;
            match guard.activate() {
                Ok(_) => println!("✅ Jemalloc profiling activated successfully"),
                Err(e) => eprintln!("⚠️  Warning: Failed to activate jemalloc profiling: {}", e),
            }
        } else {
            eprintln!("⚠️  Warning: Jemalloc profiling controller not available");
            eprintln!("    Memory profiling (/profile/memory) will not work properly.");
            eprintln!("    To enable with proper stack traces, restart with:");
            eprintln!("    _RJEM_MALLOC_CONF=prof:true,lg_prof_sample:0,prof_final:false \\");
            eprintln!("    RUSTFLAGS=\"-C force-frame-pointers=yes\" \\");
            eprintln!("    cargo run --example pprof_http --release");
            eprintln!();
            eprintln!("    Explanation:");
            eprintln!("    - prof:true          : Enable jemalloc profiling");
            eprintln!("    - lg_prof_sample:0   : Sample every allocation (0 = no sampling)");
            eprintln!("    - prof_final:false   : Don't dump profile on exit");
            eprintln!("    - force-frame-pointers: Required for accurate stack unwinding");
        }
    }

    println!("Starting pprof HTTP server example...");
    println!("Server will listen on http://localhost:8080");
    println!();
    println!("Available endpoints:");
    println!("  GET  /                                         - Status page");
    println!("  GET  /work                                     - Trigger CPU work");
    println!("  POST /allocate?mb=<n>                          - Allocate persistent memory");
    println!("  POST /profile/cpu?seconds=<n>                  - Get CPU profile (protobuf)");
    println!("  POST /profile/memory                           - Get heap profile (jemalloc)");
    println!();

    let state = Arc::new(AppState::new());
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = Arc::clone(&state);
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = Arc::clone(&state);
                handle_request(req, state)
            });

            if let Err(err) = Builder::new(hyper_util::rt::TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                eprintln!("Error serving connection from {}: {:?}", addr, err);
            }
        });
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    req: Request<Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path();
    let query = req.uri().query();

    // Increment request counter
    {
        let mut count = state.request_count.lock().await;
        *count += 1;
        println!("Request #{}: {} {}", *count, req.method(), req.uri());
    }

    match (req.method(), path) {
        (&hyper::Method::GET, "/") => Ok(handle_status(state).await),
        (&hyper::Method::GET, "/work") => Ok(handle_work().await),
        (&hyper::Method::POST, "/allocate") => Ok(handle_allocate(state, query).await),
        (&hyper::Method::POST, "/profile/cpu") => Ok(handle_cpu_profile(query).await),
        (&hyper::Method::POST, "/profile/memory") => Ok(handle_memory_profile(state).await),
        _ => Ok(not_found()),
    }
}

/// Status endpoint - shows service information
async fn handle_status(state: Arc<AppState>) -> Response<Full<Bytes>> {
    let count = state.request_count.lock().await;
    let pool = state.memory_pool.lock().await;
    let total_mb = pool.iter().map(|v| v.len()).sum::<usize>() as f64 / 1024.0 / 1024.0;

    let body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>pprof HTTP Server</title>
    <style>
        body {{ font-family: Arial, sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }}
        h1 {{ color: #333; }}
        .endpoint {{ background: #f4f4f4; padding: 10px; margin: 10px 0; border-radius: 5px; }}
        code {{ background: #eee; padding: 2px 6px; border-radius: 3px; }}
        .stats {{ background: #e8f5e9; padding: 10px; margin: 10px 0; border-radius: 5px; }}
    </style>
</head>
<body>
    <h1>🔥 pprof HTTP Server</h1>
    <p><strong>Status:</strong> Running</p>
    <p><strong>Requests Handled:</strong> {}</p>

    <div class="stats">
        <strong>Memory Stats:</strong><br>
        Allocated memory pools: {}<br>
        Total allocated: {:.2} MB
    </div>

    <h2>Available Endpoints</h2>

    <div class="endpoint">
        <strong>GET /work</strong><br>
        Trigger CPU-intensive work (for testing profiling)
    </div>

    <div class="endpoint">
        <strong>POST /allocate?mb=&lt;n&gt;</strong><br>
        Allocate persistent memory (for heap profiling demo)<br>
        Example: <code>curl -X POST "http://localhost:8080/allocate?mb=50"</code>
    </div>

    <div class="endpoint">
        <strong>POST /profile/cpu?seconds=&lt;n&gt;</strong><br>
        Get CPU profile in protobuf format<br>
        Example: <code>curl -X POST "http://localhost:8080/profile/cpu?seconds=10" &gt; cpu_profile.pb</code>
    </div>

    <div class="endpoint">
        <strong>POST /profile/memory</strong><br>
        Get heap memory profile using jemalloc<br>
        <em>Shows memory allocations (not CPU usage)</em><br>
        Example: <code>curl -X POST http://localhost:8080/profile/memory &gt; heap_profile.pb</code>
    </div>

    <h2>Quick Start - CPU Profiling</h2>
    <ol>
        <li>Start some background work: <code>curl http://localhost:8080/work</code></li>
        <li>Get a CPU profile: <code>curl -X POST "http://localhost:8080/profile/cpu?seconds=5" &gt; cpu_profile.pb</code></li>
        <li>Analyze with pprof: <code>go tool pprof -http=:9000 cpu_profile.pb</code></li>
    </ol>

    <h2>Quick Start - Memory Profiling</h2>
    <ol>
        <li>Allocate some memory: <code>curl -X POST "http://localhost:8080/allocate?mb=100"</code></li>
        <li>Get a heap profile: <code>curl -X POST http://localhost:8080/profile/memory &gt; heap_profile.pb</code></li>
        <li>Analyze with pprof: <code>go tool pprof -http=:9001 heap_profile.pb</code></li>
    </ol>
</body>
</html>"#,
        *count, pool.len(), total_mb
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

/// Work endpoint - triggers CPU-intensive work
async fn handle_work() -> Response<Full<Bytes>> {
    println!("Starting CPU-intensive work...");

    // Spawn multiple tasks doing CPU work
    let handles: Vec<_> = (0..4)
        .map(|i| {
            tokio::spawn(async move {
                // Mix of different workload patterns
                match i % 3 {
                    0 => {
                        let _ = fibonacci_work(35);
                    }
                    1 => {
                        let _ = prime_number_work(100000);
                    }
                    _ => {
                        let _ = hash_work(1000000);
                    }
                }
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    println!("CPU work completed");

    let body =
        "CPU-intensive work completed! Try profiling with: curl -X POST \"http://localhost:8080/profile/cpu?seconds=10\" > cpu_profile.pb\n";
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

/// Allocate endpoint - allocates persistent memory for heap profiling demos
async fn handle_allocate(state: Arc<AppState>, query: Option<&str>) -> Response<Full<Bytes>> {
    let mb = parse_mb_param(query).unwrap_or(10);

    println!("Allocating {} MB of memory...", mb);

    let mut pool = state.memory_pool.lock().await;

    // Allocate memory in chunks with different patterns for better profiling visibility
    let bytes_per_mb = 1024 * 1024;
    let total_bytes = mb * bytes_per_mb;

    // Create multiple allocation patterns
    // Pattern 1: Large contiguous block
    pool.push(vec![0u8; (total_bytes / 2) as usize]);

    // Pattern 2: Many smaller blocks
    for i in 0..(total_bytes / 4 / 1024) {
        pool.push(vec![(i % 256) as u8; 1024]);
    }

    // Pattern 3: Variable-sized blocks
    for i in 0..100 {
        let size = ((i + 1) * (total_bytes / 400)) as usize;
        pool.push(vec![0xAA; size]);
    }

    let total_allocated_mb = pool.iter().map(|v| v.len()).sum::<usize>() as f64 / 1024.0 / 1024.0;

    println!("Memory allocation completed. Total: {:.2} MB across {} pools",
             total_allocated_mb, pool.len());

    let body = format!(
        "Allocated {} MB successfully!\nTotal allocated: {:.2} MB across {} memory pools\n\nTry getting a heap profile:\ncurl -X POST http://localhost:8080/profile/memory > heap_profile.pb\ngo tool pprof -http=:9001 heap_profile.pb\n",
        mb, total_allocated_mb, pool.len()
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

/// CPU profile endpoint - returns profile in protobuf format
async fn handle_cpu_profile(query: Option<&str>) -> Response<Full<Bytes>> {
    let seconds = parse_seconds_param(query).unwrap_or(10);

    println!("Starting CPU profiling ({} seconds)...", seconds);
    println!("Generating background CPU load during profiling...");

    // Start profiling with lower frequency (100 Hz is more reliable)
    let guard = match pprof::ProfilerGuard::new(100) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to start profiler: {}", e);
            return error_response(format!("Failed to start profiler: {}", e));
        }
    };

    // Spawn background tasks to generate CPU load during profiling
    let mut handles = Vec::new();
    for i in 0..4 {
        let handle = tokio::spawn(async move {
            let iterations = if i % 2 == 0 { 100000 } else { 50000 };
            for _ in 0..iterations {
                // Mix of different workload patterns
                match i % 3 {
                    0 => {
                        let _ = fibonacci_work(30);
                    }
                    1 => {
                        let _ = prime_number_work(10000);
                    }
                    _ => {
                        let _ = hash_work(50000);
                    }
                }
                // Small yield to let profiler sample
                tokio::task::yield_now().await;
            }
        });
        handles.push(handle);
    }

    // Wait for the specified profiling duration or until work completes
    let work_future = async {
        for handle in handles {
            let _ = handle.await;
        }
    };

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {
            println!("Profiling duration completed");
        }
        _ = work_future => {
            println!("Background work completed");
        }
    }

    // Generate protobuf profile
    match guard.report().build() {
        Ok(report) => {
            match report.pprof() {
                Ok(profile) => {
                    // Convert profile to bytes using write_to_writer
                    let mut body = Vec::new();
                    if let Err(e) = profile.write_to_writer(&mut body) {
                        eprintln!("Failed to encode profile: {}", e);
                        return error_response(format!("Failed to encode profile: {}", e));
                    }

                    if body.is_empty() {
                        eprintln!("Warning: Generated profile is empty");
                        return error_response("Generated profile is empty. This might be due to system limitations or insufficient CPU activity.".to_string());
                    }

                    println!("CPU profile generated successfully ({} bytes)", body.len());

                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/octet-stream")
                        .header(
                            "Content-Disposition",
                            "attachment; filename=\"cpu_profile.pb\"",
                        )
                        .body(Full::new(Bytes::from(body)))
                        .unwrap()
                }
                Err(e) => {
                    eprintln!("Failed to generate pprof: {}", e);
                    error_response(format!("Failed to generate pprof: {}", e))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to build report: {}", e);
            error_response(format!("Failed to build report: {}", e))
        }
    }
}

/// Memory profile endpoint - uses jemalloc heap profiling
///
/// This endpoint generates a true heap memory profile using jemalloc's profiling capabilities.
/// It shows memory allocations, not CPU usage.
#[cfg(all(not(target_env = "msvc"), not(target_os = "windows")))]
async fn handle_memory_profile(state: Arc<AppState>) -> Response<Full<Bytes>> {
    println!("Generating heap memory profile using jemalloc...");

    // Check if profiling is activated
    let prof_ctl = jemalloc_pprof::PROF_CTL.as_ref();
    if prof_ctl.is_none() {
        return error_response(
            "Profiling controller not available. Ensure jemalloc is properly configured.\n\
             Run with:\n\
             _RJEM_MALLOC_CONF=prof:true,lg_prof_sample:0,prof_final:false \\\n\
             RUSTFLAGS=\"-C force-frame-pointers=yes\" \\\n\
             cargo run --example pprof_http --release"
                .to_string(),
        );
    }

    let prof_ctl = prof_ctl.unwrap();
    let mut prof_ctl_guard = prof_ctl.lock().await;

    // Check if profiling is active
    if !prof_ctl_guard.activated() {
        eprintln!("⚠️  Jemalloc profiling is not active!");
        return error_response(
            "Jemalloc profiling is not active.\n\
             Restart the server with:\n\
             _RJEM_MALLOC_CONF=prof:true,lg_prof_sample:0,prof_final:false \\\n\
             RUSTFLAGS=\"-C force-frame-pointers=yes\" \\\n\
             cargo run --example pprof_http --release"
                .to_string(),
        );
    }

    println!("Jemalloc profiling is active, proceeding with profile dump...");

    // Get info about existing allocations
    let pool = state.memory_pool.lock().await;
    let existing_mb = pool.iter().map(|v| v.len()).sum::<usize>() as f64 / 1024.0 / 1024.0;
    let pool_count = pool.len();
    drop(pool);

    println!("Current state: {:.2} MB allocated across {} pools", existing_mb, pool_count);

    // Create temporary allocations to make the profile more interesting
    // These will show up with different stack traces than the persistent allocations
    let temp_allocations = create_demo_allocations().await;

    println!("Created temporary demo allocations for profiling");
    println!("Dumping heap profile...");

    // Dump the profile while keeping all allocations alive
    let result = prof_ctl_guard.dump_pprof();

    // Keep temporary allocations alive during dump
    let temp_size = temp_allocations.iter().map(|v| v.len()).sum::<usize>() as f64 / 1024.0 / 1024.0;
    println!("Temporary allocations: {:.2} MB", temp_size);

    drop(temp_allocations);

    match result {
        Ok(pprof_data) => {
            if pprof_data.is_empty() {
                eprintln!("Warning: Generated heap profile is empty");
                return error_response(
                    "Generated heap profile is empty.\n\
                     This might happen if no memory is allocated or jemalloc profiling is not working.\n\
                     Try allocating memory first: curl -X POST 'http://localhost:8080/allocate?mb=50'"
                        .to_string(),
                );
            }

            println!(
                "Heap profile generated successfully ({} bytes)",
                pprof_data.len()
            );
            println!("Profile includes:");
            println!("  - Persistent allocations: {:.2} MB across {} pools", existing_mb, pool_count);
            println!("  - Temporary demo allocations: {:.2} MB", temp_size);

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/octet-stream")
                .header(
                    "Content-Disposition",
                    "attachment; filename=\"heap_profile.pb\"",
                )
                .body(Full::new(Bytes::from(pprof_data)))
                .unwrap()
        }
        Err(e) => {
            eprintln!("Failed to dump heap profile: {}", e);
            error_response(format!("Failed to dump heap profile: {}", e))
        }
    }
}

/// Memory profile endpoint - Windows/MSVC fallback (jemalloc not available)
#[cfg(any(target_env = "msvc", target_os = "windows"))]
async fn handle_memory_profile() -> Response<Full<Bytes>> {
    error_response(
        "Heap profiling is not available on Windows/MSVC targets. \
         Use Linux/macOS or consider alternative tools like heaptrack or valgrind."
            .to_string(),
    )
}

/// Parse seconds parameter from query string
fn parse_seconds_param(query: Option<&str>) -> Option<u64> {
    query.and_then(|q| {
        q.split('&')
            .find(|param| param.starts_with("seconds="))
            .and_then(|param| param.strip_prefix("seconds="))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|&seconds| seconds > 0 && seconds <= 300) // Max 5 minutes
    })
}

/// Parse MB parameter from query string
fn parse_mb_param(query: Option<&str>) -> Option<u64> {
    query.and_then(|q| {
        q.split('&')
            .find(|param| param.starts_with("mb="))
            .and_then(|param| param.strip_prefix("mb="))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|&mb| mb > 0 && mb <= 1024) // Max 1GB
    })
}

/// 404 Not Found response
fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from("404 Not Found\n")))
        .unwrap()
}

/// Error response
fn error_response(message: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "text/plain")
        .body(Full::new(Bytes::from(format!("Error: {}\n", message))))
        .unwrap()
}

// ============================================================================
// Memory allocation helpers
// ============================================================================

/// Create demo allocations with different patterns for heap profiling
async fn create_demo_allocations() -> Vec<Vec<u8>> {
    let mut allocations = Vec::new();

    // Pattern 1: Large blocks from different call sites
    allocations.extend(allocate_large_blocks());

    // Pattern 2: Many small allocations
    allocations.extend(allocate_small_blocks());

    // Pattern 3: String allocations
    allocations.extend(allocate_strings());

    // Pattern 4: Async task allocations
    let mut handles = Vec::new();
    for i in 0..4 {
        let handle = tokio::spawn(async move {
            allocate_from_task(i)
        });
        handles.push(handle);
    }

    for handle in handles {
        if let Ok(allocs) = handle.await {
            allocations.extend(allocs);
        }
    }

    allocations
}

/// Allocate large blocks (separate function for distinct stack trace)
fn allocate_large_blocks() -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    for i in 0..10 {
        blocks.push(vec![0xAA; (i + 1) * 1024 * 1024]);
    }
    blocks
}

/// Allocate many small blocks (separate function for distinct stack trace)
fn allocate_small_blocks() -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    for i in 0..1000 {
        blocks.push(vec![0xBB; (i % 100 + 1) * 1024]);
    }
    blocks
}

/// Allocate string data (separate function for distinct stack trace)
fn allocate_strings() -> Vec<Vec<u8>> {
    let mut strings = Vec::new();
    for i in 0..500 {
        let s = format!(
            "String allocation {} - This is a demo string for heap profiling visualization",
            i
        );
        strings.push(s.into_bytes());
    }
    strings
}

/// Allocate from async task (separate function for distinct stack trace)
fn allocate_from_task(task_id: usize) -> Vec<Vec<u8>> {
    let mut allocations = Vec::new();
    for i in 0..100 {
        let size = (i + 1) * 10000 * (task_id + 1);
        allocations.push(vec![(task_id % 256) as u8; size]);
    }
    allocations
}

// ============================================================================
// CPU-intensive workload functions for testing profiling
// ============================================================================

/// Compute Fibonacci number (recursive, inefficient on purpose)
fn fibonacci_work(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci_work(n - 1) + fibonacci_work(n - 2),
    }
}

/// Find prime numbers up to n
fn prime_number_work(n: u64) -> Vec<u64> {
    let mut primes = Vec::new();
    for num in 2..=n {
        if is_prime(num) {
            primes.push(num);
        }
    }
    primes
}

/// Check if a number is prime
fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for i in 2..=(n as f64).sqrt() as u64 {
        if n % i == 0 {
            return false;
        }
    }
    true
}

/// Hash computation work
fn hash_work(iterations: u64) -> u64 {
    let mut hash = 0u64;
    for i in 0..iterations {
        hash = hash.wrapping_mul(31).wrapping_add(i);
        hash ^= hash >> 16;
        hash = hash.wrapping_mul(0x85ebca6b);
        hash ^= hash >> 13;
        hash = hash.wrapping_mul(0xc2b2ae35);
        hash ^= hash >> 16;
    }
    hash
}
