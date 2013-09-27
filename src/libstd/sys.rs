// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Misc low level stuff

#[allow(missing_doc)];

use c_str::ToCStr;
use cast;
use libc;
use libc::{c_char, size_t};
use repr;
use str;
use unstable::intrinsics;

/// Returns the size of a type
#[inline]
pub fn size_of<T>() -> uint {
    unsafe { intrinsics::size_of::<T>() }
}

/// Returns the size of the type that `_val` points to
#[inline]
pub fn size_of_val<T>(_val: &T) -> uint {
    size_of::<T>()
}

/**
 * Returns the size of a type, or 1 if the actual size is zero.
 *
 * Useful for building structures containing variable-length arrays.
 */
#[inline]
pub fn nonzero_size_of<T>() -> uint {
    let s = size_of::<T>();
    if s == 0 { 1 } else { s }
}

/// Returns the size of the type of the value that `_val` points to
#[inline]
pub fn nonzero_size_of_val<T>(_val: &T) -> uint {
    nonzero_size_of::<T>()
}


/**
 * Returns the ABI-required minimum alignment of a type
 *
 * This is the alignment used for struct fields. It may be smaller
 * than the preferred alignment.
 */
#[inline]
pub fn min_align_of<T>() -> uint {
    unsafe { intrinsics::min_align_of::<T>() }
}

/// Returns the ABI-required minimum alignment of the type of the value that
/// `_val` points to
#[inline]
pub fn min_align_of_val<T>(_val: &T) -> uint {
    min_align_of::<T>()
}

/// Returns the preferred alignment of a type
#[inline]
pub fn pref_align_of<T>() -> uint {
    unsafe { intrinsics::pref_align_of::<T>() }
}

/// Returns the preferred alignment of the type of the value that
/// `_val` points to
#[inline]
pub fn pref_align_of_val<T>(_val: &T) -> uint {
    pref_align_of::<T>()
}

/// Returns the refcount of a shared box (as just before calling this)
#[inline]
pub fn refcount<T>(t: @T) -> uint {
    unsafe {
        let ref_ptr: *uint = cast::transmute_copy(&t);
        *ref_ptr - 1
    }
}

pub fn log_str<T>(t: &T) -> ~str {
    use rt::io;
    use rt::io::Decorator;

    let mut result = io::mem::MemWriter::new();
    repr::write_repr(&mut result as &mut io::Writer, t);
    str::from_utf8_owned(result.inner())
}

/// Trait for initiating task failure.
pub trait FailWithCause {
    /// Fail the current task, taking ownership of `cause`
    fn fail_with(cause: Self, file: &'static str, line: uint) -> !;
}

impl FailWithCause for ~str {
    fn fail_with(cause: ~str, file: &'static str, line: uint) -> ! {
        do cause.with_c_str |msg_buf| {
            do file.with_c_str |file_buf| {
                begin_unwind_(msg_buf, file_buf, line as libc::size_t)
            }
        }
    }
}

impl FailWithCause for &'static str {
    fn fail_with(cause: &'static str, file: &'static str, line: uint) -> ! {
        do cause.with_c_str |msg_buf| {
            do file.with_c_str |file_buf| {
                begin_unwind_(msg_buf, file_buf, line as libc::size_t)
            }
        }
    }
}

// This stage0 version is incredibly wrong.
#[cfg(stage0)]
pub fn begin_unwind_(msg: *c_char, file: *c_char, line: size_t) -> ! {
    use option::{Some, None};
    use rt::in_green_task_context;
    use rt::task::Task;
    use rt::local::Local;
    use rt::logging::Logger;
    use str::Str;

    unsafe {
        let msg = str::raw::from_c_str(msg);
        let file = str::raw::from_c_str(file);
        if in_green_task_context() {
            rterrln!("task failed at '%s', %s:%i", msg, file, line as int);
        } else {
            rterrln!("failed in non-task context at '%s', %s:%i",
                     msg, file, line as int);
        }

        let task: *mut Task = Local::unsafe_borrow();
        if (*task).unwinder.unwinding {
            rtabort!("unwinding again");
        }
        (*task).unwinder.begin_unwind();
    }
}

