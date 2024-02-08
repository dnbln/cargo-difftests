// tests/tests.rs

use {{crate_name}}::*;

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}

#[test]
fn test_sub() {
    assert_eq!(sub(3, 2), 1);
}

#[test]
fn test_mul() {
    assert_eq!(mul(2, 3), 6);
}

#[test]
fn test_div() {
    assert_eq!(div(6, 3), Some(2));
}

#[test]
fn test_div_2() {
    assert_eq!(div(6, 0), None);
}
