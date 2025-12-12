pub mod buffer_linux;
pub mod buffer_windows;
pub mod sequential_reader_linux;
pub mod sequential_reader_windows;
pub mod utils;

#[cfg(windows)]
pub use sequential_reader_windows::SequentialReader;

#[cfg(target_os = "linux")]
pub use sequential_reader_linux::SequentialReader;

#[cfg(test)]
mod test {
    use super::SequentialReader;

    #[test]
    fn test_sequential_reader() {
        let mut reader = SequentialReader::new("test_data/test_data.txt", 0, 4096, 2).unwrap();
        let mut buf = vec![0_u8; 112560];
        loop {
            let n = reader.read2buf(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            print!("{}", String::from_utf8((&buf[..n]).to_vec()).unwrap());
        }
        println!("")
    }
}
