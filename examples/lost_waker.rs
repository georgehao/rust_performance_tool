//! Example demonstrating the "lost-waker" issue
//!
//! Lost-waker occurs when a task is dropped/cancelled while in Pending state,
//! without its waker being called. This indicates incomplete async operations
//! and can lead to resource leaks or logic errors.
//!
//! Run this with:
//! ```
//! cargo run --example lost_waker
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

// Custom Future that never saves waker - this is the real lost-waker problem!
struct NeverWakes {
    value: i32,
}

impl Future for NeverWakes {
    type Output = i32;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // BUG: We return Pending but DON'T save the waker from cx.waker()
        // This means nobody will ever wake this future up!
        println!("  NeverWakes polled (but waker not saved!)");
        Poll::Pending
    }
}

// Simulates a resource that needs cleanup
struct Resource {
    id: i32,
    name: String,
}

impl Resource {
    fn new(id: i32, name: &str) -> Self {
        println!("  [Resource {}] Allocated: {}", id, name);
        Self {
            id,
            name: name.to_string(),
        }
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        println!("  [Resource {}] Cleaned up: {}", self.id, self.name);
    }
}

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("=== Lost-Waker Examples ===");
        println!("This demonstrates tasks dropped in Pending state without being woken.");
        println!("Connect with: tokio-console to see 'lost waker' warnings!\n");

        // // Scenario 1: Custom Future that forgets to save waker
        // println!("\n[Scenario 1] Custom Future without saved waker");
        // for i in 0..3 {
        //     let handle = tokio::spawn(async move {
        //         let _resource = Resource::new(i, "Never woken");
        //         println!("Task {}: Waiting on NeverWakes future...", i);
        //         NeverWakes { value: i }.await;
        //         println!("Task {}: Completed (NEVER PRINTS)", i);
        //     });

        //     // // Abort after a short time - lost waker!
        //     // tokio::spawn(async move {
        //     //     tokio::time::sleep(Duration::from_millis(500)).await;
        //     //     println!("  Aborting task {} - Lost waker!", i);
        //     //     handle.abort();
        //     // });

        //     tokio::time::sleep(Duration::from_millis(200)).await;
        // }

        // tokio::time::sleep(Duration::from_secs(2)).await;

        // // Scenario 2: Aborting task during long async operation
        // println!("\n[Scenario 2] Task aborted during async I/O");
        // for i in 0..3 {
        //     let handle = tokio::spawn(async move {
        //         let _resource = Resource::new(100 + i, "DB Connection");
        //         println!("Task {}: Starting long database query...", i);

        //         // Simulate a long-running async operation
        //         tokio::time::sleep(Duration::from_secs(10)).await;

        //         println!("Task {}: Query completed (NEVER PRINTS)", i);
        //     });

        //     // Abort during the operation - lost waker!
        //     tokio::spawn(async move {
        //         tokio::time::sleep(Duration::from_millis(300)).await;
        //         println!("  Aborting DB task {} - Lost waker!", i);
        //         // handle.abort();
        //     });

        //     tokio::time::sleep(Duration::from_millis(150)).await;
        // }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        // // Scenario 3: select! causing lost wakers
        // println!("\n[Scenario 3] select! causing lost wakers on cancelled branches");
        // for i in 0..3 {
        //     tokio::spawn(async move {
        //         let _resource = Resource::new(200 + i, "Network Connection");

        //         let slow_branch = async {
        //             println!("Task {}: Slow branch waiting...", i);
        //             tokio::time::sleep(Duration::from_secs(5)).await;
        //             println!("Task {}: Slow branch done (NEVER PRINTS)", i);
        //             "slow"
        //         };

        //         let fast_branch = async {
        //             tokio::time::sleep(Duration::from_millis(100)).await;
        //             "fast"
        //         };

        //         // When fast_branch completes, slow_branch is dropped - lost waker!
        //         tokio::select! {
        //             _ = slow_branch => {
        //                 println!("Task {}: Slow completed", i);
        //             }
        //             _ = fast_branch => {
        //                 println!("Task {}: Fast completed (slow branch lost waker!)", i);
        //             }
        //         }
        //     });

        //     tokio::time::sleep(Duration::from_millis(200)).await;
        // }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        // Scenario 4: Timeout causing lost wakers
        println!("\n[Scenario 4] Timeout causing lost wakers");
        for i in 0..3 {
            tokio::spawn(async move {
                let _resource = Resource::new(300 + i, "File Handle");
                println!("Task {}: Starting operation with timeout...", i);

                let slow_op = async {
                    println!("  Task {}: Working...", i);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    println!("  Task {}: Work done (NEVER PRINTS)", i);
                    42
                };

                // Timeout causes the slow_op to be dropped - lost waker!
                match tokio::time::timeout(Duration::from_millis(200), slow_op).await {
                    Ok(result) => println!("Task {}: Completed with {}", i, result),
                    Err(_) => println!("Task {}: Timed out (lost waker on slow_op!)", i),
                }
            });

            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        // // Scenario 5: JoinHandle dropped before completion
        // println!("\n[Scenario 5] Dropping JoinHandle before task completes");
        // for i in 0..3 {
        //     let handle = tokio::spawn(async move {
        //         let _resource = Resource::new(400 + i, "Cache Entry");
        //         println!("Task {}: Long computation...", i);
        //         tokio::time::sleep(Duration::from_secs(5)).await;
        //         println!("Task {}: Computation done (MIGHT NOT PRINT)", i);
        //         i * 2
        //     });

        //     // Drop the JoinHandle without awaiting - lost waker!
        //     tokio::spawn(async move {
        //         tokio::time::sleep(Duration::from_millis(200)).await;
        //         println!("  Dropping JoinHandle {} without awaiting - Lost waker!", i);
        //         drop(handle);
        //     });

        //     tokio::time::sleep(Duration::from_millis(150)).await;
        // }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        println!("\n=== Impact Summary ===");
        println!("Lost-waker issues can cause:");
        println!("1. Resource leaks (connections, file handles not properly closed)");
        println!("2. Incomplete operations (partial writes, uncommitted transactions)");
        println!("3. Logic errors (cleanup code not executed)");
        println!("4. Difficult-to-debug intermittent failures");
        println!("\nCheck tokio-console for 'lost waker' warnings!");

        // Some healthy tasks for comparison
        println!("\n[Healthy Tasks] For comparison...");
        for i in 0..2 {
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    println!("Healthy task {}: Running normally", i);
                }
            });
        }

        // Keep the program running
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            println!("\n--- Still running, check tokio-console! ---");
        }
    });
}
