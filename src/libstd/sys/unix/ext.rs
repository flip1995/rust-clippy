// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Experimental extensions to `std` for Unix platforms.
//!
//! For now, this module is limited to extracting file descriptors,
//! but its functionality will grow over time.
//!
//! # Example
//!
//! ```rust,ignore
//! #![feature(globs)]
//!
//! use std::old_io::fs::File;
//! use std::os::unix::prelude::*;
//!
//! fn main() {
//!     let f = File::create(&Path::new("foo.txt")).unwrap();
//!     let fd = f.as_raw_fd();
//!
//!     // use fd with native unix bindings
//! }
//! ```

#![unstable(feature = "std_misc")]

use ffi::{OsStr, OsString};
use fs::{self, Permissions, OpenOptions};
use net;
use libc;
use mem;
use sys::os_str::Buf;
use sys_common::{AsInner, AsInnerMut, IntoInner, FromInner};
use vec::Vec;

use old_io;

/// Raw file descriptors.
pub type Fd = libc::c_int;

/// Extract raw file descriptor
pub trait AsRawFd {
    /// Extract the raw file descriptor, without taking any ownership.
    fn as_raw_fd(&self) -> Fd;
}

impl AsRawFd for old_io::fs::File {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for fs::File {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd().raw()
    }
}

impl AsRawFd for old_io::pipe::PipeStream {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::pipe::UnixStream {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::pipe::UnixListener {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::pipe::UnixAcceptor {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::tcp::TcpStream {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::tcp::TcpListener {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::tcp::TcpAcceptor {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for old_io::net::udp::UdpSocket {
    fn as_raw_fd(&self) -> Fd {
        self.as_inner().fd()
    }
}

impl AsRawFd for net::TcpStream {
    fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
}
impl AsRawFd for net::TcpListener {
    fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
}
impl AsRawFd for net::UdpSocket {
    fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
}

// Unix-specific extensions to `OsString`.
pub trait OsStringExt {
    /// Create an `OsString` from a byte vector.
    fn from_vec(vec: Vec<u8>) -> Self;

    /// Yield the underlying byte vector of this `OsString`.
    fn into_vec(self) -> Vec<u8>;
}

impl OsStringExt for OsString {
    fn from_vec(vec: Vec<u8>) -> OsString {
        FromInner::from_inner(Buf { inner: vec })
    }

    fn into_vec(self) -> Vec<u8> {
        self.into_inner().inner
    }
}

// Unix-specific extensions to `OsStr`.
pub trait OsStrExt {
    fn from_byte_slice(slice: &[u8]) -> &OsStr;
    fn as_byte_slice(&self) -> &[u8];
}

impl OsStrExt for OsStr {
    fn from_byte_slice(slice: &[u8]) -> &OsStr {
        unsafe { mem::transmute(slice) }
    }
    fn as_byte_slice(&self) -> &[u8] {
        &self.as_inner().inner
    }
}

// Unix-specific extensions to `Permissions`
pub trait PermissionsExt {
    fn set_mode(&mut self, mode: i32);
}

impl PermissionsExt for Permissions {
    fn set_mode(&mut self, mode: i32) {
        *self = FromInner::from_inner(FromInner::from_inner(mode));
    }
}

// Unix-specific extensions to `OpenOptions`
pub trait OpenOptionsExt {
    /// Set the mode bits that a new file will be created with.
    ///
    /// If a new file is created as part of a `File::open_opts` call then this
    /// specified `mode` will be used as the permission bits for the new file.
    fn mode(&mut self, mode: i32) -> &mut Self;
}

impl OpenOptionsExt for OpenOptions {
    fn mode(&mut self, mode: i32) -> &mut OpenOptions {
        self.as_inner_mut().mode(mode); self
    }
}

/// A prelude for conveniently writing platform-specific code.
///
/// Includes all extension traits, and some important type definitions.
pub mod prelude {
    #[doc(no_inline)]
    pub use super::{Fd, AsRawFd, OsStrExt, OsStringExt, PermissionsExt};
}
