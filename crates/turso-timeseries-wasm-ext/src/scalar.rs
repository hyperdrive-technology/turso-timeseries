use crate::exports::{return_i64_tag, return_text_tag};

/// Scalar: extension version string.
#[no_mangle]
pub extern "C" fn tts_version(_argc: i32, _argv: i32) -> i64 {
    return_text_tag("turso-timeseries 0.1.0")
}

/// Scalar: `time_bucket(width, ts_micros)`.
#[no_mangle]
pub extern "C" fn time_bucket(argc: i32, argv: i32) -> i64 {
    if argc < 2 {
        return 0;
    }
    let width_ptr = unsafe { *(argv as *const i32) } as usize;
    let ts_ptr = unsafe { *((argv + 4) as *const i32) } as usize;
    let width = unsafe { read_tag_text(width_ptr) };
    let ts = unsafe { read_tag_i64(ts_ptr) };
    match time_bucket_micros(&width, ts) {
        Some(bucket) => return_i64_tag(bucket),
        None => 0,
    }
}

/// Scalar: parse duration width to microseconds.
#[no_mangle]
pub extern "C" fn tts_parse_duration_micros(argc: i32, argv: i32) -> i64 {
    if argc < 1 {
        return 0;
    }
    let ptr = unsafe { *(argv as *const i32) } as usize;
    let width = unsafe { read_tag_text(ptr) };
    match parse_duration_micros(&width) {
        Some(v) => return_i64_tag(v),
        None => 0,
    }
}

fn time_bucket_micros(width: &str, ts_micros: i64) -> Option<i64> {
    let width_micros = parse_duration_micros(width)?;
    if width_micros <= 0 {
        return None;
    }
    Some(ts_micros - ts_micros.rem_euclid(width_micros))
}

fn parse_duration_micros(width: &str) -> Option<i64> {
    let width = width.trim();
    if width.is_empty() {
        return None;
    }
    let split = width
        .chars()
        .position(|c| !c.is_ascii_digit())
        .unwrap_or(width.len());
    if split == 0 {
        return None;
    }
    let amount: i64 = width[..split].parse().ok()?;
    if amount <= 0 {
        return None;
    }
    let unit = &width[split..];
    let micros_per_unit: i64 = match unit {
        "us" | "µs" => 1,
        "ms" => 1_000,
        "s" => 1_000_000,
        "m" => 60 * 1_000_000,
        "h" => 60 * 60 * 1_000_000,
        "d" => 24 * 60 * 60 * 1_000_000,
        "w" => 7 * 24 * 60 * 60 * 1_000_000,
        _ => return None,
    };
    amount.checked_mul(micros_per_unit)
}

/// Minimal tagged value readers for Turso WASM ABI (TAG_TEXT=3, TAG_INT=1).
unsafe fn read_tag_i64(ptr: usize) -> i64 {
    if ptr == 0 {
        return 0;
    }
    let tag = core::ptr::read(ptr as *const u8);
    if tag != 1 {
        return 0;
    }
    core::ptr::read((ptr + 1) as *const i64)
}

unsafe fn read_tag_text(ptr: usize) -> alloc::string::String {
    if ptr == 0 {
        return alloc::string::String::new();
    }
    let tag = core::ptr::read(ptr as *const u8);
    if tag != 3 {
        return alloc::string::String::new();
    }
    let len = core::ptr::read((ptr + 1) as *const u32) as usize;
    let bytes = core::slice::from_raw_parts((ptr + 5) as *const u8, len);
    alloc::string::String::from_utf8_lossy(bytes).into_owned()
}
