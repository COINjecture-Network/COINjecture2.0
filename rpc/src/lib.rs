// COINjecture RPC Server
// JSON-RPC API for clients

// RpcEvent/Transaction carry large values by design; boxing would ripple through all match arms.
#![allow(clippy::large_enum_variant)]

pub mod server;
pub mod websocket;

pub use server::*;
pub use websocket::*;
