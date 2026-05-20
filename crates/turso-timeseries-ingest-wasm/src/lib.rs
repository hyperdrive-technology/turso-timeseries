//! Lightweight ingest helper for browser workers — not the Turso extension ABI.

use wasm_bindgen::prelude::*;
use turso_timeseries_ingest_core::{FlushPolicy, IngestBuffer};

#[wasm_bindgen]
pub struct WasmIngestBuffer {
    inner: IngestBuffer,
}

#[wasm_bindgen]
impl WasmIngestBuffer {
    #[wasm_bindgen(constructor)]
    pub fn new(max_points: usize, max_bytes: usize, max_age_ms: u64) -> Self {
        Self {
            inner: IngestBuffer::new(FlushPolicy {
                max_points,
                max_bytes,
                max_age_ms,
            }),
        }
    }

    /// Push a UTF-8 frame; returns point count sealed when flush triggers, else 0.
    pub fn push_frame(&mut self, frame: &str, now_ms: u64) -> Result<u32, JsValue> {
        match self.inner.push_lp_text(frame, now_ms) {
            Ok(Some(points)) => Ok(points.len() as u32),
            Ok(None) => Ok(0),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }
}
