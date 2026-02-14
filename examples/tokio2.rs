// tokio2.rs: Async Tokio Entry Point with Channels
// What it demonstrates:

// Using #[tokio::main] macro (automatic runtime setup)
// Async tasks sending data via channels
// OS thread pool handling blocking work
// Producer-consumer pattern with mpsc

// Key flow:

// #[tokio::main] creates multi-threaded runtime automatically
//   ↓ spawn async task 1: producer (infinite send loop)
//   ├→ sends 32 messages into channel (buffer of 32)
//   ├→ .await on send if buffer full
//   ↓ worker() thread receives messages
//   ├→ spawns OS thread for each blocking task
//   ├→ collects result via sync::mpsc
//   └→ prints results

// Key learning: Real-world pattern—async producer sends work, thread worker pool processes blocking operations, results reported back.

// Critical Difference Between Them
// Aspect	        tokio1.rs	            tokio2.rs
// Runtime setup	Manual Builder	        Auto via #[tokio::main]
// Threading	    Single-threaded Tokio	Multi-threaded (default)
// Task spawning	rt.spawn()	            tokio::spawn()
// Communication	Direct async spawning	Channels (mpsc)
// Use case	        Learning/control	    Production web servers

// The choice depends on use case:
// tokio1: Learning, debugging, CPU-bound work
// tokio2: Production servers, producer-consumer patterns

use std::{thread, time::Duration};
use tokio::sync::mpsc;

// #[tokio::main] macro that:
// Creates multi-threaded Tokio runtime automatically
// Converts main() to async (runtime calls it)
// Handles runtime cleanup on exit
#[tokio::main]
async fn main() {
    // tokio task send string to expensive_blocking_task for execution
    // 1, Create async channel
    // mpsc::channel(32): Multi-producer, single-consumer with buffer of 32
    // tx (transmitter): Send messages
    // rx (receiver): Receive messages
    // Buffer=32: Can hold 32 messages before blocking senders
    let (tx, rx) = mpsc::channel(32);
    // 2, Start worker thread
    // worker(rx) spawns OS thread, returns JoinHandle
    // Worker receives messages from channel
    let handle = worker(rx); //Start worker thread to receive from channel

    // 3, Producer task (async)
    // tokio::spawn(): Create Tokio task (runs on runtime thread pool)
    // async move: Moves tx into closure
    // loop: Infinite sender
    // tx.send().await: Send message, pause if buffer full
    // Each sent message is a task description
    tokio::spawn(async move {
        let mut i = 0;
        loop {
            i += 1;
            println!("sending task {}", i);
            tx.send(format!("task {i}")).await.unwrap();
        }
    });
    // 4, Wait for worker thread
    // handle.join(): Block main task until worker thread finishes
    // Never finishes (infinite loop), so main sleeps here
    handle.join().unwrap();
}

// Worker function runs in OS thread
// thread::spawn(move): New OS thread receives ownership of rx
// Inside: rx is blocking-capable (not async)
fn worker(mut rx: mpsc::Receiver<String>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Create sync channel and receive
        // std::sync::mpsc: Standard library sync channel (not async)
        // rx.blocking_recv(): Receive from Tokio async channel in sync way
        // Blocks OS thread until message arrives
        // Some(s): Message received, None if channel closed
        // while let Some(s): Loop while messages exist
        let (sender, receiver) = std::sync::mpsc::channel();
        while let Some(s) = rx.blocking_recv() {
            // Spawn OS thread for each task
            // sender_clone: Clone sync channel sender for this thread
            // thread::spawn(): New thread for blocking work
            // expensive_blocking_task(s): Compute hash (800ms blocking)
            // sender.send(ret): Send result back via sync channel
            let sender_clone = sender.clone();
            thread::spawn(move || {
                let ret = expensive_blocking_task(s);
                sender_clone.send(ret).unwrap();
            });
            // Receive result and print
            // receiver.recv(): Block and wait for thread to complete
            // println!(): Print hash result
            // Loop continues for next message
            let result = receiver.recv().unwrap();
            println!("result: {}", result);
        }
    })
}

fn expensive_blocking_task(s: String) -> String {
    thread::sleep(Duration::from_millis(800));
    blake3::hash(s.as_bytes()).to_string()
}

// sending task 1-32 (buffer fills)     ← Producer sends messages
// result: eb5...                       ← First task completes (800ms)
// sending task 33                      ← Producer unblocked, sends more
// result: f63...                       ← Next task completes
// ...

// sending task 1
// sending task 2
// sending task 3
// sending task 4
// sending task 5
// sending task 6
// sending task 7
// sending task 8
// sending task 9
// sending task 10
// sending task 11
// sending task 12
// sending task 13
// sending task 14
// sending task 15
// sending task 16
// sending task 17
// sending task 18
// sending task 19
// sending task 20
// sending task 21
// sending task 22
// sending task 23
// sending task 24
// sending task 25
// sending task 26
// sending task 27
// sending task 28
// sending task 29
// sending task 30
// sending task 31
// sending task 32
// sending task 33
// sending task 34
// result: eb5c58ad65c9cebf686ca58859d832d0c2d4caf663764abaa23d4401c13404de
// sending task 35
// result: f63daa30a8b3e4252ef01bdcf20c10c279e538f982be9d637a11112813d0a95d
// sending task 36
// result: b6647baf1e810fdca7c87d9314a16572666d17972208f6d43a5f1ed0964c62dc
// sending task 37
// result: 275504c8ad05abf96c69f87f24e2329d9fa908c482ecb71917353798fa3a0a07
// sending task 38
