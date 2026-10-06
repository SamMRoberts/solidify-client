//! Protocol building blocks for the planned solidify MUD client.
//!
//! This library provides byte-level Telnet decoding/encoding and configurable
//! Q-method option negotiation, plus bounded Tokio TCP sessions. It does not
//! implement specific option behavior, decode text, or initialize Tauri.
//! Its Rust interfaces are internal project contracts, not a stable plugin API.

#![forbid(unsafe_code)]

pub mod protocols;
pub mod sessions;
