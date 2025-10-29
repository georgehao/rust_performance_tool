//! Example demonstrating the "self-wakes" issue
//!
//! This example shows tasks that wake themselves excessively,
//! which is inefficient and can indicate a bug in async code.
//!
//! Self-waking occurs when a task's waker is called by the task itself
//! (often through cx.waker().wake_by_ref() or similar patterns).
//! A high self-wake percentage (>50%) usually indicates a problem.
//!
//! This example demonstrates:
//! 1. A custom Future using explicit cx.waker().wake_by_ref() (BAD)
//! 2. A better Future that yields properly with sleep (GOOD)
//! 3. The common Notify pattern that causes self-wakes (BAD)
//! 4. Normal tasks for comparison (GOOD)
//!
//! Run this with:
//! ```
//! cargo run --example self_wakes
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```
//!
//! In tokio-console, look for:
//! - High "self-wake %" in the task list (red flag if >50%)
//! - Warnings panel showing self-wake warnings
//! - Compare different tasks to see the difference

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Notify;

// Custom Future that demonstrates explicit self-waking using wake_by_ref()
struct SelfWakingFuture {
    count: u32,
    max_count: u32,
}

impl SelfWakingFuture {
    fn new(max_count: u32) -> Self {
        Self {
            count: 0,
            max_count,
        }
    }
}

impl Future for SelfWakingFuture {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        self.count += 1;

        if self.count >= self.max_count {
            println!("  [SelfWakingFuture] Completed after {} polls", self.count);
            Poll::Ready(self.count)
        } else {
            if self.count % 10 == 0 {
                println!(
                    "  [SelfWakingFuture] Poll #{}, waking self immediately...",
                    self.count
                );
            }

            // 🔥 BAD PATTERN: Immediately wake ourselves!
            // This causes the executor to poll us again right away
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }
}

// A better version that yields periodically
struct BetterYieldingFuture {
    count: u32,
    max_count: u32,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl BetterYieldingFuture {
    fn new(max_count: u32) -> Self {
        Self {
            count: 0,
            max_count,
            sleep: None,
        }
    }
}

impl Future for BetterYieldingFuture {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        loop {
            // Check if we have a sleep in progress
            if let Some(mut sleep) = self.sleep.take() {
                match sleep.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        // Sleep done, continue
                    }
                    Poll::Pending => {
                        // Still sleeping, put it back
                        self.sleep = Some(sleep);
                        return Poll::Pending;
                    }
                }
            }

            self.count += 1;

            if self.count >= self.max_count {
                println!(
                    "  [BetterYieldingFuture] Completed after {} iterations",
                    self.count
                );
                return Poll::Ready(self.count);
            }

            // ✅ GOOD: Yield with a sleep instead of immediate wake
            self.sleep = Some(Box::pin(tokio::time::sleep(Duration::from_millis(10))));
        }
    }
}

fn main() {
    // Initialize console subscriber for tokio-console
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("Starting self-wakes example...");
        println!("This demonstrates tasks that wake themselves too frequently.");
        println!("Connect with: tokio-console");
        println!("Look for high self-wake percentage in the tasks view!");
        println!();

        // Scenario 1: Custom Future with explicit wake_by_ref()
        println!("[Scenario 1] Custom Future using wake_by_ref()");
        tokio::spawn(async {
            println!("  Starting SelfWakingFuture (BAD pattern)...");
            let result = SelfWakingFuture::new(100).await;
            println!("  SelfWakingFuture completed with result: {}", result);
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Scenario 2: Better version with proper yielding
        println!("\n[Scenario 2] Better Future with proper sleep");
        tokio::spawn(async {
            println!("  Starting BetterYieldingFuture (GOOD pattern)...");
            let result = BetterYieldingFuture::new(50).await;
            println!("  BetterYieldingFuture completed with result: {}", result);
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        println!();

        // Scenario 3: Using Notify (common pattern that causes self-wakes)
        println!("[Scenario 3] Using Notify to self-wake (BAD pattern)");
        tokio::spawn(async {
            let notify = Arc::new(Notify::new());
            let notify_clone = notify.clone();

            // Task that keeps waking itself
            tokio::spawn(async move {
                loop {
                    // Wait to be notified
                    notify_clone.notified().await;

                    // Do some "work"
                    tokio::time::sleep(Duration::from_millis(10)).await;

                    // Immediately wake ourselves again (BAD PATTERN!)
                    notify_clone.notify_one();
                }
            });

            // Start the cycle
            notify.notify_one();

            // Keep the task alive
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        // Some normal tasks for comparison
        for i in 0..3 {
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    println!("Normal task {} working...", i);
                }
            });
        }

        // Keep the program running
        let mut tick = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            tick += 1;
            println!("\n=== Status Update #{} ===", tick);
            println!("Program still running... Check tokio-console!");
            println!("\nExpected observations in tokio-console:");
            println!("1. SelfWakingFuture task: Very high self-wake % (close to 100%)");
            println!("2. BetterYieldingFuture task: Low self-wake % (should be 0%)");
            println!("3. Notify-based task: High self-wake % (90%+)");
            println!("4. Normal tasks: 0% self-wake");
            println!("===================\n");
        }
    });
}
