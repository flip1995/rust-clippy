// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use boxed::Box;
use error;
use fmt;
use option::Option::{self, Some, None};
use result;
use string::String;
use sys;

/// A type for results generated by I/O related functions where the `Err` type
/// is hard-wired to `io::Error`.
///
/// This typedef is generally used to avoid writing out `io::Error` directly and
/// is otherwise a direct mapping to `std::result::Result`.
#[stable(feature = "rust1", since = "1.0.0")]
pub type Result<T> = result::Result<T, Error>;

/// The error type for I/O operations of the `Read`, `Write`, `Seek`, and
/// associated traits.
///
/// Errors mostly originate from the underlying OS, but custom instances of
/// `Error` can be created with crafted error messages and a particular value of
/// `ErrorKind`.
#[derive(PartialEq, Eq, Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Error {
    repr: Repr,
}

#[derive(PartialEq, Eq, Clone, Debug)]
enum Repr {
    Os(i32),
    Custom(Box<Custom>),
}

#[derive(PartialEq, Eq, Clone, Debug)]
struct Custom {
    kind: ErrorKind,
    desc: &'static str,
    detail: Option<String>
}

/// A list specifying general categories of I/O error.
///
/// This list is intended to grow over time and it is not recommended to
/// exhaustively match against it.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum ErrorKind {
    /// An entity was not found, often a file.
    #[stable(feature = "rust1", since = "1.0.0")]
    NotFound,
    /// The operation lacked the necessary privileges to complete.
    #[stable(feature = "rust1", since = "1.0.0")]
    PermissionDenied,
    /// The connection was refused by the remote server.
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionRefused,
    /// The connection was reset by the remote server.
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionReset,
    /// The connection was aborted (terminated) by the remote server.
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionAborted,
    /// The network operation failed because it was not connected yet.
    #[stable(feature = "rust1", since = "1.0.0")]
    NotConnected,
    /// A socket address could not be bound because the address is already in
    /// use elsewhere.
    #[stable(feature = "rust1", since = "1.0.0")]
    AddrInUse,
    /// A nonexistent interface was requested or the requested address was not
    /// local.
    #[stable(feature = "rust1", since = "1.0.0")]
    AddrNotAvailable,
    /// The operation failed because a pipe was closed.
    #[stable(feature = "rust1", since = "1.0.0")]
    BrokenPipe,
    /// An entity already exists, often a file.
    #[stable(feature = "rust1", since = "1.0.0")]
    AlreadyExists,
    /// The operation needs to block to complete, but the blocking operation was
    /// requested to not occur.
    #[stable(feature = "rust1", since = "1.0.0")]
    WouldBlock,
    /// A parameter was incorrect.
    #[stable(feature = "rust1", since = "1.0.0")]
    InvalidInput,
    /// The I/O operation's timeout expired, causing it to be canceled.
    #[stable(feature = "rust1", since = "1.0.0")]
    TimedOut,
    /// An error returned when an operation could not be completed because a
    /// call to `write` returned `Ok(0)`.
    ///
    /// This typically means that an operation could only succeed if it wrote a
    /// particular number of bytes but only a smaller number of bytes could be
    /// written.
    #[stable(feature = "rust1", since = "1.0.0")]
    WriteZero,
    /// This operation was interrupted.
    ///
    /// Interrupted operations can typically be retried.
    #[stable(feature = "rust1", since = "1.0.0")]
    Interrupted,
    /// Any I/O error not part of this list.
    #[stable(feature = "rust1", since = "1.0.0")]
    Other,

    /// Any I/O error not part of this list.
    #[unstable(feature = "std_misc",
               reason = "better expressed through extensible enums that this \
                         enum cannot be exhaustively matched against")]
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Error {
    /// Creates a new custom error from a specified kind/description/detail.
    #[unstable(feature = "io", reason = "the exact makeup of an Error may
                                         change to include `Box<Error>` for \
                                         example")]
    pub fn new(kind: ErrorKind,
               description: &'static str,
               detail: Option<String>) -> Error {
        Error {
            repr: Repr::Custom(Box::new(Custom {
                kind: kind,
                desc: description,
                detail: detail,
            }))
        }
    }

    /// Returns an error representing the last OS error which occurred.
    ///
    /// This function reads the value of `errno` for the target platform (e.g.
    /// `GetLastError` on Windows) and will return a corresponding instance of
    /// `Error` for the error code.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn last_os_error() -> Error {
        Error::from_os_error(sys::os::errno() as i32)
    }

    /// Creates a new instance of an `Error` from a particular OS error code.
    #[unstable(feature = "io",
               reason = "unclear whether this function is necessary")]
    pub fn from_os_error(code: i32) -> Error {
        Error { repr: Repr::Os(code) }
    }

    /// Returns the OS error that this error represents (if any).
    ///
    /// If this `Error` was constructed via `last_os_error` then this function
    /// will return `Some`, otherwise it will return `None`.
    #[unstable(feature = "io", reason = "function was just added and the return \
                                         type may become an abstract OS error")]
    pub fn raw_os_error(&self) -> Option<i32> {
        match self.repr {
            Repr::Os(i) => Some(i),
            Repr::Custom(..) => None,
        }
    }

    /// Return the corresponding `ErrorKind` for this error.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn kind(&self) -> ErrorKind {
        match self.repr {
            Repr::Os(code) => sys::decode_error_kind(code),
            Repr::Custom(ref c) => c.kind,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.repr {
            Repr::Os(code) => {
                let detail = sys::os::error_string(code);
                write!(fmt, "{} (os error {})", detail, code)
            }
            Repr::Custom(ref c) => {
                match **c {
                    Custom {
                        kind: ErrorKind::Other,
                        desc: "unknown error",
                        detail: Some(ref detail)
                    } => {
                        write!(fmt, "{}", detail)
                    }
                    Custom { detail: None, desc, .. } =>
                        write!(fmt, "{}", desc),
                    Custom { detail: Some(ref detail), desc, .. } =>
                        write!(fmt, "{} ({})", desc, detail)
                }
            }
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl error::Error for Error {
    fn description(&self) -> &str {
        match self.repr {
            Repr::Os(..) => "os error",
            Repr::Custom(ref c) => c.desc,
        }
    }
}
