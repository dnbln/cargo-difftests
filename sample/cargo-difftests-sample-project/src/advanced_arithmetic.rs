// src/advanced_arithmetic.rs

pub fn mul(a: i32, b: i32) -> i32 {
    a * b
}

pub fn div_unchecked(a: i32, b: i32) -> i32 {
    a / b
}

pub fn div(a: i32, b: i32) -> Option<i32> {
    if b != 0 {
        Some(div_unchecked(a, b))
    } else {
        None
        // dummy comment
    }
}
