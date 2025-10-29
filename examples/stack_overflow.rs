//! Example demonstrating stack overflow caused by large futures
//!
//! This example shows how holding large data on the stack across await points,
//! especially in recursive async functions, can lead to stack overflow.
//!
//! ⚠️  WARNING: This example WILL crash with a stack overflow!
//! This is intentional to demonstrate the problem.
//!
//! Run this with:
//! ```
//! cargo run --example stack_overflow
//! ```
//!
//! Expected result: Stack overflow crash

use std::time::Duration;

// Scenario 1: Deep recursion with large data (WILL CRASH)
fn deep_async_bad(depth: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        let data = [0u8; 100_000]; // 100 KB per level

        if depth < 10000000 {
            // ❌ Recursive call with large data on stack
            // 100 levels × 100 KB = 10 MB stack usage!
            deep_async_bad(depth + 1).await;
        }

        println!("Level {} with data len {}", depth, data.len());
    })
}

// Scenario 2: Deep recursion with boxed data (SAFE)
fn deep_async_good(depth: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        // ✅ Box moves data to heap
        let data = Box::new([0u8; 100_000]); // Only 8 bytes on stack (pointer)

        if depth < 100 {
            deep_async_good(depth + 1).await;
        }

        println!("Level {} with data len {}", depth, data.len());
    })
}

#[tokio::main]
async fn main() {
    // This WILL crash with stack overflow
    deep_async_bad(0).await;
}
