#![cfg(target_os = "linux")]

use std::ops::{Deref, DerefMut};

pub fn aligned_alloc(size: usize, page_size: usize) -> Vec<u8> {
    use std::ptr;
    let mut ptr: *mut u8 = ptr::null_mut();
    unsafe {
        let ret = libc::posix_memalign(&mut ptr as *mut _ as *mut _, page_size, size);
        if ret != 0 {
            panic!("posix_memalign failed");
        }
        Vec::from_raw_parts(ptr, size, size)
    }
}

pub struct AlignedVecU8 {
    vec: Vec<u8>,
}

impl AlignedVecU8 {
    pub fn new(buf_size: usize, page_size: usize) -> Self {
        let vec = aligned_alloc(buf_size, page_size);
        Self { vec }
    }
}

impl Deref for AlignedVecU8 {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl DerefMut for AlignedVecU8 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}

pub struct ReaderBuffer {
    pub data: AlignedVecU8,
    pub len: usize,
    pub cap: usize
}

impl ReaderBuffer {
    pub fn new(buf_size: usize, page_size: usize) -> Self {
        let data = AlignedVecU8::new(buf_size, page_size);
        Self { data, len: 0,cap: buf_size }
    }
    pub fn cap(&self) -> usize {
        self.cap
    }
    pub fn len(&self) -> usize {
        self.len
    }
}

impl Deref for ReaderBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for ReaderBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
