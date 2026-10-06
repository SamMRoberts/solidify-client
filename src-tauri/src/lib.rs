//! Protocol building blocks for the planned solidify MUD client.
//!
//! This library currently provides byte-level Telnet framing only. It does not
//! open connections, negotiate options, decode text, or initialize Tauri.
//! Its Rust interfaces are internal project contracts, not a stable plugin API.

#![forbid(unsafe_code)]

pub mod protocols;
