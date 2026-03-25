// cargo run --example chat
// telnet 0.0.0.0 8080 // 新开两个终端，输入telnet命令连接到服务器，输入用户名后就可以聊天了

// Summary of the Final Blueprint
// tokio provides the infrastructure (networking, lightweight threading).
// tokio-util + futures constructs the data pipeline (bytes ↔ lines).
// Arc + DashMap creates the global registry safely shared across the infrastructure.
// tokio::sync::mpsc provides the postal service routing data between isolated network tasks.
// tracing + anyhow acts as the dashboard, ensuring you know exactly what is happening inside the machine.

// Foundation: LinesCodec
// A protocol-aware codec that knows how to split bytes by newlines (\n)
// Transforms raw bytes ↔ strings

// Middle Layer: Framed<TcpStream, LinesCodec>
// Wraps a TcpStream with the LinesCodec
// Creates a bidirectional message stream where each message is a line
// Implements both Sink (sending lines) and Stream (receiving lines) traits from futures

// Upper Layers: The Trait Extensions
// SinkExt — adds convenience methods like .send() to anything implementing Sink
// StreamExt — adds convenience methods like .next(), .split() to anything implementing Stream
// SplitStream — the receiver half after calling .split() on a Framed

// Summary
// TcpStream: Plumber. Moves raw bytes back and forth across the internet. (AsyncRead / AsyncWrite)
// LinesCodec: Translator. Knows how to turn bytes ending in \n into Strings, and Strings into bytes ending in \n. (Decoder / Encoder)
// Framed: The Manager. Owns the memory buffers, drives the TcpStream to fetch data, hands that data to the Translator, and gives you a clean Stream of Strings.

// 1. The Rulebook: LinesCodec
// LinesCodec implements two traits: Decoder (for reading) and Encoder (for writing). It acts as the "rulebook" for what constitutes a message.
// As a Decoder: It knows exactly one trick: scan a chunk of bytes until you find \n or \r\n. If it finds one, it extracts those bytes, validates that they are valid UTF-8, and converts them into a Rust String.
// As an Encoder: It takes a Rust String, adds a \n to the end, and converts it into a chunk of bytes.

// 2. The Engine: Framed
// Framed is an adapter. It takes an underlying I/O stream (your TcpStream) and pairs it with the codec (LinesCodec).
// Under the hood, Framed manages two internal memory buffers (usually using a fast byte-manipulation library called BytesMut): a Read Buffer and a Write Buffer.

// When you run this line of code:
// let username = stream.next().await;

// Here is the exact step-by-step mechanism occurring behind the scenes:

// Request: Your code asks stream (which is the Framed wrapper) for the .next() message.
// Inspect Buffer: Framed looks at its internal Read Buffer and asks the LinesCodec: "Can you decode a message from what we currently have?"
// Read from OS (if needed): If the codec says "No, I haven't seen a \n yet", Framed puts the task to sleep. It waits for the OS to say there is new TCP data. Once data arrives, Framed reads those raw bytes from the TcpStream and appends them to its Read Buffer.
// Extract & Parse: Once Framed has enough bytes, the LinesCodec spots the \n. It cuts that exact sequence of bytes out of the buffer, translates it into a standard Rust String, and leaves the remaining unparsed bytes in the buffer for the next call.
// Return: Framed returns Some(Ok(String)) back to your stream.next().await call.
// Reversing the Magic (Sending data)
// The same magic happens in reverse when you write code to send a message:
// stream.send("Enter your username:").await?;

// Framed passes the &str to LinesCodec.
// LinesCodec encodes it into bytes and appends \n (e.g., [69, 110, 116, ... , 10]).
// Framed pushes these bytes into its internal Write Buffer.
// Framed automatically manages flushing that buffer down into the raw TcpStream to go over the network.

use anyhow::Result;
use dashmap::DashMap;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use std::{fmt, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _}; //因为两个Layer冲突，后者是trait，我们只需要使用其方法，所以，用匿名_来引入。

