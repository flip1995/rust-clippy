// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use error::Error;
use libc;

/// A trait for implementing arbitrary return types in the `main` function.
///
/// The c-main function only supports to return integers as return type.
/// So, every type implementing the `Termination` trait has to be converted
/// to an integer.
///
/// The default implementations are returning `libc::EXIT_SUCCESS` to indicate
/// a successful execution. In case of a failure, `libc::EXIT_FAILURE` is returned.
#[cfg_attr(not(stage0), lang = "termination")]
#[unstable(feature = "termination_trait", issue = "0")]
pub trait Termination {
    /// Is called to get the representation of the value as status code.
    /// This status code is returned to the operating system.
    fn report(self) -> i32;
}

#[unstable(feature = "termination_trait", issue = "0")]
impl Termination for () {
    fn report(self) -> i32 { libc::EXIT_SUCCESS }
}

#[unstable(feature = "termination_trait", issue = "0")]
impl<T: Termination, E: Error> Termination for Result<T, E> {
    fn report(self) -> i32 {
        match self {
            Ok(val) => val.report(),
            Err(err) => {
                print_error(err);
                libc::EXIT_FAILURE
            }
        }
    }
}

#[unstable(feature = "termination_trait", issue = "0")]
fn print_error<E: Error>(err: E) {
    eprintln!("Error: {}", err.description());

    if let Some(ref err) = err.cause() {
        eprintln!("Caused by: {}", err.description());
    }
}

#[unstable(feature = "termination_trait", issue = "0")]
impl Termination for ! {
    fn report(self) -> i32 { unreachable!(); }
}

#[unstable(feature = "termination_trait", issue = "0")]
impl Termination for bool {
    fn report(self) -> i32 {
        if self { libc::EXIT_SUCCESS } else { libc::EXIT_FAILURE }
    }
}

#[unstable(feature = "termination_trait", issue = "0")]
impl Termination for i32 {
    fn report(self) -> i32 {
        self
    }
}
