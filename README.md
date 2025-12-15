
# fiox

> **Zero-Copy, Asynchronous File I/O for Rust — powered by io_uring on Linux and IOCP on Windows, with built-in ring buffer support.**


fiox delivers **ultra-low-latency, high-throughput file I/O** by leveraging modern kernel interfaces:
- **Linux**: Direct `io_uring` integration (bypassing traditional syscall overhead)
- **Windows**: Native `I/O Completion Ports` (IOCP) for scalable async I/O

Paired with efficient **lock-free ring buffers**, RingIO enables seamless zero-copy data pipelines — ideal for databases, log processors, and real-time analytics.

---

## ✨ Features

- ✅ **Cross-platform**: Linux (io_uring) & Windows (IOCP)  
- ✅ **Zero-copy reads/writes** using direct io  
- ✅ Built-in **SPSC ring buffers** for high-speed data staging  
- ✅ Batching & coalescing for reduced syscalls  



### SequentialReader

``` rust

let read_start_pos = 10;
let mut reader =
    SequentialReader::new("test_data/test_data.txt", read_start_pos, 4096, 2).unwrap();


let buf_size = 112560;
let mut buf = vec![0_u8; buf_size];
let mut read_size = 0;
loop {
    let n = reader.read2buf(&mut buf).unwrap();
    if n == 0 {
        break;
    }
    print!("{}", String::from_utf8((&buf[..n]).to_vec()).unwrap());
}
```

### SequentialWriter

```rust
#[test]
    
let mut writer =
    SequentialWriter::new("test_data/test_data_writer.txt", 0, 4096, 2).unwrap();
for i in 0..1000 {
    writer
        .write(format!("line:{}, abcdefghijklmnopqrstuvwxyz\n", i).as_bytes())
        .unwrap();
}
```