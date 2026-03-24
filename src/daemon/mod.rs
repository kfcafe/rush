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
pub use pi_client::{PiClient, PiClientError};
pub use protocol::{PiToRush, ShellContext};
