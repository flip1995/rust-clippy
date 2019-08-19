#![unstable(issue = "0", feature = "windows_stdio")]

use crate::io;
use crate::sys::c;
use crate::sys::handle::Handle;
use crate::mem::ManuallyDrop;

pub struct Stdin {
}
pub struct Stdout;
pub struct Stderr;

const MAX_BUFFER_SIZE: usize = 8192;
pub const STDIN_BUF_SIZE: usize = MAX_BUFFER_SIZE / 2 * 3;

pub fn get_handle(handle_id: c::DWORD) -> io::Result<c::HANDLE> {
    let handle = unsafe { c::GetStdHandle(handle_id) };
    if handle == c::INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else if handle.is_null() {
        Err(io::Error::from_raw_os_error(c::ERROR_INVALID_HANDLE as i32))
    } else {
        Ok(handle)
    }
}

fn write(handle_id: c::DWORD, data: &[u8]) -> io::Result<usize> {
    let handle = get_handle(handle_id)?;
    let handle = Handle::new(handle);
    ManuallyDrop::new(handle).write(data)
}

impl Stdin {
    pub fn new() -> io::Result<Stdin> {
        Ok(Stdin { })
    }
}

impl io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let handle = get_handle(c::STD_INPUT_HANDLE)?;
        let handle = Handle::new(handle);
        ManuallyDrop::new(handle).read(buf)
    }
}

impl Stdout {
    pub fn new() -> io::Result<Stdout> {
        Ok(Stdout)
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_OUTPUT_HANDLE, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub fn new() -> io::Result<Stderr> {
        Ok(Stderr)
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_ERROR_HANDLE, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn is_ebadf(err: &io::Error) -> bool {
    err.raw_os_error() == Some(c::ERROR_INVALID_HANDLE as i32)
}

pub fn panic_output() -> Option<impl io::Write> {
    Stderr::new().ok()
}