// FIXME #4427: Temporary until rt::rt_fail_ goes away
#[cfg(not(stage0))]
pub fn begin_unwind_(msg: *c_char, file: *c_char, line: size_t) -> ! {
    use rt::in_green_task_context;
    use rt::task::Task;
    use rt::local::Local;
    use rt::logging::Logger;
    use str::Str;

    unsafe {
        // XXX: Bad re-allocations. fail! needs some refactoring
        let msg = str::raw::from_c_str(msg);
        let file = str::raw::from_c_str(file);

        if in_green_task_context() {
            // Be careful not to allocate in this block, if we're failing we may
            // have been failing due to a lack of memory in the first place...
            do Local::borrow |task: &mut Task| {
                let n = task.name.map(|n| n.as_slice()).unwrap_or("<unnamed>");
                format_args!(|args| { task.logger.log(args) },
                             "task '{}' failed at '{}', {}:{}",
                             n, msg.as_slice(), file.as_slice(), line);
            }
        } else {
            rterrln!("failed in non-task context at '%s', %s:%i",
                     msg, file, line as int);
        }

        let task: *mut Task = Local::unsafe_borrow();
        if (*task).unwinder.unwinding {
            rtabort!("unwinding again");
        }
        (*task).unwinder.begin_unwind();
    }
}

#[cfg(test)]
mod tests {
    use cast;
    use sys::*;

    #[test]
    fn size_of_basic() {
        assert_eq!(size_of::<u8>(), 1u);
        assert_eq!(size_of::<u16>(), 2u);
        assert_eq!(size_of::<u32>(), 4u);
        assert_eq!(size_of::<u64>(), 8u);
    }

    #[test]
    #[cfg(target_arch = "x86")]
    #[cfg(target_arch = "arm")]
    #[cfg(target_arch = "mips")]
    fn size_of_32() {
        assert_eq!(size_of::<uint>(), 4u);
        assert_eq!(size_of::<*uint>(), 4u);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn size_of_64() {
        assert_eq!(size_of::<uint>(), 8u);
        assert_eq!(size_of::<*uint>(), 8u);
    }

    #[test]
    fn size_of_val_basic() {
        assert_eq!(size_of_val(&1u8), 1);
        assert_eq!(size_of_val(&1u16), 2);
        assert_eq!(size_of_val(&1u32), 4);
        assert_eq!(size_of_val(&1u64), 8);
    }

    #[test]
    fn nonzero_size_of_basic() {
        type Z = [i8, ..0];
        assert_eq!(size_of::<Z>(), 0u);
        assert_eq!(nonzero_size_of::<Z>(), 1u);
        assert_eq!(nonzero_size_of::<uint>(), size_of::<uint>());
    }

    #[test]
    fn nonzero_size_of_val_basic() {
        let z = [0u8, ..0];
        assert_eq!(size_of_val(&z), 0u);
        assert_eq!(nonzero_size_of_val(&z), 1u);
        assert_eq!(nonzero_size_of_val(&1u), size_of_val(&1u));
    }

    #[test]
    fn align_of_basic() {
        assert_eq!(pref_align_of::<u8>(), 1u);
        assert_eq!(pref_align_of::<u16>(), 2u);
        assert_eq!(pref_align_of::<u32>(), 4u);
    }

    #[test]
    #[cfg(target_arch = "x86")]
    #[cfg(target_arch = "arm")]
    #[cfg(target_arch = "mips")]
    fn align_of_32() {
        assert_eq!(pref_align_of::<uint>(), 4u);
        assert_eq!(pref_align_of::<*uint>(), 4u);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn align_of_64() {
        assert_eq!(pref_align_of::<uint>(), 8u);
        assert_eq!(pref_align_of::<*uint>(), 8u);
    }

    #[test]
    fn align_of_val_basic() {
        assert_eq!(pref_align_of_val(&1u8), 1u);
        assert_eq!(pref_align_of_val(&1u16), 2u);
        assert_eq!(pref_align_of_val(&1u32), 4u);
    }

    #[test]
    fn synthesize_closure() {
        use unstable::raw::Closure;
        unsafe {
            let x = 10;
            let f: &fn(int) -> int = |y| x + y;

            assert_eq!(f(20), 30);

            let original_closure: Closure = cast::transmute(f);

            let actual_function_pointer = original_closure.code;
            let environment = original_closure.env;

            let new_closure = Closure {
                code: actual_function_pointer,
                env: environment
            };

            let new_f: &fn(int) -> int = cast::transmute(new_closure);
            assert_eq!(new_f(20), 30);
        }
    }

    #[test]
    #[should_fail]
    fn fail_static() { FailWithCause::fail_with("cause", file!(), line!())  }

    #[test]
    #[should_fail]
    fn fail_owned() { FailWithCause::fail_with(~"cause", file!(), line!())  }
}
