//! Protocol, session, and application building blocks for the solidify MUD client.
//!
//! This library provides byte-level Telnet decoding/encoding and configurable
//! Q-method option negotiation, bounded Tokio TCP sessions, and independent
//! UTF-8/ANSI presentation decoding, plus a bounded single-connection
//! application coordinator. The optional desktop binary initializes Tauri and
//! hosts the React transcript renderer; protocol APIs remain UI-independent.
//! Presentation styles include basic flags, bright/indexed colors, and direct RGB.
//! An opt-in profile implements passive TTYPE/NAWS, remote ECHO and SGA.
//! The optional `profiles` feature adds bounded saved connections and transcript
//! appearance with strict validation, exclusive locking and atomic replacement.
//! Other option extensions and full terminal emulation are deferred.
//! Its Rust interfaces are internal project contracts, not a stable plugin API.

#![forbid(unsafe_code)]

pub mod application;
pub mod protocols;
pub mod sessions;

#[cfg(feature = "profiles")]
pub mod storage;
