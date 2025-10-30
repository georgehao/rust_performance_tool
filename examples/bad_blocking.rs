//! Example demonstrating VERY BAD async patterns
//!
//! This example shows what happens when you do CPU-intensive work
//! directly in async functions WITHOUT yielding, causing the executor
//! to be blocked and other tasks to starve.
//!
//! Run this with:
//! ```
//! cargo run --example bad_blocking
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```
//!
//! You should see:
//! - Very high Busy percentage (50%+)
//! - Very long Poll times (seconds!)
//! - "Never yielded" warnings
//! - Other tasks being starved

use std::time::Duration;

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("=== Bad Blocking Examples ===");
        println!("This demonstrates EXTREMELY BAD async patterns!");
        println!("Watch tokio-console for:");
        println!("- High Busy percentage");
        println!("- Long Poll times");
        println!("- Never yielded warnings");
        println!("- Task starvation\n");

        // ❌ BAD: Long synchronous operations
        tokio::spawn(async {
            println!("[Bad Task 3] Mixed blocking patterns...");

            let mut count = 0;
            loop {
                // Some async work
                tokio::time::sleep(Duration::from_millis(100)).await;

                println!("[Bad Task 3] Starting mixed blocking operations...");

                // ❌ BAD: Mix of blocking operations
                for i in 0..5 {
                    // Blocking CPU work
                    let mut sum = 0u64;
                    for j in 0..200_000_000 {
                        sum = sum.wrapping_add(j);
                    }

                    // Blocking I/O
                    std::thread::sleep(Duration::from_millis(500));

                    println!("[Bad Task 3] Iteration {} done", i);
                    // Notice: NO await points during the work!
                }

                println!("[Bad Task 3] All blocking work done");
                count += 1;
                println!("[Bad Task 3] Count: {}", count);
            }
        });

        // // ✅ GOOD: Using spawn_blocking for CPU-intensive work
        // tokio::spawn(async {
        //     println!("[Good Task] Proper handling of blocking operations...");

        //     let mut count = 0;
        //     loop {
        //         // Some async work
        //         tokio::time::sleep(Duration::from_millis(100)).await;

        //         println!("[Good Task] Starting blocking operations...");

        //         // ✅ GOOD: Move blocking work to dedicated thread pool
        //         tokio::task::spawn_blocking(|| {
        //             for i in 0..5 {
        //                 // Blocking CPU work
        //                 let mut sum = 0u64;
        //                 for j in 0..200_000_000 {
        //                     sum = sum.wrapping_add(j);
        //                 }

        //                 // Blocking I/O
        //                 std::thread::sleep(Duration::from_millis(500));

        //                 println!("[Good Task] Iteration {} done", i);
        //             }

        //             println!("[Good Task] All blocking work done");
        //         })
        //         .await
        //         .unwrap();

        //         // Update count AFTER blocking work completes
        //         count += 1;
        //         println!("[Good Task] Count: {}", count);
        //     }
        // });

        // Monitoring task
        tokio::spawn(async {
            let mut report_count = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                report_count += 1;

                println!("\n╔════════════════════════════════════════╗");
                println!("║  Status Report #{}                    ║", report_count);
                println!("╠════════════════════════════════════════╣");
                println!("║  Check tokio-console for:              ║");
                println!("║  • Busy % > 50% (should be < 1%)      ║");
                println!("║  • Poll times in SECONDS (should be µs)║");
                println!("║  • Never yielded warnings             ║");
                println!("║  • Good tasks being starved           ║");
                println!("╚════════════════════════════════════════╝\n");
            }
        });

        // Keep running
        println!("\n🔥 Running BAD async code...");
        println!("💡 Open tokio-console to see the disaster!\n");

        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
