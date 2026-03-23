pub mod client;
pub mod config;
pub mod pi_client;
pub mod pi_rpc;
/// Rush daemon implementation for sub-millisecond startup via persistent server
///
/// This module implements the daemon architecture specified in docs/daemon-architecture.md:
/// - `protocol`: Message framing and serialization (length-prefixed binary format)
/// - `server`: Unix socket server and accept loop (with fork-based session workers)
/// - `worker`: Fork-based session workers (per-client isolation)
/// - `client`: Thin client logic for daemon communication
/// - `config`: Configuration parsing from .rushrc (banner, custom stats)
/// - `pi_client`: Client for Pi agent IPC over Unix sockets
/// - `pi_rpc`: Pi RPC subprocess manager for fast `|?` execution
pub mod protocol;
pub mod server;
pub mod worker;
pub mod worker_pool;

pub use client::DaemonClient;
pub use config::{BannerConfig, BannerShow, BannerStyle, CustomStatConfig, DaemonConfig};
pub use pi_client::{PiClient, PiClientError};
pub use pi_rpc::{PiCommand, PiEvent, PiRpcError, PiRpcManager};
pub use protocol::{
    decode_message,
    encode_message,
    read_message,
    write_message,
    Execute,
    ExecutionResult,
    Message,
    PiToRush,
    // Rush ↔ Pi IPC types
    RushToPi,
    SessionInit,
    SessionInitAck,
    ShellContext,
    Shutdown,
    Signal,
    StatsRequest,
    StatsResponse,
};
pub use server::{CustomStatCached, DaemonServer, SessionHandle, SessionId, StatsCache};
pub use worker_pool::{PoolConfig, PoolStats, Worker, WorkerPool, WorkerState};
