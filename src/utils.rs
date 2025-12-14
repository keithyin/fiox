pub fn get_file_size(fpath: &str) -> u64 {
    let metadata = std::fs::metadata(fpath).unwrap();
    metadata.len()
}