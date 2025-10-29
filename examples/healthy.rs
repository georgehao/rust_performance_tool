//! Example of healthy, well-behaved async tasks
//! 
//! This example shows properly written async tasks for comparison.
//! 
//! Run this with:
//! ```
//! cargo run --example healthy
//! ```
//! 
//! Then in another terminal:
//! ```
//! tokio-console
//! ```

use std::time::Duration;
use tokio::sync::mpsc;

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("Starting healthy tasks example...");
        println!("This demonstrates well-behaved async patterns.");
        println!("Connect with: tokio-console");
        println!("All tasks should show healthy metrics!");
        println!();

        let (tx, mut rx) = mpsc::channel(100);

        // Producer task - properly yields between work
        tokio::spawn(async move {
            let mut counter = 0;
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                counter += 1;
                
                if tx.send(counter).await.is_err() {
                    break;
                }
                println!("Producer: sent message {}", counter);
            }
        });

        // Consumer task - properly awaits messages
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                println!("Consumer: received message {}", msg);
                // Simulate some async work
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // CPU-intensive task done properly with spawn_blocking
        tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                
                println!("CPU task: starting computation...");
                let result = tokio::task::spawn_blocking(|| {
                    // Heavy CPU work in blocking thread pool
                    let mut sum = 0u64;
                    for i in 0..50_000_000 {
                        sum = sum.wrapping_add(i);
                    }
                    sum
                }).await.unwrap();
                
                println!("CPU task: result = {}", result);
            }
        });

        // Timer task - simple and efficient
        tokio::spawn(async {
            let mut tick = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                tick += 1;
                println!("Timer: tick {}", tick);
            }
        });

        // HTTP request simulation - proper async I/O pattern
        tokio::spawn(async {
            loop {
                println!("HTTP task: starting request...");
                // Simulate async I/O with proper yielding
                tokio::time::sleep(Duration::from_millis(800)).await;
                println!("HTTP task: request completed successfully");
                
                // Wait before next request
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });

        // Graceful shutdown pattern
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        println!("Graceful task: working...");
                    }
                    _ = shutdown_rx.recv() => {
                        println!("Graceful task: shutting down...");
                        break;
                    }
                }
            }
        });

        // Keep the program running
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            println!("All systems healthy! Check tokio-console for metrics.");
        }
    });
}

