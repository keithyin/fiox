pub mod buffer_linux;
pub mod buffer_windows;
pub mod sequential_reader_linux;
pub mod sequential_reader_windows;
pub mod utils;

#[cfg(windows)]
pub use sequential_reader_windows::SequentialReader;

#[cfg(target_os = "linux")]
pub use sequential_reader_linux::SequentialReader;
