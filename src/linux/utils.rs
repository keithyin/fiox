/// convert Rust &str path to wide null-terminated Vec<u16>

pub const fn get_page_size() -> usize {
    // unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
    4096
}
