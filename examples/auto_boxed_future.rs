//! Example demonstrating "auto-boxed-future" warnings
//!
//! This example shows futures that were automatically boxed by the Tokio runtime
//! because they exceeded the size threshold. Auto-boxing has performance overhead.
//!
//! Auto-boxing occurs when:
//! - The Future is larger than Tokio's threshold (typically 2KB)
//! - Tokio automatically boxes it to prevent stack overflow
//! - This adds heap allocation overhead
//!
//! Run this with:
//! ```
//! cargo run --example auto_boxed_future
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```
//!
//! In tokio-console, look for:
//! - "auto-boxed-future" warnings
//! - Task details showing the future was auto-boxed

use std::time::Duration;

// Very large struct that causes auto-boxing when used in spawned tasks
#[derive(Clone)]
#[allow(dead_code)] // Some fields intentionally unused for size demonstration
struct VeryLargeStruct {
    data1: [u8; 5000], // 5 KB
    data2: [u8; 5000], // 5 KB
    data3: [u8; 5000], // 5 KB
    data4: [u8; 5000], // 5 KB
                       // Total: 20 KB - definitely exceeds Tokio's threshold!
}

impl VeryLargeStruct {
    fn new() -> Self {
        Self {
            data1: [0; 5000],
            data2: [1; 5000],
            data3: [2; 5000],
            data4: [3; 5000],
        }
    }

    fn compute(&self) -> usize {
        self.data1.iter().map(|&x| x as usize).sum::<usize>()
            + self.data2.iter().map(|&x| x as usize).sum::<usize>()
    }
}

// ❌ BAD: Spawning with large state causes auto-boxing
async fn bad_auto_boxed_task() {
    println!("[BAD] Auto-boxed task started");

    // Large data captured by the async block
    let large_struct = VeryLargeStruct::new();

    loop {
        // Using the large struct across await points
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = large_struct.compute();

        tokio::time::sleep(Duration::from_millis(100)).await;

        println!("[BAD] Computed: {}", result);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ❌ BAD: Spawning tasks with large closures
fn spawn_with_large_closure() {
    println!("[BAD] Spawning with large closure");

    // This closure captures a lot of large data
    let data1 = VeryLargeStruct::new();
    let data2 = VeryLargeStruct::new();

    // ❌ tokio::spawn will auto-box this because it's too large
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = data1.compute() + data2.compute();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

// ✅ GOOD: Use Box to explicitly control boxing
async fn good_explicit_box_task() {
    println!("[GOOD] Explicitly boxed task started");

    // Explicitly box large data
    let large_struct = Box::new(VeryLargeStruct::new());

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = large_struct.compute();

        tokio::time::sleep(Duration::from_millis(100)).await;

        println!("[GOOD] Computed: {}", result);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ✅ GOOD: Pass by reference or use Arc
async fn good_shared_data_task() {
    use std::sync::Arc;

    println!("[GOOD] Shared data task started");

    // Use Arc to share data without copying
    let large_struct = Arc::new(VeryLargeStruct::new());

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = large_struct.compute();

        tokio::time::sleep(Duration::from_millis(100)).await;

        println!("[GOOD] Computed: {}", result);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ❌ BAD: Complex nested async with large state
async fn bad_complex_nested() {
    println!("[BAD] Complex nested with large state");

    let state1 = VeryLargeStruct::new();
    let state2 = VeryLargeStruct::new();

    loop {
        // Nested async operations holding large state
        let result = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            state1.compute()
        }
        .await;

        let result2 = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            state2.compute()
        }
        .await;

        println!("[BAD] Nested results: {} + {}", result, result2);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ✅ GOOD: Minimize state size
async fn good_minimal_state() {
    println!("[GOOD] Minimal state task");

    loop {
        // Only keep what's needed
        let result1 = {
            let data = Box::new(VeryLargeStruct::new());
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.compute()
        }; // data dropped

        let result2 = {
            let data = Box::new(VeryLargeStruct::new());
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.compute()
        }; // data dropped

        println!("[GOOD] Results: {} + {}", result1, result2);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// Demonstrate size comparison
mod size_demo {
    use super::*;

    pub fn show_sizes() {
        println!("\n=== Future Size Information ===");
        println!(
            "VeryLargeStruct size: {} bytes",
            std::mem::size_of::<VeryLargeStruct>()
        );
        println!(
            "Box<VeryLargeStruct> size: {} bytes",
            std::mem::size_of::<Box<VeryLargeStruct>>()
        );
        println!("Tokio auto-box threshold: ~2048 bytes");
        println!(
            "Our struct: {} KB",
            std::mem::size_of::<VeryLargeStruct>() / 1024
        );
        println!("===============================\n");
    }
}

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("=== Auto-Boxed Future Examples ===");
        println!("This demonstrates futures that get auto-boxed by Tokio.");
        println!("Connect with: tokio-console");
        println!("Look for 'auto-boxed-future' warnings!\n");

        size_demo::show_sizes();

        // Scenario 1: Auto-boxed due to large state
        println!("[Scenario 1] Large state causing auto-boxing (BAD)");
        tokio::spawn(bad_auto_boxed_task());
        tokio::time::sleep(Duration::from_millis(500)).await;

        // println!("\n[Scenario 2] Explicitly boxed data (GOOD)");
        // tokio::spawn(good_explicit_box_task());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // // Scenario 2: Large closures
        // println!("\n[Scenario 3] Large closures causing auto-boxing (BAD)");
        // for _ in 0..3 {
        //     spawn_with_large_closure();
        //     tokio::time::sleep(Duration::from_millis(200)).await;
        // }

        // println!("\n[Scenario 4] Shared data with Arc (GOOD)");
        // tokio::spawn(good_shared_data_task());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // // Scenario 3: Complex nested
        // println!("\n[Scenario 5] Complex nested async (BAD)");
        // tokio::spawn(bad_complex_nested());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // println!("\n[Scenario 6] Minimal state (GOOD)");
        // tokio::spawn(good_minimal_state());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // // Normal tasks
        // for i in 0..2 {
        //     tokio::spawn(async move {
        //         loop {
        //             tokio::time::sleep(Duration::from_secs(2)).await;
        //             println!("[Normal] Task {} (small future)", i);
        //         }
        //     });
        // }

        // Status monitoring
        let mut tick = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            tick += 1;

            println!("\n╔════════════════════════════════════════╗");
            println!("║  Status Update #{}                    ║", tick);
            println!("╠════════════════════════════════════════╣");
            println!("║ Check tokio-console for:               ║");
            println!("║ • auto-boxed-future warnings           ║");
            println!("║ • Tasks marked as auto-boxed           ║");
            println!("║ • Performance impact of auto-boxing    ║");
            println!("║                                        ║");
            println!("║ BAD tasks: Will show auto-box warnings ║");
            println!("║ GOOD tasks: Should not be auto-boxed   ║");
            println!("╚════════════════════════════════════════╝\n");
        }
    });
}
