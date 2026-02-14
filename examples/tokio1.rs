// tokio1.rs: Single-Threaded Runtime with Spawned Tasks
// What it demonstrates:

// Creating explicit runtime with Builder
// Running async code from sync context
// Spawning multiple concurrent tasks
// Mixing async I/O (fs::read) with blocking work

// key flow:

// main() [sync]
//   ↓ spawn OS thread
//   ↓ create single-threaded Tokio runtime
//   ↓ block_on(run()) [execute async]
//     ├→ spawn task 1: fs::read() [async I/O]
//     └→ spawn task 2: expensive_blocking_task() [blocks]
//     ↓ sleep(1 sec) - yields control
//   ↓ both tasks complete

// Key learning: How to manually build and control a Tokio runtime. Shows that despite single thread, both I/O and blocking work proceed concurrently.

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

use std::{thread, time::Duration}; //OS thread and timing utilities

use tokio::{
    fs,                          //Async file system operations
    runtime::{Builder, Runtime}, //tokio::runtime::Builder: to build runtime manually. Runtime: Tokio runtime type
    time::sleep,                 //Async sleep (doesn't block thread)
};

fn main() {
    let handle = thread::spawn(|| {
        //creates new OS thread. Closure || { } runs in that thread
        let rt = Builder::new_current_thread().enable_all().build().unwrap(); //Build single-threaded runtime with all features enabled
                                                                              //run(&rt) is async function that receives runtime reference
        rt.block_on(run(&rt)); //Execute async function from sync context. block this thread until async function completes.
    });

    handle.join().unwrap(); //Wait for spawned thread(OS thread) to finish before exiting main
}

// Blocking work function
// This function blocks whatever thread calls it.
// Simulates a blocking task by sleeping for 800ms and then computing a BLAKE3 hash of the input string.
// This function runs in the spawned task within the Tokio runtime.
fn expensive_blocking_task(s: String) -> String {
    thread::sleep(Duration::from_millis(800)); //Simulate blocking work. CPU-intensive (must block)
    blake3::hash(s.as_bytes()).to_string() //Compute BLAKE3 hash and return as hex string. Compute hash (can't be made async, must block)
}

// First spawned task
async fn run(rt: &Runtime) {
    rt.spawn(async {
        //Spawn first async task. Create task on this runtime
        println!("future 1"); //Log start of future 1. Executes immediately (before I/O)
        let content = fs::read("Cargo.toml").await.unwrap(); //Pause task, ask OS to read file. Read file asynchronously, yielding control while waiting. While paused, other tasks can run. .unwrap(): Panic if file not found
        println!("content: {:?}", content.len()); //Log length of file content
    });
    rt.spawn(async {
        //Spawn second async task
        println!("future 2"); //Log start of future 2
        let result = expensive_blocking_task("hello".to_string()); //Run blocking task. Calls blocking function (doesn't use .await because function is sync). Important: This blocks the single thread while computing hash. But first task already started, so output interleaves
        println!("result: {}", result); //Log result of blocking task
    });
    // sleep().await: Pause this run() function
    // Yields control to let spawned tasks progress
    // After 1 second, run() completes, main exits
    sleep(Duration::from_secs(1)).await; //Sleep to allow tasks to complete. Yield control to runtime to let spawned tasks run. Sleep for 1 second (longer than blocking task) to ensure both tasks finish before run() exits
}

// future 1
// future 2
// result: ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f
// content: 4052

// Execution Timeline:
// Time    Main Thread                   Task 1 (fs::read)        Task 2 (blocking hash)
// ────────────────────────────────────────────────────────────────────────────────────
// 0ms     thread::spawn()
//         └─ creates OS thread #2

//         [OS Thread #2 starts]
//         Builder::new_current_thread()
//         └─ creates runtime

//         rt.block_on(run(&rt))
//         └─ starts running run() async function

// 5ms     rt.spawn(task1)              ✓ Task created

// 6ms     rt.spawn(task2)                                       ✓ Task created

// 7ms     sleep(1 sec).await
//         └─ main run() pauses here

//         [Runtime scheduler: "task1 or task2?"]

// 8ms                                  ► fs::read("Cargo.toml")
//                                       prints "future 1"
//                                       .await ◄─ PAUSES HERE
//                                       └─ asks OS to read disk
//                                       (tells scheduler: "wake me when done")

// 9ms     [Scheduler: no other choice, task2 runs]
//                                                                ► expensive_blocking_task()
//                                                                prints "future 2"
//                                                                thread::sleep(800ms)
//                                                                ◄─ BLOCKS HERE
//                                                                (No .await! Actual blocking!)
// 10ms    [OS finishes disk read, wakes task1]
//         [But... scheduler can't run task1 yet!]
//         Why? Because task2 is BLOCKING the only thread!

//         ✗ DEADLOCK SITUATION: Task1 ready, but can't run
//         ✗ Task2 is sleeping, holding the thread hostage

// ...
// 100ms   [After 800ms of task2 sleeping]
//                                                                ✓ thread::sleep(800ms) done
//                                                                ✓ hash completes
//                                                                prints result
//                                                                ✓ Task2 finishes

// 105ms   [NOW scheduler can run task1]
//                                       ◄─ RESUMES (waiting 95ms!)
//                                       gets disk content
//                                       prints content length
//                                       ✓ Task1 finishes

// 1010ms  [Main sleep(1 sec) completes]
//         run() function finishes

// 1015ms  handle.join()
//         ✓ OS Thread #2 exits

//         main() returns
//         Program ends
