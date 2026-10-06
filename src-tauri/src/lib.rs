//! Protocol building blocks for the planned solidify MUD client.
//!
//! This library provides byte-level Telnet decoding/encoding and configurable
//! Q-method option negotiation, bounded Tokio TCP sessions, and independent
//! UTF-8/basic ANSI presentation decoding. It does not implement specific option
//! behavior, render a terminal, or initialize Tauri.
//! Its Rust interfaces are internal project contracts, not a stable plugin API.

#![forbid(unsafe_code)]

pub mod protocols;
pub mod sessions;
