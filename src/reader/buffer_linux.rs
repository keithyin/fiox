#![cfg(linux)]

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

pub struct ReaderBuffer {
    data: AlignedVecU8,
}

impl ReaderBuffer {
    pub fn new(buf_size: usize, page_size: usize) -> Self {
        let vec = AlignedVecU8::new(buf_size, page_size);
        Self {
            vec,
            cap: buf_size,
            size: 0,
        }
    }
}
