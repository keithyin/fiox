#[cfg(windows)]
pub fn str_to_wide(path: &str) -> Vec<u16> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

    let mut v: Vec<u16> = OsStr::new(path).encode_wide().collect();
    v.push(0);
    v
}