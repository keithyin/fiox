#![cfg(windows)]
#![allow(non_snake_case)]
use std::io::{Read, Seek};

use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::IO::{GetQueuedCompletionStatus, OVERLAPPED};
use windows_sys::Win32::System::Threading::INFINITE;

use super::buffer::ReaderBuffer;
use crate::buffer_aux::{BufferDataPos, BufferStatus};
use crate::utils::get_file_size;
use crate::windows::handles::{FileHandle, FileMode, IocpHandle};

pub struct SequentialReader {
    fpath: String,
    handle: FileHandle,
    buffers: Vec<super::buffer::ReaderBuffer>,
    buffers_status: Vec<BufferStatus>,
    buffer_size: usize,
    data_pos: BufferDataPos,
    file_pos_cursor: u64,
    end_pos: u64,
    init_flag: bool,
    pendding: usize,

    iocp: IocpHandle,
}
unsafe impl Send for SequentialReader {}

impl SequentialReader {
    pub fn new(
        fpath: &str,
        start_pos: u64,
        buffer_size: usize,
        num_buffer: usize,
        end_pos: Option<u64>,
    ) -> anyhow::Result<Self> {
        assert!(buffer_size % 4096 == 0);

        let file_size = get_file_size(fpath);
        let end_pos = match end_pos {
            Some(pos) => pos,
            None => file_size,
        };
        if end_pos > file_size {
            anyhow::bail!("end_pos {} is larger than file size {}", end_pos, file_size);
        }

        let handle = FileHandle::new(fpath, FileMode::Read)?;

        let mut iocp = IocpHandle::new()?;
        iocp.init(handle.handle)?;

        let data_pose = BufferDataPos {
            buf_idx: 0,
            offset: (start_pos % 4096) as usize,
        };
        let file_pos_cursor = start_pos - data_pose.offset as u64;

        let buffers = (0..num_buffer)
            .into_iter()
            .map(|idx| ReaderBuffer::new(buffer_size, idx))
            .collect();

        Ok(Self {
            fpath: fpath.to_string(),
            handle: handle,
            buffers: buffers,
            buffers_status: vec![BufferStatus::default(); num_buffer],
            buffer_size: buffer_size,
            data_pos: data_pose,
            file_pos_cursor: file_pos_cursor,
            end_pos: end_pos,
            init_flag: false,
            pendding: 0,
            iocp,
        })
    }

    pub fn read2buf(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let req_len = buf.len();
        let mut remaining_bytes = req_len;
        let mut fill_pos = 0;

        while remaining_bytes > 0 {
            self.wait_inner_buf_ready()?;
            if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Invalid {
                return Ok(fill_pos);
            }

            let cur_buf_read_n =
                remaining_bytes.min(self.buffers[self.data_pos.buf_idx].len - self.data_pos.offset);

            unsafe {
                std::ptr::copy(
                    self.buffers[self.data_pos.buf_idx]
                        .data
                        .add(self.data_pos.offset),
                    buf.as_mut_ptr().add(fill_pos),
                    cur_buf_read_n,
                );
            }

            self.data_pos.offset += cur_buf_read_n;
            fill_pos += cur_buf_read_n;
            remaining_bytes -= cur_buf_read_n;

            if self.data_pos.offset == self.buffers[self.data_pos.buf_idx].len {
                self.buffers_status[self.data_pos.buf_idx] = BufferStatus::Ready4Submit;
                self.submit_read_event(self.data_pos.buf_idx);

                self.data_pos.buf_idx += 1;
                self.data_pos.buf_idx %= self.buffers.len();
                self.data_pos.offset = 0;
            }
        }

        return Ok(req_len);
    }

    fn wait_inner_buf_ready(&mut self) -> anyhow::Result<()> {
        if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Ready4Process {
            return Ok(());
        }

        if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Invalid {
            return Ok(());
        }

        if !self.init_flag {
            for buf_idx in 0..self.buffers.len() {
                self.submit_read_event(buf_idx);
            }
            self.init_flag = true;
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
            if self.buffers_status[self.data_pos.buf_idx] == BufferStatus::Ready4Process {
                return Ok(());
            }
        }

        anyhow::bail!("buf_idx={} request failed", self.data_pos.buf_idx)
    }

    fn submit_read_event(&mut self, buf_idx: usize) {
        if self.file_pos_cursor >= self.end_pos {
            self.buffers_status[buf_idx] = BufferStatus::Invalid;
            return;
        }

        if (self.file_pos_cursor + self.buffer_size as u64) > self.end_pos {
            // use other method to read the remaining data

            // println!("...HERE...");
            let mut f = std::fs::File::open(&self.fpath).unwrap();
            f.seek(std::io::SeekFrom::Start(self.file_pos_cursor))
                .unwrap();
            let remaining_bytes = (self.end_pos - self.file_pos_cursor) as usize;
            let buf_slice = unsafe {
                std::slice::from_raw_parts_mut(self.buffers[buf_idx].data, remaining_bytes)
            };

            f.read_exact(buf_slice).unwrap();

            // f.read_exact(buf_slice).unwrap();
            self.buffers[buf_idx].len = remaining_bytes;
            self.buffers_status[buf_idx] = BufferStatus::Ready4Process;
            self.file_pos_cursor += remaining_bytes as u64;
            assert_eq!(self.file_pos_cursor, self.end_pos);
            return;
        }

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
            ReadFile(
                self.handle.handle,
                self.buffers[buf_idx].data as *mut _,
                self.buffer_size as u32,
                std::ptr::null_mut(), // lpNumberOfBytesRead = NULL for async
                &mut self.buffers[buf_idx].overlapped as *mut _,
            )
        };

        // if ok == 0 {
        //     panic!("ReadFile error");
        // }

        self.pendding += 1;

        self.file_pos_cursor += self.buffer_size as u64;
    }
}
