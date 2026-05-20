use crate::manifest::MANIFEST_JSON;

static mut BUMP: u32 = 4096;

#[no_mangle]
static mut WASM_HEAP: [u8; 131072] = [0; 131072];

#[no_mangle]
pub extern "C" fn turso_malloc(size: i32) -> i32 {
    unsafe {
        let ptr = BUMP;
        BUMP = BUMP.saturating_add(size as u32);
        ptr as i32
    }
}

#[no_mangle]
pub extern "C" fn turso_ext_init(_argc: i32, _argv: i32) -> i64 {
    return_text_tag(MANIFEST_JSON)
}

/// Return TAG_TEXT pointer (tag byte + len + bytes + NUL).
pub fn return_text_tag(text: &str) -> i64 {
    let bytes = text.as_bytes();
    let size = 1 + 4 + bytes.len() + 1;
    let ptr = turso_malloc(size as i32) as usize;
    unsafe {
        core::ptr::write(ptr as *mut u8, 3);
        core::ptr::write((ptr + 1) as *mut u32, bytes.len() as u32);
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), (ptr + 5) as *mut u8, bytes.len());
        core::ptr::write((ptr + 5 + bytes.len()) as *mut u8, 0);
    }
    ptr as i64
}

/// Return TAG_INT pointer.
pub fn return_i64_tag(value: i64) -> i64 {
    let ptr = turso_malloc(9) as usize;
    unsafe {
        core::ptr::write(ptr as *mut u8, 1);
        core::ptr::write((ptr + 1) as *mut i64, value);
    }
    ptr as i64
}
