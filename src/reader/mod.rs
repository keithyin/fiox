pub mod sequential_reader_windows;
pub mod buffer_windows;
pub mod utils;


#[cfg(windows)]
pub use sequential_reader_windows::SequentialReader;