// The channel will allocate a bounded queue that can hold exactly 128 unread messages.
// It Prevents Out-Of-Memory (OOM) Crashes
// Without this limit (an unbounded channel), a slow network connection combined with a highly active chat room would cause the queue to grow infinitely (10,000... 1,000,000 messages...) until your entire server crashes from running out of RAM.
// By capping it at 128, you are telling the OS: "If a user's connection lags behind by 128 messages, force the rest of the system to wait for them to catch up rather than consuming all my server's memory."
const MAX_MESSAGES: usize = 128;

// State: The global shared memory (Arc<State>).
// It holds a DashMap where the key is the client's SocketAddr and the value is an mpsc::Sender.
// This sender is the "mailbox" used to send messages to that specific client.
// However, there is a specific reason it's often written this way in tutorials: It is a massive idiom in the Rust async/web ecosystem.

// The "State" Idiom in Rust
// In frameworks like Axum, Actix, and bare Tokio servers, developers almost always create a struct called State, AppState, or ServerState to hold global variables that need to be wrapped in an Arc and shared across hundreds of connections.
// Right now, your State only has one field:peers;
// But typically, as a server grows, developers just keep dumping other global resources into this struct:
// struct State {
//     peers: DashMap<SocketAddr, mpsc::Sender<Arc<Message>>>,
//     db_pool: PgPool,              // Database connection
//     metrics: Registry,            // Prometheus metrics
//     banned_ips: DashSet<IpAddr>,  // Security
// }

// 对于三个核心数据结构 State, Peer, Message 的理解，完全达到了一个资深系统工程师写高级异步Rust代码的水平！
// Your understanding of
// how Rust decouples the Routing (State),
// the I/O (Peer),
// and the Payload (Message)
// is completely spot on the level of a senior systems engineers who have been writing concurrent Rust code for a senior-level async systems!

#[derive(Debug, Default)]
struct State {
    peers: DashMap<SocketAddr, mpsc::Sender<Arc<Message>>>, // the "Registry" that maps each client's SocketAddr to their personal message sender (mailbox)
}

// Peer: The Local Worker
// Peer: Represents the local state of a connected user.
// While State is global, Peer is strictly local to the specific tokio::spawn task created when a user connects.

// What it holds: It holds the user's username and the SplitStream (the "Read Half" of the TCP connection we discussed earlier).
// Maintenance: Peer only exists while the handle_client function is running. Once the user disconnects, the function finishes, and the Peer struct is instantly destroyed and dropped from memory.

// stream.split() creates stream_sender (SplitSink) and stream_receiver (SplitStream).
// stream_sender gets moved into a tokio::spawn task to handle broadcasting.
// stream_receiver is stored inside the Peer struct so the main loop can .next().await it to read what the user types.
// Maintenance: Peer only exists while the handle_client function is running. Once the user disconnects, the function finishes, and the Peer struct is instantly destroyed and dropped from memory.

// An Adapter (like Framed) takes one interface (AsyncRead/AsyncWrite raw bytes) and completely translates it into a totally different interface (Stream/Sink of Strings). It adapts a low-level pipe into a high-level iterator.
// Specifically, SplitStream is a Smart Pointer (with a Lock) that points to the original Framed object in memory.
#[derive(Debug)]
struct Peer {
    username: String,
    stream: SplitStream<Framed<TcpStream, LinesCodec>>, // where a user types messages, and we read them with .next().await
}

// Message: An enum representing the types of events in the system (Join, Leave, Chat).
// It implements Display to automatically format how these events look as text.
#[derive(Debug)]
enum Message {
    UserJoined(String),
    UserLeft(String),
    Chat { sender: String, content: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initializes tracing/logging with INFO level
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    // 因为会和上述 tracing_subscriber::registry().with(layer).init() 冲突，所以直接用下面的init来初始化日志系统，默认是INFO级别
    // console_subscriber::init();

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("Starting chat server on {}", addr);
    let state = Arc::new(State::default());

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Accepted connection from: {}", addr);
        let state_cloned = state.clone();
        tokio::spawn(async move {
            // if let Err(e) = handle_client(state_cloned, addr, stream).await {
            //     warn!("Failed to handle client {}: {}", addr, e);
            // }
            match handle_client(state_cloned, addr, stream).await {
                Ok(_) => {
                    // Client disconnected normally, nothing to do
                }
                Err(e) => {
                    warn!("Failed to handle client {}: {}", addr, e);
                }
            }
        });
    }
}

