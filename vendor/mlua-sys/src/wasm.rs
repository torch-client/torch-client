use std::alloc::{self, Layout};
use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::OnceLock;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
const LONGJMP = { mluaSysLongjmp: true };

export function mlua_sys_throw_longjmp() {
    throw LONGJMP;
}

export function mlua_sys_invoke_vii(table, index, a, b) {
    try {
        table.get(index)(a, b);
        return true;
    } catch (e) {
        if (e !== LONGJMP) {
            throw e;
        }
        return false;
    }
}

export function mlua_sys_now_ms() {
    return performance.now();
}

export function mlua_sys_log(text) {
    console.log(text);
}
"#)]
unsafe extern "C" {
    fn mlua_sys_throw_longjmp();
    fn mlua_sys_invoke_vii(table: &wasm_bindgen::JsValue, index: u32, a: i32, b: i32) -> bool;
    fn mlua_sys_now_ms() -> f64;
    fn mlua_sys_log(text: &str);
}

unsafe extern "C" {
    fn mlua_shim_stack_save() -> usize;
    fn mlua_shim_stack_restore(sp: usize);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn invoke_vii(index: u32, a: i32, b: i32) {
    let sp = unsafe { mlua_shim_stack_save() };
    if !mlua_sys_invoke_vii(&wasm_bindgen::function_table(), index, a, b) {
        unsafe { mlua_shim_stack_restore(sp) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _emscripten_throw_longjmp() -> ! {
    mlua_sys_throw_longjmp();
    std::process::abort()
}

const HEADER: usize = 16;

fn block_layout(size: usize) -> Option<Layout> {
    Layout::from_size_align(size.checked_add(HEADER)?, HEADER).ok()
}

unsafe fn allocate(size: usize, zeroed: bool) -> *mut c_void {
    let Some(layout) = block_layout(size) else {
        return ptr::null_mut();
    };
    let base = unsafe {
        if zeroed {
            alloc::alloc_zeroed(layout)
        } else {
            alloc::alloc(layout)
        }
    };
    if base.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        base.cast::<usize>().write(size);
        base.add(HEADER).cast()
    }
}

unsafe fn block_of(p: *mut c_void) -> (*mut u8, Layout) {
    let base = unsafe { p.cast::<u8>().sub(HEADER) };
    let size = unsafe { base.cast::<usize>().read() };
    let layout = block_layout(size).unwrap();
    (base, layout)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { allocate(size, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    match count.checked_mul(size) {
        Some(total) => unsafe { allocate(total, true) },
        None => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return unsafe { allocate(size, false) };
    }
    if block_layout(size).is_none() {
        return ptr::null_mut();
    }
    let (base, layout) = unsafe { block_of(p) };
    let base = unsafe { alloc::realloc(base, layout, size + HEADER) };
    if base.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        base.cast::<usize>().write(size);
        base.add(HEADER).cast()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    let (base, layout) = unsafe { block_of(p) };
    unsafe { alloc::dealloc(base, layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    std::process::abort()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn time(out: *mut i64) -> i64 {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if !out.is_null() {
        unsafe { out.write(now) };
    }
    now
}

#[unsafe(no_mangle)]
pub extern "C" fn clock() -> i64 {
    static START: OnceLock<f64> = OnceLock::new();
    let now = mlua_sys_now_ms();
    let start = *START.get_or_init(|| now);
    ((now - start) * 1000.0) as i64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlua_shim_log(text: *const c_char, len: usize) {
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len) };
    mlua_sys_log(&String::from_utf8_lossy(bytes));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlua_shim_parse_f64(text: *const c_char, len: usize) -> f64 {
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len) };
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

const FLAG_LEFT: u32 = 1;
const FLAG_PLUS: u32 = 2;
const FLAG_SPACE: u32 = 4;
const FLAG_ALT: u32 = 8;
const FLAG_ZERO: u32 = 16;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlua_shim_format_f64(
    buf: *mut u8,
    cap: usize,
    conv: u8,
    flags: u32,
    width: c_int,
    prec: c_int,
    value: f64,
) -> c_int {
    let text = format_float(conv, flags, width, prec, value);
    if !buf.is_null() {
        let n = text.len().min(cap);
        unsafe { ptr::copy_nonoverlapping(text.as_ptr(), buf, n) };
    }
    c_int::try_from(text.len()).unwrap_or(c_int::MAX)
}

fn format_float(conv: u8, flags: u32, width: c_int, prec: c_int, value: f64) -> String {
    let upper = conv.is_ascii_uppercase();
    let alt = flags & FLAG_ALT != 0;
    let prec = usize::try_from(prec).ok();
    let abs = value.abs();

    let sign = if value.is_sign_negative() {
        "-"
    } else if flags & FLAG_PLUS != 0 {
        "+"
    } else if flags & FLAG_SPACE != 0 {
        " "
    } else {
        ""
    };

    let (prefix, mut body) = if abs.is_nan() {
        ("", String::from("nan"))
    } else if abs.is_infinite() {
        ("", String::from("inf"))
    } else {
        match conv.to_ascii_lowercase() {
            b'f' => ("", fixed(abs, prec.unwrap_or(6), alt)),
            b'e' => ("", exponential(abs, prec.unwrap_or(6), alt)),
            b'g' => ("", general(abs, prec, alt)),
            _ => (if upper { "0X" } else { "0x" }, hex(abs, prec, alt)),
        }
    };
    if upper {
        body.make_ascii_uppercase();
    }

    let len = sign.len() + prefix.len() + body.len();
    let fill = usize::try_from(width).unwrap_or(0).saturating_sub(len);
    let mut out = String::with_capacity(len + fill);
    if flags & FLAG_LEFT != 0 {
        out.push_str(sign);
        out.push_str(prefix);
        out.push_str(&body);
        out.extend(std::iter::repeat_n(' ', fill));
    } else if flags & FLAG_ZERO != 0 && abs.is_finite() {
        out.push_str(sign);
        out.push_str(prefix);
        out.extend(std::iter::repeat_n('0', fill));
        out.push_str(&body);
    } else {
        out.extend(std::iter::repeat_n(' ', fill));
        out.push_str(sign);
        out.push_str(prefix);
        out.push_str(&body);
    }
    out
}

fn fixed(abs: f64, prec: usize, alt: bool) -> String {
    let mut s = format!("{abs:.prec$}");
    if alt && prec == 0 {
        s.push('.');
    }
    s
}

fn exponential(abs: f64, prec: usize, alt: bool) -> String {
    let formatted = format!("{abs:.prec$e}");
    let (mantissa, exponent) = split_exponent(&formatted);
    let point = if alt && prec == 0 { "." } else { "" };
    let exp_sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}{point}e{exp_sign}{:02}", exponent.unsigned_abs())
}

fn split_exponent(s: &str) -> (&str, i32) {
    match s.split_once('e') {
        Some((mantissa, exponent)) => (mantissa, exponent.parse().unwrap_or(0)),
        None => (s, 0),
    }
}

fn general(abs: f64, prec: Option<usize>, alt: bool) -> String {
    let p = match prec {
        None => 6,
        Some(0) => 1,
        Some(p) => p,
    };
    let decimals = p - 1;
    let x = if abs == 0.0 {
        0
    } else {
        split_exponent(&format!("{abs:.decimals$e}")).1
    };
    let p_i64 = i64::try_from(p).unwrap_or(i64::MAX);
    let x_i64 = i64::from(x);

    let mut s = if x_i64 < -4 || x_i64 >= p_i64 {
        exponential(abs, decimals, alt)
    } else {
        fixed(abs, usize::try_from(p_i64 - 1 - x_i64).unwrap_or(0), alt)
    };

    if !alt {
        let end = s.find('e').unwrap_or(s.len());
        let (mantissa, rest) = s.split_at(end);
        if mantissa.contains('.') {
            let trimmed = mantissa.trim_end_matches('0').trim_end_matches('.');
            s = format!("{trimmed}{rest}");
        }
    }
    s
}

fn hex(abs: f64, prec: Option<usize>, alt: bool) -> String {
    const FRACTION_DIGITS: usize = 13;
    let bits = abs.to_bits();
    let biased = (bits >> 52) & 0x7ff;
    let mut fraction = bits & ((1u64 << 52) - 1);
    let (mut lead, exponent) = match (biased, fraction) {
        (0, 0) => (0u64, 0i64),
        (0, _) => (0, -1022),
        _ => (1, biased as i64 - 1023),
    };

    let mut digits = match prec {
        Some(p) if p < FRACTION_DIGITS => {
            let shift = (FRACTION_DIGITS - p) * 4;
            let full = (lead << 52) | fraction;
            let mut kept = full >> shift;
            let rest = full & ((1u64 << shift) - 1);
            let half = 1u64 << (shift - 1);
            if rest > half || (rest == half && kept & 1 == 1) {
                kept += 1;
            }
            lead = kept >> (p * 4);
            fraction = kept & ((1u64 << (p * 4)) - 1);
            if p == 0 {
                String::new()
            } else {
                format!("{fraction:0p$x}")
            }
        }
        _ => format!("{fraction:013x}"),
    };
    match prec {
        None => {
            let trimmed = digits.trim_end_matches('0').len();
            digits.truncate(trimmed);
        }
        Some(p) if p > FRACTION_DIGITS => digits.extend(std::iter::repeat_n('0', p - FRACTION_DIGITS)),
        Some(_) => {}
    }

    let point = if !digits.is_empty() || alt { "." } else { "" };
    format!("{lead}{point}{digits}p{exponent:+}")
}
