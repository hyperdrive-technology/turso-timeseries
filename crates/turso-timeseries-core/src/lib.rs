//! Portable time-series core: models, codecs, ingest, query, and maintenance.
//!
//! This crate has no Turso dependency and is intended to work with `no_std + alloc`
//! when the `alloc` feature is enabled without `std`.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod ingest;
pub mod maintenance;
pub mod model;
pub mod query;
pub mod storage;

pub use error::{Error, Result};
