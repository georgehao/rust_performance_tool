//! Example with multiple types of issues mixed together
//!
//! This example demonstrates various async problems occurring simultaneously,
//! which is more realistic for debugging real-world applications.
//!
//! Run this with:
//! ```
//! cargo run --example mixed_issues
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Notify};

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("Starting mixed issues example...");
        println!("This demonstrates multiple problems occurring together.");
        println!("Connect with: tokio-console");
        println!("Try to identify and distinguish different issues!");
        println!();

        // // Issue 1: Self-waking task
        // let notify = Arc::new(Notify::new());
        // let notify_clone = notify.clone();
        // tokio::spawn(async move {
        //     loop {
        //         notify_clone.notified().await;
        //         tokio::time::sleep(Duration::from_millis(5)).await;
        //         notify_clone.notify_one(); // Bad: waking itself
        //     }
        // });
        // notify.notify_one();

        // Issue 2: Never yields (busy loop)
        println!("[Issue 2] Spawning never-yielding task...");
        tokio::spawn(async {
            let mut counter = 0u64;
            let mut iteration = 0u64;
            loop {
                for _ in 0..500_000 {
                    counter = counter.wrapping_add(1);
                }
                iteration += 1;

                // Print every 10,000 iterations to see it's running
                if iteration % 10_000 == 0 {
                    println!(
                        "[Issue 2] Still running... iteration {}, counter {}",
                        iteration, counter
                    );
                }
                // No await point!
            }
        });

        // Some healthy tasks for comparison
        for i in 0..2 {
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    println!("Healthy task {}: all good!", i);
                }
            });
        }

        // Monitoring task
        tokio::spawn(async {
            let mut tick = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                tick += 1;
                println!("\n=== Status Update #{} ===", tick);
                println!("Check tokio-console to identify:");
                println!("- Tasks with high self-wake %");
                println!("- Tasks that never yield");
                println!("- Lost waker warnings");
                println!("- Long poll times");
                println!("- Task count growth");
                println!("========================\n");
            }
        });

        // Keep the program running
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
