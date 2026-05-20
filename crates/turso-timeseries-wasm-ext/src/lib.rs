//! Raw Turso WASM extension (`CREATE EXTENSION ... LANGUAGE wasm`).
#![no_std]

extern crate alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod exports;
mod manifest;
mod panic;
mod scalar;

pub use exports::turso_ext_init;
pub use scalar::{time_bucket, tts_parse_duration_micros, tts_version};
