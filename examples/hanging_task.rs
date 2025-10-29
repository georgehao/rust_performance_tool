//! Example demonstrating "hanging task" issues
//!
//! This example shows tasks that get stuck waiting forever,
//! never completing and leaking resources.
//!
//! Run this with:
//! ```
//! cargo run --example hanging_task
//! ```
//!
//! Then in another terminal:
//! ```
//! tokio-console
//! ```

use std::future::pending;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

fn main() {
    console_subscriber::init();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("Starting hanging task example...");
        println!("This demonstrates tasks that hang forever and never complete.");
        println!("Connect with: tokio-console");
        println!("Look for tasks with continuously growing Idle time!");
        println!();

        // Scenario 1: Using pending() - the most obvious hanging task
        tokio::spawn(async {
            println!("Task 1: Using pending() - will hang forever");
            pending::<()>().await;
            println!("This will NEVER print!");
        });

        // Scenario 2: Waiting on a channel that will never receive data
        let (_tx, mut rx) = mpsc::channel::<String>(10);
        // Note: We keep _tx alive but never send anything

        tokio::spawn(async move {
            println!("Task 2: Waiting for channel message that never comes...");
            match rx.recv().await {
                Some(msg) => println!("Received: {}", msg),
                None => println!("Channel closed"),
            }
            println!("Task 2 completed (will never reach here)");
        });

        // Scenario 3: Waiting for oneshot that never sends
        let (_tx, rx) = oneshot::channel::<i32>();

        tokio::spawn(async move {
            println!("Task 3: Waiting for oneshot signal...");
            match rx.await {
                Ok(value) => println!("Received value: {}", value),
                Err(_) => println!("Sender dropped"),
            }
        });

        // Scenario 4: Joining a task that runs forever
        let infinite_task = tokio::spawn(async {
            let mut counter = 0u64;
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                counter += 1;
                println!("Infinite task tick: {}", counter);
            }
        });

        tokio::spawn(async move {
            println!("Task 4: Waiting to join infinite task...");
            let _ = infinite_task.await;
            println!("Infinite task completed (will never happen)");
        });

        // Scenario 5: Deadlock-like situation with channels
        let (tx1, mut rx1) = mpsc::channel::<String>(1);
        let (tx2, mut rx2) = mpsc::channel::<String>(1);

        tokio::spawn(async move {
            println!("Task 5a: Waiting for message from Task 5b...");
            if let Some(msg) = rx1.recv().await {
                println!("5a received: {}", msg);
                let _ = tx2.send("Reply from 5a".to_string()).await;
            }
        });

        tokio::spawn(async move {
            println!("Task 5b: Waiting for message from Task 5a...");
            if let Some(msg) = rx2.recv().await {
                println!("5b received: {}", msg);
                let _ = tx1.send("Reply from 5b".to_string()).await;
            }
        });
        // Neither task sends first, so both hang forever!

        // Scenario 6: Waiting with no timeout on slow operation
        tokio::spawn(async {
            println!("Task 6: Simulating hung HTTP request (no timeout)...");
            // In real code, this might be a network request that hangs
            pending::<()>().await;
            println!("Request completed (never happens)");
        });

        //Scenario 7: Lock/synchronization issue
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let data = Arc::new(Mutex::new(0));
        let data_clone = data.clone();

        tokio::spawn(async move {
            println!("Task 7a: Acquiring lock and holding it...");
            let _guard = data.lock().await;
            println!("Task 7a: Lock acquired, now hanging...");
            pending::<()>().await; // Hold lock forever!
        });

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            println!("Task 7b: Trying to acquire lock...");
            let _guard = data_clone.lock().await;
            println!("Task 7b: Lock acquired! (will never happen)");
        });

        // Monitoring task to print status
        tokio::spawn(async {
            let mut tick = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                tick += 1;
                println!("\n=== Status Update #{} ===", tick);
                println!("Check tokio-console for:");
                println!("- Tasks with state: Idle");
                println!("- Continuously growing Idle time");
                println!("- Tasks that never complete");
                println!("- Increasing task count (memory leak)");
                println!("========================\n");
            }
        });

        // Keep the program running
        println!("\nProgram running. Watch the hanging tasks in tokio-console!");
        println!("You should see multiple tasks stuck in Idle state.\n");

        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            println!("Main: Still running with {} hanging tasks...", 8);
        }
    });
}
