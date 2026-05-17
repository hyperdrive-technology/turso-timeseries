//! wasm-bindgen surface for browser-side time-series helpers.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn tts_extension_loaded() -> i32 {
    1
}

#[wasm_bindgen]
pub fn tts_time_bucket_ns(ts_ns: i64, width_ns: i64) -> Result<i64, JsValue> {
    turso_timeseries::time_bucket_ns(ts_ns, width_ns).map_err(|e| JsValue::from_str(&e.to_string()))
}
