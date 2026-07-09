//! theorem-proving-practice
//!
//! A personal library of theorems plus a daily practice routine: draw a few
//! theorems at random and compile them into a nicely formatted PDF to prove by
//! hand. See `README.md` for usage and `CLAUDE.md` for the design.
//!
//! All logic lives in this library crate; `src/main.rs` is a thin binary over
//! it. Dependencies point inward toward [`domain`], which is pure data with no
//! I/O. The layers [`storage`], [`selection`], and [`render`] are the
//! trait-backed extension points; [`app`] wires them together; [`cli`] adapts
//! command-line input to the app.

pub mod app;
pub mod cli;
pub mod config;
pub mod domain;
mod fs_atomic;
pub mod render;
pub mod selection;
pub mod storage;
pub mod vaults;