// Step 2: Client Handshake (handle_client)
// Framing: Wraps the raw byte stream in Framed::new(stream, LinesCodec::new()). This magically turns a stream of raw bytes into a stream of String lines.
// Asks the user for their username and waits for the first line of input.
// Framed acts as a adapter, managing the internal buffers and translating between raw bytes and Strings according to the LinesCodec rules. It handles all the complexity of reading from the network, buffering data, and parsing it into lines for you.
// Framed's adapter pattern allows you to work with high-level abstractions (like Strings) while it takes care of the low-level details (like TCP buffering and byte manipulation). This is a common pattern in Rust's async ecosystem, where you often wrap raw streams in layers of adapters to get the exact interface you need for your application logic.
// Framed turns the lower api of AsyncRead/AsyncWrite into a higher api of Stream/Sink of Strings, and also manages the internal buffers and the state of the TCP connection for you.
async fn handle_client(state: Arc<State>, addr: SocketAddr, stream: TcpStream) -> Result<()> {
    let mut framed_stream = Framed::new(stream, LinesCodec::new());
    framed_stream.send("Enter your username:").await?;

    let username = match framed_stream.next().await {
        Some(Ok(username)) => username,
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };

    let mut peer = state.add(addr, username, framed_stream).await;

    let message = Arc::new(Message::user_joined(&peer.username));
    info!("{}", message);
    state.broadcast(addr, message).await;

    while let Some(line) = peer.stream.next().await {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                warn!("Failed to read line from {}: {}", addr, e);
                break;
            }
        };

        let message = Arc::new(Message::chat(&peer.username, line));

        state.broadcast(addr, message).await;
    }

    // when while loop exit, peer has left the chat or line reading failed
    // remove peer from state
    state.peers.remove(&addr);

    // notify others that a user has left
    let message = Arc::new(Message::user_left(&peer.username));
    info!("{}", message);

    state.broadcast(addr, message).await;

    Ok(())
}

impl State {
    async fn broadcast(&self, addr: SocketAddr, message: Arc<Message>) {
        for peer in self.peers.iter() {
            if peer.key() == &addr {
                continue;
            }
            if let Err(e) = peer.value().send(message.clone()).await {
                warn!("Failed to send message to {}: {}", peer.key(), e);
                // if send failed, peer might be gone, remove peer from state
                self.peers.remove(peer.key());
            }
        }
    }

    // 这段代码的很多逻辑，应该放在 impl Peer 里，而不是 State 里。
    // Your intuition is 100% correct. You have an excellent eye for Separation of Concerns (SoC).
    // Right now, State::add is violating the Single Responsibility Principle. State is supposed to be just a Registry/Router, but right now it is heavily meddling in I/O setup by splitting the TCP stream and spawning background worker tasks.
    // That logic absolutely belongs in impl Peer! The Peer struct should be responsible for its own lifecycle, memory allocation, and background tasks.
    // If you refactor this, you would create a Peer::new() method that handles the heavy lifting, and State::add would shrink to just 2 lines.
    async fn add(
        &self,
        addr: SocketAddr,
        username: String,
        framed_stream: Framed<TcpStream, LinesCodec>,
    ) -> Peer {
        let (tx, mut rx) = mpsc::channel(MAX_MESSAGES);
        self.peers.insert(addr, tx);

        // ask user for username

        let (mut stream_sender, stream_receiver) = framed_stream.split();

        // receive messages from others, and send them to the client
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = stream_sender.send(message.to_string()).await {
                    warn!("Failed to send message to {}: {}", addr, e);
                    break;
                }
            }
        });

        // return peer
        Peer {
            username,
            stream: stream_receiver,
        }
    }
}

