use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

/// convert Rust &str path to wide null-terminated Vec<u16>
#[cfg(windows)]
pub fn str_to_wide(path: &str) -> Vec<u16> {
    let mut v: Vec<u16> = OsStr::new(path).encode_wide().collect();
    v.push(0);
    v
}

pub fn get_file_size(fpath: &str) -> u64 {
    let metadata = std::fs::metadata(fpath).unwrap();
    metadata.len()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ReaderDataPos {
    pub buf_idx: usize,
    pub offset: usize,
}


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReaderBufferStatus {
    #[default]
    Ready4Submit,
    Ready4Read,
    Invalid,
}
