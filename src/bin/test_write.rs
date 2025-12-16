use fiox::SequentialWriter;

fn main() {

    let args = std::env::args().collect::<Vec<String>>();
    assert!(args.len() > 2);
    let fpath = args[1].trim();
    let n_gbytes = args[2].parse::<usize>().unwrap();

    // let fpath = "E:/datas/fiox_write.bin";
    let mut writer = SequentialWriter::new(fpath, 0, 1024 * 1024, 8).unwrap();

    let data = b"1234567890abcdefghijklmnopqrstuvwxyz\n";
    for _ in 0..(1024 * 1024 * 1024 * n_gbytes / data.len()) {
        writer.write(data).unwrap();
    }
}