// 类似每个 Variant 的构造函数
// each functions inside impl Message is just a convenient constructor for creating different types of messages.
// It abstracts away the details of how the message content is formatted and allows you to create messages with simple function calls like Message::user_joined("Alice") instead of manually constructing the enum variants each time.
impl Message {
    fn user_joined(username: &str) -> Self {
        let content = format!("{} has joined the chat", username);
        Self::UserJoined(content)
    }

    fn user_left(username: &str) -> Self {
        let content = format!("{} has left the chat", username);
        Self::UserLeft(content)
    }

    fn chat(sender: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Chat {
            sender: sender.into(),
            content: content.into(),
        }
    }
}

// How write! fits in:
// The write!(f, ...) macro is just the mechanism that pushes characters into a "buffer".

// If you called .to_string(), f is a string buffer in memory.
// If you called info!("{}", message), f is the logging output buffer.
// By implementing fmt::Display once, you created a unified way to format the message for both the network transmission and your users see in Telnet (stream_sender) and the logs you see in your server console!

// The magic that opens up the enum and extracts the string is Pattern Matching (the match statement combined with variable destructuring).

// 每个 variant 该如何被格式化成字符串，完全由 fmt::Display 的实现决定了！
// Pattern Matching and Variable Destructuring
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserJoined(content) => write!(f, "[{}]", content),
            Self::UserLeft(content) => write!(f, "[{} :(]", content),
            Self::Chat { sender, content } => write!(f, "{}: {}", sender, content),
        }
    }
}

// How it unfolds behind the scenes:
// Step 1: The Trigger
// Something calls message.to_string() or info!("{}", message). Rust passes the Message enum into the fmt function as &self.

// Step 2: Inspecting the Enum (match self)
// Rust looks at the enum in memory. Under the hood, an enum is stored as a "Tag" (a hidden number that says which variant it is) plus the actual "Data."
// Rust checks the Tag and says: "Ah, this is the UserJoined variant!"

// Step 3: Destructuring (The Unpacking)
// This is where the actual extraction happens:
// Self::UserJoined(content) =>

// When Rust matches the UserJoined variant, it sees that you provided a variable name inside the parenthesis: (content).
// Rust automatically reaches inside the enum, grabs a reference to the inner String ("kevin has joined the chat"), and temporarily assigns it to a new local variable named content.
// (Note: You could have named this variable anything, like Self::UserJoined(my_string) =>).

// Step 4: Writing the Output
// Now that the inner text is safely held in the content variable, execution moves to the right side of the arrow =>:
// write!(f, "[{}]", content)

// The write! macro takes the string inside content, wraps it in literal brackets [ and ], and pushes it into the final text buffer.

// Graceful Shutdown: The Missing Piece
// The short answer is: That code does not exist in your chat.rs right now!

// Currently, your server has no "Graceful Shutdown" logic.

// What actually happens when you press Ctrl+C right now:
// You press Ctrl+C in the terminal.
// The OS sends a SIGINT (Interrupt Signal) to the Rust process.
// Because your chat.rs doesn't explicitly catch this signal, the Rust program instantly dies in the middle of whatever it was doing.
// The OS swoeps in, reclaims the server's memory, and forcefully closes all open TCP socket file descriptors.
// The telnet clients receive a raw TCP network packet (FIN or RST) from the operating system saying "the connection was dropped". They see something like Connection closed by foreign host. on their screen, but they never get a nice customized String from the application.
// How do we add Graceful Shutdown?
// To broadcast a message gracefully when the admin types Ctrl+C, you need to use two tools from Tokio:

// tokio::signal::ctrl_c() listens for the Ctrl+C command.
// tokio::select! races multiple async tasks at once (e.g. "Wait for a new user" vs "Wait for Ctrl+C").
