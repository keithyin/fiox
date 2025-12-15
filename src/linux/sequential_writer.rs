#![cfg(target_os = "linux")]
use std::{
    fs::{self, OpenOptions},
    io::{Seek, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

use crate::{
    buffer_aux::{BufferDataPos, BufferStatus},
    linux::utils::get_page_size,
};
use anyhow::Context;
use io_uring::IoUring;

use super::buffer::Buffer;
pub struct SequentialWriter {
    #[allow(unused)]
    file: fs::File, // 不能删掉。要保证文件是打开的！
    fpath: String,
    buffer_size: usize,
    ring: IoUring,
    buffers: Vec<Buffer>,
    buffers_flag: Vec<BufferStatus>,
    data_location: BufferDataPos, // 即将要读取的 buffer 以及 offset
    pending_io: usize,
    file_pos_cursor: u64,
}

impl SequentialWriter {
    /// the caller need to make sure the sequential meta is valid
    pub fn new(
        fpath: &str,
        start_pos: u64,
        buffer_size: usize,
        num_buffer: usize,
    ) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .custom_flags(libc::O_DIRECT)
            .open(fpath)
            .unwrap();

        let page_size = get_page_size();
        assert_eq!(buffer_size % page_size, 0);
        assert_eq!(start_pos as usize % page_size, 0);

        if page_size == 0 {
            anyhow::bail!("get_page_size returned 0, which is invalid");
        }

        let ring = IoUring::new(num_buffer as u32).unwrap();

        let mut buffers: Vec<Buffer> = (0..num_buffer)
            .map(|_| Buffer::new(buffer_size, page_size))
            .collect();

        let iovecs = buffers
            .iter_mut()
            .map(|buf| {
                let buf_len = buf.cap();
                libc::iovec {
                    iov_base: buf.as_mut_ptr() as *mut _,
                    iov_len: buf_len as usize,
                }
            })
            .collect::<Vec<_>>();

        let offset = start_pos as usize % buffer_size;
        let readstart = start_pos - offset as u64;

        let data_location = BufferDataPos {
            buf_idx: 0,
            offset: offset as usize,
        };

        let buffers_flag = vec![BufferStatus::Ready4Process; num_buffer];

        unsafe {
            ring.submitter()
                .register_buffers(iovecs.as_slice())
                .expect("register buffers error");
            ring.submitter()
                .register_files(&[file.as_raw_fd()])
                .expect("register file error");
        }

        Ok(Self {
            file,
            fpath: fpath.to_string(),
            buffer_size,
            ring,
            buffers,
            buffers_flag,
            data_location: data_location,
            pending_io: 0,
            file_pos_cursor: readstart,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let record_len = data.len();
        let mut data_start = 0;

        while data_start < record_len {
            let buf_idx = self.data_location.buf_idx;
            self.wait_buf_ready4write(buf_idx)?;

            let expected_data_size = record_len - data_start;

            let (fill_size, current_buf_remaining) = {
                let buf = &mut self.buffers[buf_idx];

                let current_buf_remaining = (self.buffer_size - self.data_location.offset) as usize;
                let fill_size = current_buf_remaining.min(expected_data_size);

                (&mut buf[self.data_location.offset..self.data_location.offset + fill_size])
                    .copy_from_slice(&data[data_start..data_start + fill_size]);

                (fill_size, current_buf_remaining)
            };

            self.data_location.offset += fill_size;
            data_start += fill_size;
            if expected_data_size >= current_buf_remaining {
                let next_buf_idx: usize = (buf_idx + 1) % self.buffers.len();
                self.data_location.buf_idx = next_buf_idx;
                self.data_location.offset = 0;

                self.buffers_flag[buf_idx] = BufferStatus::Ready4Submit;
                self.submit_write_event(buf_idx)?;
            }
        }

        Ok(())
    }

    fn wait_buf_ready4write(&mut self, buf_idx: usize) -> anyhow::Result<()> {
        if self.buffers_flag[buf_idx] == BufferStatus::Ready4Process {
            return Ok(());
        }

        while self.pending_io > 0 {
            self.ring
                .submit_and_wait(1)
                .expect("Failed to submit and wait");
            let cqe = self
                .ring
                .completion()
                .next()
                .expect("No completion event found");
            self.pending_io -= 1;

            let idx = cqe.user_data() as usize;
            self.buffers_flag[idx] = BufferStatus::Ready4Process;
            self.buffers[idx].len = cqe.result() as usize;
            assert_eq!(self.buffers[idx].len, self.buffer_size);

            if self.buffers_flag[buf_idx] == BufferStatus::Ready4Process {
                return Ok(());
            }
        }

        anyhow::bail!("buffer {} is not ready for read", buf_idx);
    }

    fn submit_write_event(&mut self, buf_idx: usize) -> anyhow::Result<()> {
        let buf_cap = self.buffers[buf_idx].cap();
        self.buffers[buf_idx].len = 0; // reset length before read
        let sqe = io_uring::opcode::WriteFixed::new(
            io_uring::types::Fixed(0),
            self.buffers[buf_idx].as_mut_ptr(),
            buf_cap as u32,
            buf_idx as u16,
        )
        .offset(self.file_pos_cursor)
        .build()
        .user_data(buf_idx as u64);

        unsafe {
            self.ring
                .submission()
                .push(&sqe)
                .context("Failed to push submission queue entry")?;
        }
        self.pending_io += 1;
        self.file_pos_cursor += buf_cap as u64;
        Ok(())
    }
}

impl Drop for SequentialWriter {
    fn drop(&mut self) {
        while self.pending_io > 0 {
            self.ring
                .submit_and_wait(1)
                .expect("Failed to submit and wait");
            let _cqe = self
                .ring
                .completion()
                .next()
                .expect("No completion event found");
            self.pending_io -= 1;
        }

        if self.data_location.offset > 0 {
            let mut f = std::fs::OpenOptions::new()
                .read(true) // 可读
                .write(true) // 可写
                .create(true) // 不存在则创建
                .open(&self.fpath)
                .unwrap();
            f.seek(std::io::SeekFrom::Start(self.file_pos_cursor as u64))
                .unwrap();
            f.write_all(&self.buffers[self.data_location.buf_idx][..self.data_location.offset])
                .unwrap();
        }
    }
}
