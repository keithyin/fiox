#![cfg(windows)]
#![allow(non_snake_case)]
use std::io::{Seek, Write};
use windows_sys::Win32::Storage::FileSystem::WriteFile;
use windows_sys::Win32::System::IO::{GetQueuedCompletionStatus, OVERLAPPED};
use windows_sys::Win32::System::Threading::INFINITE;

use super::buffer::ReaderBuffer;
use crate::buffer_aux::{BufferDataPos, BufferStatus};
use crate::windows::handles::{FileHandle, IocpHandle};

pub struct SequentialWriter {
    fpath: String,
    handle: FileHandle,
    buffers: Vec<super::buffer::ReaderBuffer>,
    buffers_status: Vec<BufferStatus>,
    buffer_size: usize,
    data_pos: BufferDataPos,
    file_pos_cursor: u64,
    pendding: usize,

    iocp: IocpHandle,
}

impl SequentialWriter {
    pub fn new(
        fpath: &str,
        start_pos: u64,
        buffer_size: usize,
        num_buffer: usize,
    ) -> anyhow::Result<Self> {
        assert!(buffer_size % 4096 == 0);
        assert!(start_pos as usize % 4096 == 0);

        let handle = FileHandle::new(fpath, crate::windows::handles::FileMode::Write)?;

        let mut iocp = IocpHandle::new()?;
        iocp.init(handle.handle)?;

        let data_pose = BufferDataPos {
            buf_idx: 0,
            offset: (start_pos % 4096) as usize,
        };
        let file_pos_cursor = start_pos - data_pose.offset as u64;

        // println!("file_pos_cursor:{}", file_pos_cursor);

        let buffers = (0..num_buffer)
            .into_iter()
            .map(|idx| ReaderBuffer::new(buffer_size, idx))
            .collect();
        Ok(Self {
            fpath: fpath.to_string(),
            handle: handle,
            buffers: buffers,
            buffers_status: vec![BufferStatus::Ready4Process; num_buffer],
            buffer_size: buffer_size,
            data_pos: data_pose,
            file_pos_cursor: file_pos_cursor,
            pendding: 0,
            iocp,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let req_len = data.len();
        let mut remaining_bytes = req_len;
        let mut write_pos = 0;

        while remaining_bytes > 0 {
            self.wait_inner_buf_ready()?;

            let cur_buf_write_n = remaining_bytes.min(self.buffer_size - self.data_pos.offset);

            unsafe {
                std::ptr::copy(
                    data.as_ptr().add(write_pos),
                    self.buffers[self.data_pos.buf_idx]
                        .data
                        .add(self.data_pos.offset),
                    cur_buf_write_n,
                );
            }

            self.data_pos.offset += cur_buf_write_n;
            write_pos += cur_buf_write_n;
            remaining_bytes -= cur_buf_write_n;

            if self.data_pos.offset == self.buffer_size {
                self.buffers_status[self.data_pos.buf_idx] = BufferStatus::Ready4Submit;
                self.submit_write_event(self.data_pos.buf_idx);

                self.data_pos.buf_idx += 1;
                self.data_pos.buf_idx %= self.buffers.len();
                self.data_pos.offset = 0;
            }
        }

        return Ok(());
    }

    fn wait_inner_buf_ready(&mut self) -> anyhow::Result<()> {
        if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Ready4Process {
            return Ok(());
        }

        while self.pendding > 0 {
            let mut bytes_transferred: u32 = 0;
            let mut completion_key: usize = 0;
            let mut pov: *mut OVERLAPPED = std::ptr::null_mut();
            let _ok = unsafe {
                GetQueuedCompletionStatus(
                    self.iocp.handle,
                    &mut bytes_transferred as *mut u32,
                    &mut completion_key as *mut usize,
                    &mut pov as *mut *mut OVERLAPPED,
                    INFINITE,
                )
            };

            if pov == std::ptr::null_mut() {
                panic!("pov is null");
            }

            let task: *mut ReaderBuffer = pov as *mut ReaderBuffer;
            unsafe {
                (*task).len = bytes_transferred as usize;
            }

            assert_eq!(bytes_transferred, self.buffer_size as u32);
            let idx = unsafe { (*task).idx };
            self.buffers_status[idx] = BufferStatus::Ready4Process;
            self.pendding -= 1;

            // println!("wait_inner_buf_ready:{}", self.pendding);

            if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Ready4Process {
                return Ok(());
            }
        }

        anyhow::bail!("buf_idx={} request failed", self.data_pos.buf_idx)
    }

    fn submit_write_event(&mut self, buf_idx: usize) {
        let lo = (self.file_pos_cursor & 0xFFFF_FFFF) as u32;
        let hi = (self.file_pos_cursor >> 32) as u32;

        // self.buffers[buf_idx].overlapped.Pointer = std::ptr::null_mut(); // not used
        unsafe {
            std::ptr::write_bytes(
                &mut self.buffers[buf_idx].overlapped as *mut OVERLAPPED,
                0,
                1,
            );
        }

        self.buffers[buf_idx].overlapped.Anonymous.Pointer = std::ptr::null_mut();

        self.buffers[buf_idx].overlapped.Anonymous.Anonymous.Offset = lo;

        self.buffers[buf_idx]
            .overlapped
            .Anonymous
            .Anonymous
            .OffsetHigh = hi;

        self.buffers[buf_idx].overlapped.Internal = 0;
        self.buffers[buf_idx].overlapped.InternalHigh = 0;

        self.buffers[buf_idx].offset = self.file_pos_cursor;
        self.buffers[buf_idx].len = 0;

        let _ok = unsafe {
            WriteFile(
                self.handle.handle,
                self.buffers[buf_idx].data as *mut _,
                self.buffer_size as u32,
                std::ptr::null_mut(), // lpNumberOfBytesRead = NULL for async
                &mut self.buffers[buf_idx].overlapped as *mut _,
            )
        };

        // if _ok == 0 {
        //     println!("Last Error Code : {}", unsafe { GetLastError() });
        //     panic!("Write error");
        // }

        self.pendding += 1;
        // println!("submit_write_event:{}", self.pendding);

        self.file_pos_cursor += self.buffer_size as u64;
    }
}

impl Drop for SequentialWriter {
    fn drop(&mut self) {
        while self.pendding > 0 {
            let mut bytes_transferred: u32 = 0;
            let mut completion_key: usize = 0;
            let mut pov: *mut OVERLAPPED = std::ptr::null_mut();
            let _ok = unsafe {
                GetQueuedCompletionStatus(
                    self.iocp.handle,
                    &mut bytes_transferred as *mut u32,
                    &mut completion_key as *mut usize,
                    &mut pov as *mut *mut OVERLAPPED,
                    INFINITE,
                )
            };

            if pov == std::ptr::null_mut() {
                panic!("pov is null");
            }

            let task: *mut ReaderBuffer = pov as *mut ReaderBuffer;
            unsafe {
                (*task).len = bytes_transferred as usize;
            }

            assert_eq!(bytes_transferred, self.buffer_size as u32);
            let idx = unsafe { (*task).idx };
            self.buffers_status[idx] = BufferStatus::Ready4Process;
            self.pendding -= 1;
        }

        if self.data_pos.offset > 0 {
            let mut f = std::fs::OpenOptions::new()
                .read(true) // 可读
                .write(true) // 可写
                .create(true) // 不存在则创建
                .open(&self.fpath)
                .unwrap();
            f.seek(std::io::SeekFrom::Start(self.file_pos_cursor as u64))
                .unwrap();
            f.write_all(unsafe {
                std::slice::from_raw_parts(
                    self.buffers[self.data_pos.buf_idx].data,
                    self.data_pos.offset,
                )
            })
            .unwrap();
        }
    }
}
