use fiox::SequentialWriter;

fn main() {
    let fpath = "E:/datas/fiox_write.bin";
    let mut writer = SequentialWriter::new(fpath, 0, 1024 * 1024, 8).unwrap();

    let data = b"1234567890abcdefghijklmnopqrstuvwxyz\n";
    for _ in 0..(1024 * 1024 * 1024 * 100 / data.len()) {
        writer.write(data).unwrap();
    }
}
