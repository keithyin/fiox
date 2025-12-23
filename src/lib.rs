pub mod buffer_aux;
pub mod linux;
pub mod utils;
pub mod windows;

#[cfg(windows)]
pub use windows::sequential_reader::SequentialReader;

#[cfg(windows)]
pub use windows::sequential_writer::SequentialWriter;


#[cfg(target_os = "linux")]
pub use linux::sequential_reader::SequentialReader;

#[cfg(target_os = "linux")]
pub use linux::sequential_writer::SequentialWriter;

#[cfg(test)]
mod test {
    use std::{
        fs,
        io::{Read, Seek},
    };

    use crate::SequentialWriter;

    use super::SequentialReader;

    #[test]
    fn test_sequential_reader() {
        let read_start_pos = 10;
        let mut reader =
            SequentialReader::new("test_data/test_data.txt", read_start_pos, 4096, 2, None).unwrap();
        let mut reader2 = fs::File::open("test_data/test_data.txt").unwrap();
        reader2
            .seek(std::io::SeekFrom::Start(read_start_pos))
            .unwrap();
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

    #[test]
    fn test_sequential_writer() {
        let mut writer =
            SequentialWriter::new("test_data/test_data_writer.txt", 0, 4096, 2).unwrap();
        for i in 0..1000 {
            writer
                .write(format!("line:{}, abcdefghijklmnopqrstuvwxyz\n", i).as_bytes())
                .unwrap();
        }
    }
}
