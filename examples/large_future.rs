//! Example demonstrating "large-future" warnings
//!
//! This example shows futures that occupy a large amount of stack space,
//! which can lead to performance issues and potential stack overflows.
//!
//! Large futures occur when:
//! - Too many large variables are held across await points
//! - Deeply nested async operations
//! - Large arrays or buffers in async functions
//!
//! Run this with:
//! ```
//! cargo run --example large_future
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```
//!
//! In tokio-console, look for:
//! - "large-future" warnings
//! - Future size information in task details

use std::time::Duration;

// Large struct that will be held across await points
struct LargeData {
    buffer1: [u8; 10_000], // 10 KB
    buffer2: [u8; 10_000], // 10 KB
    buffer3: [u8; 10_000], // 10 KB
    buffer4: [u8; 10_000], // 10 KB
                           // Total: 40 KB per instance
}

impl LargeData {
    fn new() -> Self {
        Self {
            buffer1: [0u8; 10_000],
            buffer2: [1u8; 10_000],
            buffer3: [2u8; 10_000],
            buffer4: [3u8; 10_000],
        }
    }

    fn process(&self) -> usize {
        self.buffer1.len() + self.buffer2.len() + self.buffer3.len() + self.buffer4.len()
    }
}

// ❌ BAD: Large future - holding large data across await points
async fn bad_large_future_task() {
    println!("[BAD] Task with large future started");

    loop {
        // ❌ Creating large data on the stack
        let large_data1 = LargeData::new(); // 40 KB
        let large_data2 = LargeData::new(); // 40 KB
        let large_data3 = LargeData::new(); // 40 KB
                                            // Total: 120 KB on stack!

        // ❌ These are held across await points
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = large_data1.process() + large_data2.process() + large_data3.process();

        // ❌ Still holding the data
        tokio::time::sleep(Duration::from_millis(100)).await;

        println!("[BAD] Processed {} bytes", result);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ✅ GOOD: Small future - using Box to move data to heap
async fn good_boxed_data_task() {
    println!("[GOOD] Task with boxed data started");

    loop {
        // ✅ Box moves data to heap, keeping stack small
        let large_data1 = Box::new(LargeData::new());
        let large_data2 = Box::new(LargeData::new());
        let large_data3 = Box::new(LargeData::new());

        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = large_data1.process() + large_data2.process() + large_data3.process();

        tokio::time::sleep(Duration::from_millis(100)).await;

        println!("[GOOD] Processed {} bytes", result);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ❌ BAD: Deeply nested futures with large state
async fn bad_deeply_nested() {
    println!("[BAD] Deeply nested task started");

    let data1 = LargeData::new();

    async {
        let data2 = LargeData::new();

        async {
            let data3 = LargeData::new();

            tokio::time::sleep(Duration::from_millis(100)).await;

            println!(
                "[BAD] Nested processing: {}",
                data1.process() + data2.process() + data3.process()
            );
        }
        .await;
    }
    .await;
}

// ✅ GOOD: Flatten the async operations
async fn good_flattened() {
    println!("[GOOD] Flattened task started");

    loop {
        // Process in stages, dropping data between stages
        let result1 = {
            let data = Box::new(LargeData::new());
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.process()
        }; // data dropped here

        let result2 = {
            let data = Box::new(LargeData::new());
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.process()
        }; // data dropped here

        println!("[GOOD] Flattened processing: {}", result1 + result2);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ❌ BAD: Holding large buffers across many await points
async fn bad_many_buffers() {
    println!("[BAD] Task with many buffers started");

    loop {
        let buffer1 = vec![0u8; 50_000]; // 50 KB
        let buffer2 = vec![0u8; 50_000]; // 50 KB

        // Many await points while holding large buffers
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = buffer1.len();

        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = buffer2.len();

        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("[BAD] Buffers: {} + {}", buffer1.len(), buffer2.len());

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ✅ GOOD: Process and drop buffers promptly
async fn good_prompt_drop() {
    println!("[GOOD] Task with prompt drops started");

    loop {
        // Process buffer1 and drop immediately
        let len1 = {
            let buffer1 = vec![0u8; 50_000];
            tokio::time::sleep(Duration::from_millis(10)).await;
            buffer1.len()
        }; // buffer1 dropped here

        // Process buffer2 and drop immediately
        let len2 = {
            let buffer2 = vec![0u8; 50_000];
            tokio::time::sleep(Duration::from_millis(10)).await;
            buffer2.len()
        }; // buffer2 dropped here

        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("[GOOD] Buffers: {} + {}", len1, len2);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("=== Large Future Examples ===");
        println!("This demonstrates futures that occupy large stack space.");
        println!("Connect with: tokio-console");
        println!("Look for 'large-future' warnings!\n");

        // Scenario 1: Large data on stack
        // println!("[Scenario 1] Large data held across await points (BAD)");
        // tokio::spawn(bad_large_future_task());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // println!("\n[Scenario 2] Boxed data on heap (GOOD)");
        // tokio::spawn(good_boxed_data_task());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // // Scenario 2: Deeply nested
        // println!("\n[Scenario 3] Deeply nested futures (BAD)");
        // for _ in 0..5 {
        //     tokio::spawn(bad_deeply_nested());
        //     tokio::time::sleep(Duration::from_millis(200)).await;
        // }

        // println!("\n[Scenario 4] Flattened operations (GOOD)");
        // tokio::spawn(good_flattened());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // Scenario 3: Many buffers
        println!("\n[Scenario 5] Holding many buffers (BAD)");
        tokio::spawn(bad_many_buffers());
        tokio::time::sleep(Duration::from_millis(500)).await;

        // println!("\n[Scenario 6] Prompt buffer drops (GOOD)");
        // tokio::spawn(good_prompt_drop());
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // Normal tasks for comparison
        for i in 0..2 {
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    println!("[Normal] Task {} running", i);
                }
            });
        }

        // Status monitoring
        let mut tick = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            tick += 1;

            println!("\n╔══════════════════════════════════╗");
            println!("║  Status Update #{}              ║", tick);
            println!("╠══════════════════════════════════╣");
            println!("║ Check tokio-console for:         ║");
            println!("║ • Large-future warnings          ║");
            println!("║ • Future size in task details    ║");
            println!("║ • Stack usage comparison         ║");
            println!("╚══════════════════════════════════╝\n");
        }
    });
}
