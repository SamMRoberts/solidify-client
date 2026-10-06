//! Protocol building blocks for the planned solidify MUD client.
//!
//! This library provides byte-level Telnet decoding/encoding and configurable
//! Q-method option negotiation. It does not open connections, implement specific
//! option behavior, decode text, or initialize Tauri.
//! Its Rust interfaces are internal project contracts, not a stable plugin API.

#![forbid(unsafe_code)]

pub mod protocols;
