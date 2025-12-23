#![cfg(target_os = "linux")]
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek},
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, OpenOptionsExt},
    },
};

use crate::{
    buffer_aux::{BufferDataPos, BufferStatus},
    linux::utils::get_page_size,
};
use anyhow::Context;
use io_uring::IoUring;

use super::buffer::Buffer;
pub struct SequentialReader {
    #[allow(unused)]
    file: fs::File, // 不能删掉。要保证文件是打开的！
    fpath: String,
    buff_size: usize,
    ring: IoUring,
    buffers: Vec<Buffer>,
    buffers_flag: Vec<BufferStatus>,
    data_location: BufferDataPos, // 即将要读取的 buffer 以及 offset
    pending_io: usize,
    init_flag: bool,
    file_pos_cursor: u64,
    end_pos: u64,
}

impl SequentialReader {
    /// the caller need to make sure the sequential meta is valid
    pub fn new(
        fpath: &str,
        start_pos: u64,
        buffer_size: usize,
        num_buffer: usize,
        end_pos: Option<u64>,
    ) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECT)
            .open(fpath)
            .unwrap();

        let page_size = get_page_size();
        assert_eq!(buffer_size % page_size, 0);

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

        let buffers_flag = vec![BufferStatus::Ready4Submit; num_buffer];

        unsafe {
            ring.submitter()
                .register_buffers(iovecs.as_slice())
                .expect("register buffers error");
            ring.submitter()
                .register_files(&[file.as_raw_fd()])
                .expect("register file error");
        }

        let file_size = crate::utils::get_file_size(fpath);
        let end_pos = match end_pos {
            Some(pos) => pos,
            None => file_size,
        };

        if end_pos > file_size {
            anyhow::bail!("end_pos {} is larger than file size {}", end_pos, file_size);
        }

        Ok(Self {
            file,
            fpath: fpath.to_string(),
            buff_size: buffer_size,
            ring,
            buffers,
            buffers_flag,
            data_location: data_location,
            pending_io: 0,
            init_flag: false,
            file_pos_cursor: readstart,
            end_pos: end_pos,
        })
    }

    pub fn read2buf(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        self.read_exact(buf)
    }

    fn read_exact(&mut self, data: &mut [u8]) -> anyhow::Result<usize> {
        let record_len = data.len();
        let mut data_start = 0;

        while data_start < record_len {
            let buf_idx = self.data_location.buf_idx;
            self.wait_buf_ready4read(buf_idx)?;
            if self.buffers_flag[buf_idx] == BufferStatus::Invalid {
                // no more data to read
                return Ok(data_start);
            }

            let expected_data_size = record_len - data_start;

            let (fill_size, current_buf_remaining) = {
                let buf = &self.buffers[buf_idx];

                let current_buf_remaining = (buf.len() - self.data_location.offset) as usize;
                let fill_size = current_buf_remaining.min(expected_data_size);
                data[data_start..data_start + fill_size].copy_from_slice(
                    &buf[self.data_location.offset..self.data_location.offset + fill_size],
                );
                (fill_size, current_buf_remaining)
            };

            self.data_location.offset += fill_size;
            data_start += fill_size;
            if expected_data_size >= current_buf_remaining {
                // current buffer is not enough, need to read next buffer
                // self.ready4sqe_buffer_indices.push(buf_idx);
                let next_buf_idx: usize = (buf_idx + 1) % self.buffers.len();
                self.data_location.buf_idx = next_buf_idx;
                self.data_location.offset = 0;

                self.buffers_flag[buf_idx] = BufferStatus::Ready4Submit;
                self.submit_read_event(buf_idx)?;
            }
        }

        Ok(record_len)
    }

    fn wait_buf_ready4read(&mut self, buf_idx: usize) -> anyhow::Result<()> {
        if self.buffers_flag[buf_idx] == BufferStatus::Ready4Process {
            return Ok(());
        }

        if self.buffers_flag[buf_idx] == BufferStatus::Invalid {
            return Ok(());
        }

        // initial state
        if !self.init_flag {
            for idx in 0..self.buffers.len() {
                self.submit_read_event(idx)?;
            }
            self.init_flag = true;
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
            assert_eq!(self.buffers[idx].len, self.buff_size);

            if self.buffers_flag[buf_idx] == BufferStatus::Ready4Process {
                return Ok(());
            }
        }

        anyhow::bail!("buffer {} is not ready for read", buf_idx);
    }

    fn submit_read_event(&mut self, buf_idx: usize) -> anyhow::Result<()> {
        if self.file_pos_cursor >= self.end_pos {
            // no more data to read
            self.buffers_flag[buf_idx] = BufferStatus::Invalid;
            return Ok(());
        }

        if (self.file_pos_cursor + self.buff_size as u64) > self.end_pos {
            // last read
            let remaining_bytes = (self.end_pos - self.file_pos_cursor) as usize;
            let f = std::fs::File::open(&self.fpath)?;
            f.read_exact_at(
                &mut self.buffers[buf_idx][..remaining_bytes],
                self.file_pos_cursor,
            )?;

            // println!(" ..... LAST READ HERE .....");
            self.buffers_flag[buf_idx] = BufferStatus::Ready4Process;
            self.buffers[buf_idx].len = remaining_bytes;
            self.file_pos_cursor += remaining_bytes as u64;
            return Ok(());
        }

        let buf_cap = self.buffers[buf_idx].cap();
        self.buffers[buf_idx].len = 0; // reset length before read
        let sqe = io_uring::opcode::ReadFixed::new(
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
