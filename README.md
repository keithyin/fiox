
# fiox

> **Zero-Copy, Asynchronous File I/O for Rust — powered by io_uring on Linux and IOCP on Windows, with built-in ring buffer support.**


RingIO delivers **ultra-low-latency, high-throughput file I/O** by leveraging modern kernel interfaces:
- **Linux**: Direct `io_uring` integration (bypassing traditional syscall overhead)
- **Windows**: Native `I/O Completion Ports` (IOCP) for scalable async I/O

Paired with efficient **lock-free ring buffers**, RingIO enables seamless zero-copy data pipelines — ideal for databases, log processors, and real-time analytics.

---

## ✨ Features

- ✅ **Cross-platform**: Linux (io_uring) & Windows (IOCP)  
- ✅ **Zero-copy reads/writes** using direct io  
- ✅ Built-in **SPSC ring buffers** for high-speed data staging  
- ✅ Batching & coalescing for reduced syscalls  
