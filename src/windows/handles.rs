#![cfg(windows)]
#![allow(non_snake_case)]

use crate::windows::utils::str_to_wide;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Foundation::{GENERIC_READ, GetLastError};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_FLAG_SEQUENTIAL_SCAN,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::CreateIoCompletionPort;

#[derive(Debug, Clone, Copy)]
pub enum FileMode {
    Read,
    Write,
}

pub struct FileHandle {
    pub handle: *mut c_void,
}
impl FileHandle {
    pub fn new(fpath: &str, file_mode: FileMode) -> anyhow::Result<Self> {
        let fpath_wide = str_to_wide(fpath);

        let dwdesiredaccess = match file_mode {
            FileMode::Read => GENERIC_READ,
            FileMode::Write => GENERIC_WRITE,
        };

        let dwcreationdisposition = match file_mode {
            FileMode::Read => OPEN_EXISTING,
            FileMode::Write => OPEN_ALWAYS,
        };

        let handle = unsafe {
            CreateFileW(
                fpath_wide.as_ptr(),
                dwdesiredaccess,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                dwcreationdisposition,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED | FILE_FLAG_SEQUENTIAL_SCAN,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            anyhow::bail!("INVALID_HANDLE_VALUE");
        }

        Ok(Self { handle })
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        let ret = unsafe { CloseHandle(self.handle) };
        // println!("FileHandle Drop: ret={}", ret);
        if ret == 0 {
            let err = unsafe { GetLastError() };
            eprintln!("CloseHandle failed with error: {}", err);
        }
    }
}

pub struct IocpHandle {
    pub handle: *mut c_void,
}

impl IocpHandle {
    pub fn new() -> anyhow::Result<Self> {
        let iocp =
            unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 0) };
        if iocp == std::ptr::null_mut() {
            anyhow::bail!("CreateIoCompletionPort New Failed");
        }
        Ok(Self { handle: iocp })
    }

    pub fn init(&mut self, filehandle: *mut c_void) -> anyhow::Result<()> {
        let new_handle = unsafe { CreateIoCompletionPort(filehandle, self.handle, 0, 0) };
        if new_handle == std::ptr::null_mut() {
            anyhow::bail!("CreateIoCompletionPort Init Failed");
        }
        self.handle = new_handle;
        Ok(())
    }
}

impl Drop for IocpHandle {
    fn drop(&mut self) {
        let ret = unsafe { CloseHandle(self.handle) };
        println!("IocpHandle Drop: ret={}", ret);
        if ret == 0 {
            let err = unsafe { GetLastError() };
            eprintln!("CloseHandle failed with error: {}", err);
        }
    }
}
