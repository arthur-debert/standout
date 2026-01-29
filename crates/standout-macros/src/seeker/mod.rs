//! Implementation of the `#[derive(Seekable)]` macro.
//!
//! This module provides derive macro support for the Seeker query engine,
//! generating accessor functions and field constants from struct annotations.

mod attrs;
mod derive;

pub use derive::seekable_derive_impl;
