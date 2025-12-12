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
    use std::{fs, io::{Read, Seek}};

    use super::SequentialReader;

    #[test]
    fn test_sequential_reader() {
        let read_start_pos = 10;
        let mut reader = SequentialReader::new("test_data/test_data.txt", read_start_pos, 4096, 2).unwrap();
        let mut reader2 = fs::File::open("test_data/test_data.txt").unwrap();
        reader2.seek(std::io::SeekFrom::Start(read_start_pos)).unwrap();
        let file_size = fs::metadata("test_data/test_data.txt").unwrap().len();
        
        let buf_size = 112560;
        let mut buf = vec![0_u8; buf_size];
        let mut buf2 = vec![0_u8; buf_size];
        let mut read_size = 0;
        loop {
            let n = reader.read2buf(&mut buf).unwrap();
            reader2.read_exact(&mut buf2[..n]).unwrap();
            assert_eq!(&buf[..n], &buf2[..n]);
            read_size += n as u64;
            if n == 0 {
                break;
            }
            // print!("{}", String::from_utf8((&buf[..n]).to_vec()).unwrap());
        }
        assert_eq!(read_size, file_size - read_start_pos);
        println!("");
    }
}
