// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
 * Higher-level interfaces to libc::* functions and operating system services.
 *
 * In general these take and return rust types, use rust idioms (enums,
 * closures, vectors) rather than C idioms, and do more extensive safety
 * checks.
 *
 * This module is not meant to only contain 1:1 mappings to libc entries; any
 * os-interface code that is reasonably useful and broadly applicable can go
 * here. Including utility routines that merely build on other os code.
 *
 * We assume the general case is that users do not care, and do not want to
 * be made to care, which operating system they are on. While they may want
 * to special case various special cases -- and so we will not _hide_ the
 * facts of which OS the user is on -- they should be given the opportunity
 * to write OS-ignorant code by default.
 */

#[allow(missing_doc)];

#[cfg(unix)]
use c_str::CString;
use clone::Clone;
use container::Container;
use iter::range;
use libc;
use libc::{c_char, c_void, c_int, size_t};
use option::{Some, None};
use os;
use prelude::*;
use ptr;
use str;
use to_str;
use unstable::finally::Finally;
use vec;

pub use os::consts::*;

/// Delegates to the libc close() function, returning the same return value.
pub fn close(fd: c_int) -> c_int {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        libc::close(fd)
    }
}

pub static TMPBUF_SZ : uint = 1000u;
static BUF_BYTES : uint = 2048u;

#[cfg(unix)]
pub fn getcwd() -> Path {
    #[fixed_stack_segment]; #[inline(never)];
    let mut buf = [0 as libc::c_char, ..BUF_BYTES];
    do buf.as_mut_buf |buf, len| {
        unsafe {
            if libc::getcwd(buf, len as size_t).is_null() {
                fail!()
            }

            Path::new(CString::new(buf as *c_char, false))
        }
    }
}

#[cfg(windows)]
pub fn getcwd() -> Path {
    #[fixed_stack_segment]; #[inline(never)];
    use libc::DWORD;
    use libc::GetCurrentDirectoryW;
    let mut buf = [0 as u16, ..BUF_BYTES];
    do buf.as_mut_buf |buf, len| {
        unsafe {
            if libc::GetCurrentDirectoryW(len as DWORD, buf) == 0 as DWORD {
                fail!();
            }
        }
    }
    Path::new(str::from_utf16(buf))
}

#[cfg(windows)]
pub mod win32 {
    use libc;
    use vec;
    use str;
    use option::{None, Option};
    use option;
    use os::TMPBUF_SZ;
    use libc::types::os::arch::extra::DWORD;

    pub fn fill_utf16_buf_and_decode(f: &fn(*mut u16, DWORD) -> DWORD)
        -> Option<~str> {
        #[fixed_stack_segment]; #[inline(never)];

        unsafe {
            let mut n = TMPBUF_SZ as DWORD;
            let mut res = None;
            let mut done = false;
            while !done {
                let mut k: DWORD = 0;
                let mut buf = vec::from_elem(n as uint, 0u16);
                do buf.as_mut_buf |b, _sz| {
                    k = f(b, TMPBUF_SZ as DWORD);
                    if k == (0 as DWORD) {
                        done = true;
                    } else if (k == n &&
                               libc::GetLastError() ==
                               libc::ERROR_INSUFFICIENT_BUFFER as DWORD) {
                        n *= (2 as DWORD);
                    } else {
                        done = true;
                    }
                }
                if k != 0 && done {
                    let sub = buf.slice(0, k as uint);
                    res = option::Some(str::from_utf16(sub));
                }
            }
            return res;
        }
    }

    pub fn as_utf16_p<T>(s: &str, f: &fn(*u16) -> T) -> T {
        let mut t = s.to_utf16();
        // Null terminate before passing on.
        t.push(0u16);
        t.as_imm_buf(|buf, _len| f(buf))
    }
}

/*
Accessing environment variables is not generally threadsafe.
Serialize access through a global lock.
*/
fn with_env_lock<T>(f: &fn() -> T) -> T {
    use unstable::finally::Finally;

    unsafe {
        return do (|| {
            rust_take_env_lock();
            f()
        }).finally {
            rust_drop_env_lock();
        };
    }

    externfn!(fn rust_take_env_lock());
    externfn!(fn rust_drop_env_lock());
}

/// Returns a vector of (variable, value) pairs for all the environment
/// variables of the current process.
pub fn env() -> ~[(~str,~str)] {
    unsafe {
        #[cfg(windows)]
        unsafe fn get_env_pairs() -> ~[~str] {
            #[fixed_stack_segment]; #[inline(never)];
            use c_str;
            use str::StrSlice;

            use libc::funcs::extra::kernel32::{
                GetEnvironmentStringsA,
                FreeEnvironmentStringsA
            };
            let ch = GetEnvironmentStringsA();
            if (ch as uint == 0) {
                fail!("os::env() failure getting env string from OS: {}",
                       os::last_os_error());
            }
            let mut result = ~[];
            do c_str::from_c_multistring(ch as *libc::c_char, None) |cstr| {
                result.push(cstr.as_str().unwrap().to_owned());
            };
            FreeEnvironmentStringsA(ch);
            result
        }
        #[cfg(unix)]
        unsafe fn get_env_pairs() -> ~[~str] {
            #[fixed_stack_segment]; #[inline(never)];

            extern {
                fn rust_env_pairs() -> **libc::c_char;
            }
            let environ = rust_env_pairs();
            if (environ as uint == 0) {
                fail!("os::env() failure getting env string from OS: {}",
                       os::last_os_error());
            }
            let mut result = ~[];
            ptr::array_each(environ, |e| {
                let env_pair = str::raw::from_c_str(e);
                debug!("get_env_pairs: {}", env_pair);
                result.push(env_pair);
            });
            result
        }

        fn env_convert(input: ~[~str]) -> ~[(~str, ~str)] {
            let mut pairs = ~[];
            for p in input.iter() {
                let vs: ~[&str] = p.splitn_iter('=', 1).collect();
                debug!("splitting: len: {}", vs.len());
                assert_eq!(vs.len(), 2);
                pairs.push((vs[0].to_owned(), vs[1].to_owned()));
            }
            pairs
        }
        do with_env_lock {
            let unparsed_environ = get_env_pairs();
            env_convert(unparsed_environ)
        }
    }
}

#[cfg(unix)]
/// Fetches the environment variable `n` from the current process, returning
/// None if the variable isn't set.
pub fn getenv(n: &str) -> Option<~str> {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        do with_env_lock {
            let s = do n.with_c_str |buf| {
                libc::getenv(buf)
            };
            if s.is_null() {
                None
            } else {
                Some(str::raw::from_c_str(s))
            }
        }
    }
}

#[cfg(windows)]
/// Fetches the environment variable `n` from the current process, returning
/// None if the variable isn't set.
pub fn getenv(n: &str) -> Option<~str> {
    #[fixed_stack_segment]; #[inline(never)];

    unsafe {
        do with_env_lock {
            use os::win32::{as_utf16_p, fill_utf16_buf_and_decode};
            do as_utf16_p(n) |u| {
                do fill_utf16_buf_and_decode() |buf, sz| {
                    libc::GetEnvironmentVariableW(u, buf, sz)
                }
            }
        }
    }
}


#[cfg(unix)]
/// Sets the environment variable `n` to the value `v` for the currently running
/// process
pub fn setenv(n: &str, v: &str) {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        do with_env_lock {
            do n.with_c_str |nbuf| {
                do v.with_c_str |vbuf| {
                    libc::funcs::posix01::unistd::setenv(nbuf, vbuf, 1);
                }
            }
        }
    }
}


#[cfg(windows)]
/// Sets the environment variable `n` to the value `v` for the currently running
/// process
pub fn setenv(n: &str, v: &str) {
    #[fixed_stack_segment]; #[inline(never)];

    unsafe {
        do with_env_lock {
            use os::win32::as_utf16_p;
            do as_utf16_p(n) |nbuf| {
                do as_utf16_p(v) |vbuf| {
                    libc::SetEnvironmentVariableW(nbuf, vbuf);
                }
            }
        }
    }
}

/// Remove a variable from the environment entirely
pub fn unsetenv(n: &str) {
    #[cfg(unix)]
    fn _unsetenv(n: &str) {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            do with_env_lock {
                do n.with_c_str |nbuf| {
                    libc::funcs::posix01::unistd::unsetenv(nbuf);
                }
            }
        }
    }
    #[cfg(windows)]
    fn _unsetenv(n: &str) {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            do with_env_lock {
                use os::win32::as_utf16_p;
                do as_utf16_p(n) |nbuf| {
                    libc::SetEnvironmentVariableW(nbuf, ptr::null());
                }
            }
        }
    }

    _unsetenv(n);
}

pub struct Pipe {
    input: c_int,
    out: c_int
}

#[cfg(unix)]
pub fn pipe() -> Pipe {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        let mut fds = Pipe {input: 0 as c_int,
                            out: 0 as c_int };
        assert_eq!(libc::pipe(&mut fds.input), (0 as c_int));
        return Pipe {input: fds.input, out: fds.out};
    }
}

#[cfg(windows)]
pub fn pipe() -> Pipe {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        // Windows pipes work subtly differently than unix pipes, and their
        // inheritance has to be handled in a different way that I do not
        // fully understand. Here we explicitly make the pipe non-inheritable,
        // which means to pass it to a subprocess they need to be duplicated
        // first, as in std::run.
        let mut fds = Pipe {input: 0 as c_int,
                    out: 0 as c_int };
        let res = libc::pipe(&mut fds.input, 1024 as ::libc::c_uint,
                             (libc::O_BINARY | libc::O_NOINHERIT) as c_int);
        assert_eq!(res, 0 as c_int);
        assert!((fds.input != -1 as c_int && fds.input != 0 as c_int));
        assert!((fds.out != -1 as c_int && fds.input != 0 as c_int));
        return Pipe {input: fds.input, out: fds.out};
    }
}

fn dup2(src: c_int, dst: c_int) -> c_int {
    #[fixed_stack_segment]; #[inline(never)];
    unsafe {
        libc::dup2(src, dst)
    }
}

/// Returns the proper dll filename for the given basename of a file.
pub fn dll_filename(base: &str) -> ~str {
    format!("{}{}{}", DLL_PREFIX, base, DLL_SUFFIX)
}

/// Optionally returns the filesystem path to the current executable which is
/// running. If any failure occurs, None is returned.
pub fn self_exe_path() -> Option<Path> {

    #[cfg(target_os = "freebsd")]
    fn load_self() -> Option<~[u8]> {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            use libc::funcs::bsd44::*;
            use libc::consts::os::extra::*;
            let mib = ~[CTL_KERN as c_int,
                        KERN_PROC as c_int,
                        KERN_PROC_PATHNAME as c_int, -1 as c_int];
            let mut sz: size_t = 0;
            let err = sysctl(vec::raw::to_ptr(mib), mib.len() as ::libc::c_uint,
                             ptr::mut_null(), &mut sz, ptr::null(), 0u as size_t);
            if err != 0 { return None; }
            if sz == 0 { return None; }
            let mut v: ~[u8] = vec::with_capacity(sz as uint);
            let err = do v.as_mut_buf |buf,_| {
                sysctl(vec::raw::to_ptr(mib), mib.len() as ::libc::c_uint,
                       buf as *mut c_void, &mut sz, ptr::null(), 0u as size_t)
            };
            if err != 0 { return None; }
            if sz == 0 { return None; }
            vec::raw::set_len(&mut v, sz as uint - 1); // chop off trailing NUL
            Some(v)
        }
    }

    #[cfg(target_os = "linux")]
    #[cfg(target_os = "android")]
    fn load_self() -> Option<~[u8]> {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            use libc::funcs::posix01::unistd::readlink;

            let mut path: ~[u8] = vec::with_capacity(TMPBUF_SZ);

            let len = do path.as_mut_buf |buf, _| {
                do "/proc/self/exe".with_c_str |proc_self_buf| {
                    readlink(proc_self_buf, buf as *mut c_char, TMPBUF_SZ as size_t) as uint
                }
            };
            if len == -1 {
                None
            } else {
                vec::raw::set_len(&mut path, len as uint);
                Some(path)
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn load_self() -> Option<~[u8]> {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            use libc::funcs::extra::_NSGetExecutablePath;
            let mut sz: u32 = 0;
            _NSGetExecutablePath(ptr::mut_null(), &mut sz);
            if sz == 0 { return None; }
            let mut v: ~[u8] = vec::with_capacity(sz as uint);
            let err = do v.as_mut_buf |buf,_| {
                _NSGetExecutablePath(buf as *mut i8, &mut sz)
            };
            if err != 0 { return None; }
            vec::raw::set_len(&mut v, sz as uint - 1); // chop off trailing NUL
            Some(v)
        }
    }

    #[cfg(windows)]
    fn load_self() -> Option<~[u8]> {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            use os::win32::fill_utf16_buf_and_decode;
            do fill_utf16_buf_and_decode() |buf, sz| {
                libc::GetModuleFileNameW(0u as libc::DWORD, buf, sz)
            }.map(|s| s.into_bytes())
        }
    }

    load_self().and_then(|path| Path::new_opt(path).map(|mut p| { p.pop(); p }))
}

/**
 * Returns the path to the user's home directory, if known.
 *
 * On Unix, returns the value of the 'HOME' environment variable if it is set
 * and not equal to the empty string.
 *
 * On Windows, returns the value of the 'HOME' environment variable if it is
 * set and not equal to the empty string. Otherwise, returns the value of the
 * 'USERPROFILE' environment variable if it is set and not equal to the empty
 * string.
 *
 * Otherwise, homedir returns option::none.
 */
pub fn homedir() -> Option<Path> {
    // FIXME (#7188): getenv needs a ~[u8] variant
    return match getenv("HOME") {
        Some(ref p) if !p.is_empty() => Path::new_opt(p.as_slice()),
        _ => secondary()
    };

    #[cfg(unix)]
    fn secondary() -> Option<Path> {
        None
    }

    #[cfg(windows)]
    fn secondary() -> Option<Path> {
        do getenv("USERPROFILE").and_then |p| {
            if !p.is_empty() {
                Path::new_opt(p)
            } else {
                None
            }
        }
    }
}

/**
 * Returns the path to a temporary directory.
 *
 * On Unix, returns the value of the 'TMPDIR' environment variable if it is
 * set and non-empty and '/tmp' otherwise.
 * On Android, there is no global temporary folder (it is usually allocated
 * per-app), hence returns '/data/tmp' which is commonly used.
 *
 * On Windows, returns the value of, in order, the 'TMP', 'TEMP',
 * 'USERPROFILE' environment variable  if any are set and not the empty
 * string. Otherwise, tmpdir returns the path to the Windows directory.
 */
pub fn tmpdir() -> Path {
    return lookup();

    fn getenv_nonempty(v: &str) -> Option<Path> {
        match getenv(v) {
            Some(x) =>
                if x.is_empty() {
                    None
                } else {
                    Path::new_opt(x)
                },
            _ => None
        }
    }

    #[cfg(unix)]
    fn lookup() -> Path {
        if cfg!(target_os = "android") {
            Path::new("/data/tmp")
        } else {
            getenv_nonempty("TMPDIR").unwrap_or(Path::new("/tmp"))
        }
    }

    #[cfg(windows)]
    fn lookup() -> Path {
        getenv_nonempty("TMP").or(
            getenv_nonempty("TEMP").or(
                getenv_nonempty("USERPROFILE").or(
                   getenv_nonempty("WINDIR")))).unwrap_or(Path::new("C:\\Windows"))
    }
}

/**
 * Convert a relative path to an absolute path
 *
 * If the given path is relative, return it prepended with the current working
 * directory. If the given path is already an absolute path, return it
 * as is.
 */
// NB: this is here rather than in path because it is a form of environment
// querying; what it does depends on the process working directory, not just
// the input paths.
pub fn make_absolute(p: &Path) -> Path {
    if p.is_absolute() {
        p.clone()
    } else {
        let mut ret = getcwd();
        ret.push(p);
        ret
    }
}

/// Changes the current working directory to the specified path, returning
/// whether the change was completed successfully or not.
pub fn change_dir(p: &Path) -> bool {
    return chdir(p);

    #[cfg(windows)]
    fn chdir(p: &Path) -> bool {
        #[fixed_stack_segment]; #[inline(never)];
        unsafe {
            use os::win32::as_utf16_p;
            return do as_utf16_p(p.as_str().unwrap()) |buf| {
                libc::SetCurrentDirectoryW(buf) != (0 as libc::BOOL)
            };
        }
    }

    #[cfg(unix)]
    fn chdir(p: &Path) -> bool {
        #[fixed_stack_segment]; #[inline(never)];
        do p.with_c_str |buf| {
            unsafe {
                libc::chdir(buf) == (0 as c_int)
            }
        }
    }
}

#[cfg(unix)]
/// Returns the platform-specific value of errno
pub fn errno() -> int {
    #[cfg(target_os = "macos")]
    #[cfg(target_os = "freebsd")]
    fn errno_location() -> *c_int {
        #[fixed_stack_segment]; #[inline(never)];
        #[nolink]
        extern {
            fn __error() -> *c_int;
        }
        unsafe {
            __error()
        }
    }

    #[cfg(target_os = "linux")]
    #[cfg(target_os = "android")]
    fn errno_location() -> *c_int {
        #[fixed_stack_segment]; #[inline(never)];
        #[nolink]
        extern {
            fn __errno_location() -> *c_int;
        }
        unsafe {
            __errno_location()
        }
    }

    unsafe {
        (*errno_location()) as int
    }
}

#[cfg(windows)]
/// Returns the platform-specific value of errno
pub fn errno() -> uint {
    #[fixed_stack_segment]; #[inline(never)];
    use libc::types::os::arch::extra::DWORD;

    #[cfg(target_arch = "x86")]
    #[link_name = "kernel32"]
    extern "stdcall" {
        fn GetLastError() -> DWORD;
    }

    #[cfg(target_arch = "x86_64")]
    #[link_name = "kernel32"]
    extern {
        fn GetLastError() -> DWORD;
    }

    unsafe {
        GetLastError() as uint
    }
}

/// Get a string representing the platform-dependent last error
pub fn last_os_error() -> ~str {
    #[cfg(unix)]
    fn strerror() -> ~str {
        #[cfg(target_os = "macos")]
        #[cfg(target_os = "android")]
        #[cfg(target_os = "freebsd")]
        fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t)
                      -> c_int {
            #[fixed_stack_segment]; #[inline(never)];

            #[nolink]
            extern {
                fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t)
                              -> c_int;
            }
            unsafe {
                strerror_r(errnum, buf, buflen)
            }
        }

        // GNU libc provides a non-compliant version of strerror_r by default
        // and requires macros to instead use the POSIX compliant variant.
        // So we just use __xpg_strerror_r which is always POSIX compliant
        #[cfg(target_os = "linux")]
        fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int {
            #[fixed_stack_segment]; #[inline(never)];
            #[nolink]
            extern {
                fn __xpg_strerror_r(errnum: c_int,
                                    buf: *mut c_char,
                                    buflen: size_t)
                                    -> c_int;
            }
            unsafe {
                __xpg_strerror_r(errnum, buf, buflen)
            }
        }

        let mut buf = [0 as c_char, ..TMPBUF_SZ];

        do buf.as_mut_buf |buf, len| {
            unsafe {
                if strerror_r(errno() as c_int, buf, len as size_t) < 0 {
                    fail!("strerror_r failure");
                }

                str::raw::from_c_str(buf as *c_char)
            }
        }
    }

    #[cfg(windows)]
    fn strerror() -> ~str {
        #[fixed_stack_segment]; #[inline(never)];

        use libc::types::os::arch::extra::DWORD;
        use libc::types::os::arch::extra::LPWSTR;
        use libc::types::os::arch::extra::LPVOID;
        use libc::types::os::arch::extra::WCHAR;

        #[cfg(target_arch = "x86")]
        #[link_name = "kernel32"]
        extern "stdcall" {
            fn FormatMessageW(flags: DWORD,
                              lpSrc: LPVOID,
                              msgId: DWORD,
                              langId: DWORD,
                              buf: LPWSTR,
                              nsize: DWORD,
                              args: *c_void)
                              -> DWORD;
        }

        #[cfg(target_arch = "x86_64")]
        #[link_name = "kernel32"]
        extern {
            fn FormatMessageW(flags: DWORD,
                              lpSrc: LPVOID,
                              msgId: DWORD,
                              langId: DWORD,
                              buf: LPWSTR,
                              nsize: DWORD,
                              args: *c_void)
                              -> DWORD;
        }

        static FORMAT_MESSAGE_FROM_SYSTEM: DWORD = 0x00001000;
        static FORMAT_MESSAGE_IGNORE_INSERTS: DWORD = 0x00000200;

        // This value is calculated from the macro
        // MAKELANGID(LANG_SYSTEM_DEFAULT, SUBLANG_SYS_DEFAULT)
        let langId = 0x0800 as DWORD;
        let err = errno() as DWORD;

        let mut buf = [0 as WCHAR, ..TMPBUF_SZ];

        unsafe {
            do buf.as_mut_buf |buf, len| {
                let res = FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM |
                                         FORMAT_MESSAGE_IGNORE_INSERTS,
                                         ptr::mut_null(),
                                         err,
                                         langId,
                                         buf,
                                         len as DWORD,
                                         ptr::null());
                if res == 0 {
                    fail!("[{}] FormatMessage failure", errno());
                }
            }

            str::from_utf16(buf)
        }
    }

    strerror()
}

/**
 * Sets the process exit code
 *
 * Sets the exit code returned by the process if all supervised tasks
 * terminate successfully (without failing). If the current root task fails
 * and is supervised by the scheduler then any user-specified exit status is
 * ignored and the process exits with the default failure status
 */
pub fn set_exit_status(code: int) {
    use rt;
    rt::set_exit_status(code);
}

unsafe fn load_argc_and_argv(argc: c_int, argv: **c_char) -> ~[~str] {
    let mut args = ~[];
    for i in range(0u, argc as uint) {
        args.push(str::raw::from_c_str(*argv.offset(i as int)));
    }
    args
}

/**
 * Returns the command line arguments
 *
 * Returns a list of the command line arguments.
 */
#[cfg(target_os = "macos")]
fn real_args() -> ~[~str] {
    #[fixed_stack_segment]; #[inline(never)];

    unsafe {
        let (argc, argv) = (*_NSGetArgc() as c_int,
                            *_NSGetArgv() as **c_char);
        load_argc_and_argv(argc, argv)
    }
}

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[cfg(target_os = "freebsd")]
fn real_args() -> ~[~str] {
    use rt;

    match rt::args::clone() {
        Some(args) => args,
        None => fail!("process arguments not initialized")
    }
}

#[cfg(windows)]
fn real_args() -> ~[~str] {
    #[fixed_stack_segment]; #[inline(never)];

    let mut nArgs: c_int = 0;
    let lpArgCount: *mut c_int = &mut nArgs;
    let lpCmdLine = unsafe { GetCommandLineW() };
    let szArgList = unsafe { CommandLineToArgvW(lpCmdLine, lpArgCount) };

    let mut args = ~[];
    for i in range(0u, nArgs as uint) {
        unsafe {
            // Determine the length of this argument.
            let ptr = *szArgList.offset(i as int);
            let mut len = 0;
            while *ptr.offset(len as int) != 0 { len += 1; }

            // Push it onto the list.
            args.push(vec::raw::buf_as_slice(ptr, len,
                                             str::from_utf16));
        }
    }

    unsafe {
        LocalFree(szArgList as *c_void);
    }

    return args;
}

type LPCWSTR = *u16;

#[cfg(windows, target_arch = "x86")]
#[link_name="kernel32"]
#[abi="stdcall"]
extern "stdcall" {
    fn GetCommandLineW() -> LPCWSTR;
    fn LocalFree(ptr: *c_void);
}

#[cfg(windows, target_arch = "x86_64")]
#[link_name="kernel32"]
extern {
    fn GetCommandLineW() -> LPCWSTR;
    fn LocalFree(ptr: *c_void);
}

#[cfg(windows, target_arch = "x86")]
#[link_name="shell32"]
#[abi="stdcall"]
extern "stdcall" {
    fn CommandLineToArgvW(lpCmdLine: LPCWSTR, pNumArgs: *mut c_int) -> **u16;
}

#[cfg(windows, target_arch = "x86_64")]
#[link_name="shell32"]
extern {
    fn CommandLineToArgvW(lpCmdLine: LPCWSTR, pNumArgs: *mut c_int) -> **u16;
}

struct OverriddenArgs {
    val: ~[~str]
}

/// Returns the arguments which this program was started with (normally passed
/// via the command line).
pub fn args() -> ~[~str] {
    real_args()
}

#[cfg(target_os = "macos")]
extern {
    // These functions are in crt_externs.h.
    pub fn _NSGetArgc() -> *c_int;
    pub fn _NSGetArgv() -> ***c_char;
}

// Round up `from` to be divisible by `to`
fn round_up(from: uint, to: uint) -> uint {
    let r = if from % to == 0 {
        from
    } else {
        from + to - (from % to)
    };
    if r == 0 {
        to
    } else {
        r
    }
}

#[cfg(unix)]
pub fn page_size() -> uint {
    #[fixed_stack_segment]; #[inline(never)];

    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as uint
    }
}

#[cfg(windows)]
pub fn page_size() -> uint {
    #[fixed_stack_segment]; #[inline(never)];

    unsafe {
        let mut info = libc::SYSTEM_INFO::new();
        libc::GetSystemInfo(&mut info);

        return info.dwPageSize as uint;
    }
}

pub struct MemoryMap {
    data: *mut u8,
    len: size_t,
    kind: MemoryMapKind
}

pub enum MemoryMapKind {
    MapFile(*c_void),
    MapVirtual
}

pub enum MapOption {
    MapReadable,
    MapWritable,
    MapExecutable,
    MapAddr(*c_void),
    MapFd(c_int),
    MapOffset(uint)
}

pub enum MapError {
    // Linux-specific errors
    ErrFdNotAvail,
    ErrInvalidFd,
    ErrUnaligned,
    ErrNoMapSupport,
    ErrNoMem,
    ErrUnknown(libc::c_int),

    // Windows-specific errors
    ErrUnsupProt,
    ErrUnsupOffset,
    ErrAlreadyExists,
    ErrVirtualAlloc(uint),
    ErrCreateFileMappingW(uint),
    ErrMapViewOfFile(uint)
}

impl to_str::ToStr for MapError {
    fn to_str(&self) -> ~str {
        match *self {
            ErrFdNotAvail => ~"fd not available for reading or writing",
            ErrInvalidFd => ~"Invalid fd",
            ErrUnaligned => ~"Unaligned address, invalid flags, \
                              negative length or unaligned offset",
            ErrNoMapSupport=> ~"File doesn't support mapping",
            ErrNoMem => ~"Invalid address, or not enough available memory",
            ErrUnknown(code) => format!("Unknown error={}", code),
            ErrUnsupProt => ~"Protection mode unsupported",
            ErrUnsupOffset => ~"Offset in virtual memory mode is unsupported",
            ErrAlreadyExists => ~"File mapping for specified file already exists",
            ErrVirtualAlloc(code) => format!("VirtualAlloc failure={}", code),
            ErrCreateFileMappingW(code) => format!("CreateFileMappingW failure={}", code),
            ErrMapViewOfFile(code) => format!("MapViewOfFile failure={}", code)
        }
    }
}

#[cfg(unix)]
impl MemoryMap {
    pub fn new(min_len: uint, options: &[MapOption]) -> Result<MemoryMap, MapError> {
        #[fixed_stack_segment]; #[inline(never)];

        use libc::off_t;

        let mut addr: *c_void = ptr::null();
        let mut prot: c_int = 0;
        let mut flags: c_int = libc::MAP_PRIVATE;
        let mut fd: c_int = -1;
        let mut offset: off_t = 0;
        let len = round_up(min_len, page_size()) as size_t;

        for &o in options.iter() {
            match o {
                MapReadable => { prot |= libc::PROT_READ; },
                MapWritable => { prot |= libc::PROT_WRITE; },
                MapExecutable => { prot |= libc::PROT_EXEC; },
                MapAddr(addr_) => {
                    flags |= libc::MAP_FIXED;
                    addr = addr_;
                },
                MapFd(fd_) => {
                    flags |= libc::MAP_FILE;
                    fd = fd_;
                },
                MapOffset(offset_) => { offset = offset_ as off_t; }
            }
        }
        if fd == -1 { flags |= libc::MAP_ANON; }

        let r = unsafe {
            libc::mmap(addr, len, prot, flags, fd, offset)
        };
        if r.equiv(&libc::MAP_FAILED) {
            Err(match errno() as c_int {
                libc::EACCES => ErrFdNotAvail,
                libc::EBADF => ErrInvalidFd,
                libc::EINVAL => ErrUnaligned,
                libc::ENODEV => ErrNoMapSupport,
                libc::ENOMEM => ErrNoMem,
                code => ErrUnknown(code)
            })
        } else {
            Ok(MemoryMap {
               data: r as *mut u8,
               len: len,
               kind: if fd == -1 {
                   MapVirtual
               } else {
                   MapFile(ptr::null())
               }
            })
        }
    }

    pub fn granularity() -> uint {
        page_size()
    }
}

#[cfg(unix)]
impl Drop for MemoryMap {
    fn drop(&mut self) {
        #[fixed_stack_segment]; #[inline(never)];

        unsafe {
            match libc::munmap(self.data as *c_void, self.len) {
                0 => (),
                -1 => match errno() as c_int {
                    libc::EINVAL => error!("invalid addr or len"),
                    e => error!("unknown errno={}", e)
                },
                r => error!("Unexpected result {}", r)
            }
        }
    }
}

#[cfg(windows)]
impl MemoryMap {
    pub fn new(min_len: uint, options: &[MapOption]) -> Result<MemoryMap, MapError> {
        #[fixed_stack_segment]; #[inline(never)];

        use libc::types::os::arch::extra::{LPVOID, DWORD, SIZE_T, HANDLE};

        let mut lpAddress: LPVOID = ptr::mut_null();
        let mut readable = false;
        let mut writable = false;
        let mut executable = false;
        let mut fd: c_int = -1;
        let mut offset: uint = 0;
        let len = round_up(min_len, page_size()) as SIZE_T;

        for &o in options.iter() {
            match o {
                MapReadable => { readable = true; },
                MapWritable => { writable = true; },
                MapExecutable => { executable = true; }
                MapAddr(addr_) => { lpAddress = addr_ as LPVOID; },
                MapFd(fd_) => { fd = fd_; },
                MapOffset(offset_) => { offset = offset_; }
            }
        }

        let flProtect = match (executable, readable, writable) {
            (false, false, false) if fd == -1 => libc::PAGE_NOACCESS,
            (false, true, false) => libc::PAGE_READONLY,
            (false, true, true) => libc::PAGE_READWRITE,
            (true, false, false) if fd == -1 => libc::PAGE_EXECUTE,
            (true, true, false) => libc::PAGE_EXECUTE_READ,
            (true, true, true) => libc::PAGE_EXECUTE_READWRITE,
            _ => return Err(ErrUnsupProt)
        };

        if fd == -1 {
            if offset != 0 {
                return Err(ErrUnsupOffset);
            }
            let r = unsafe {
                libc::VirtualAlloc(lpAddress,
                                   len,
                                   libc::MEM_COMMIT | libc::MEM_RESERVE,
                                   flProtect)
            };
            match r as uint {
                0 => Err(ErrVirtualAlloc(errno())),
                _ => Ok(MemoryMap {
                   data: r as *mut u8,
                   len: len,
                   kind: MapVirtual
                })
            }
        } else {
            let dwDesiredAccess = match (executable, readable, writable) {
                (false, true, false) => libc::FILE_MAP_READ,
                (false, true, true) => libc::FILE_MAP_WRITE,
                (true, true, false) => libc::FILE_MAP_READ | libc::FILE_MAP_EXECUTE,
                (true, true, true) => libc::FILE_MAP_WRITE | libc::FILE_MAP_EXECUTE,
                _ => return Err(ErrUnsupProt) // Actually, because of the check above,
                                              // we should never get here.
            };
            unsafe {
                let hFile = libc::get_osfhandle(fd) as HANDLE;
                let mapping = libc::CreateFileMappingW(hFile,
                                                       ptr::mut_null(),
                                                       flProtect,
                                                       0,
                                                       0,
                                                       ptr::null());
                if mapping == ptr::mut_null() {
                    return Err(ErrCreateFileMappingW(errno()));
                }
                if errno() as c_int == libc::ERROR_ALREADY_EXISTS {
                    return Err(ErrAlreadyExists);
                }
                let r = libc::MapViewOfFile(mapping,
                                            dwDesiredAccess,
                                            ((len as u64) >> 32) as DWORD,
                                            (offset & 0xffff_ffff) as DWORD,
                                            0);
                match r as uint {
                    0 => Err(ErrMapViewOfFile(errno())),
                    _ => Ok(MemoryMap {
                       data: r as *mut u8,
                       len: len,
                       kind: MapFile(mapping as *c_void)
                    })
                }
            }
        }
    }

    /// Granularity of MapAddr() and MapOffset() parameter values.
    /// This may be greater than the value returned by page_size().
    pub fn granularity() -> uint {
        #[fixed_stack_segment]; #[inline(never)];

        unsafe {
            let mut info = libc::SYSTEM_INFO::new();
            libc::GetSystemInfo(&mut info);

            return info.dwAllocationGranularity as uint;
        }
    }
}

#[cfg(windows)]
impl Drop for MemoryMap {
    fn drop(&mut self) {
        #[fixed_stack_segment]; #[inline(never)];

        use libc::types::os::arch::extra::{LPCVOID, HANDLE};
        use libc::consts::os::extra::FALSE;

        unsafe {
            match self.kind {
                MapVirtual => {
                    if libc::VirtualFree(self.data as *mut c_void,
                                         self.len,
                                         libc::MEM_RELEASE) == FALSE {
                        error!("VirtualFree failed: {}", errno());
                    }
                },
                MapFile(mapping) => {
                    if libc::UnmapViewOfFile(self.data as LPCVOID) == FALSE {
                        error!("UnmapViewOfFile failed: {}", errno());
                    }
                    if libc::CloseHandle(mapping as HANDLE) == FALSE {
                        error!("CloseHandle failed: {}", errno());
                    }
                }
            }
        }
    }
}

pub mod consts {

    #[cfg(unix)]
    pub use os::consts::unix::*;

    #[cfg(windows)]
    pub use os::consts::windows::*;

    #[cfg(target_os = "macos")]
    pub use os::consts::macos::*;

    #[cfg(target_os = "freebsd")]
    pub use os::consts::freebsd::*;

    #[cfg(target_os = "linux")]
    pub use os::consts::linux::*;

    #[cfg(target_os = "android")]
    pub use os::consts::android::*;

    #[cfg(target_os = "win32")]
    pub use os::consts::win32::*;

    #[cfg(target_arch = "x86")]
    pub use os::consts::x86::*;

    #[cfg(target_arch = "x86_64")]
    pub use os::consts::x86_64::*;

    #[cfg(target_arch = "arm")]
    pub use os::consts::arm::*;

    #[cfg(target_arch = "mips")]
    pub use os::consts::mips::*;

    pub mod unix {
        pub static FAMILY: &'static str = "unix";
    }

    pub mod windows {
        pub static FAMILY: &'static str = "windows";
    }

    pub mod macos {
        pub static SYSNAME: &'static str = "macos";
        pub static DLL_PREFIX: &'static str = "lib";
        pub static DLL_SUFFIX: &'static str = ".dylib";
        pub static DLL_EXTENSION: &'static str = "dylib";
        pub static EXE_SUFFIX: &'static str = "";
        pub static EXE_EXTENSION: &'static str = "";
    }

    pub mod freebsd {
        pub static SYSNAME: &'static str = "freebsd";
        pub static DLL_PREFIX: &'static str = "lib";
        pub static DLL_SUFFIX: &'static str = ".so";
        pub static DLL_EXTENSION: &'static str = "so";
        pub static EXE_SUFFIX: &'static str = "";
        pub static EXE_EXTENSION: &'static str = "";
    }

    pub mod linux {
        pub static SYSNAME: &'static str = "linux";
        pub static DLL_PREFIX: &'static str = "lib";
        pub static DLL_SUFFIX: &'static str = ".so";
        pub static DLL_EXTENSION: &'static str = "so";
        pub static EXE_SUFFIX: &'static str = "";
        pub static EXE_EXTENSION: &'static str = "";
    }

    pub mod android {
        pub static SYSNAME: &'static str = "android";
        pub static DLL_PREFIX: &'static str = "lib";
        pub static DLL_SUFFIX: &'static str = ".so";
        pub static DLL_EXTENSION: &'static str = "so";
        pub static EXE_SUFFIX: &'static str = "";
        pub static EXE_EXTENSION: &'static str = "";
    }

    pub mod win32 {
        pub static SYSNAME: &'static str = "win32";
        pub static DLL_PREFIX: &'static str = "";
        pub static DLL_SUFFIX: &'static str = ".dll";
        pub static DLL_EXTENSION: &'static str = "dll";
        pub static EXE_SUFFIX: &'static str = ".exe";
        pub static EXE_EXTENSION: &'static str = "exe";
    }


    pub mod x86 {
        pub static ARCH: &'static str = "x86";
    }
    pub mod x86_64 {
        pub static ARCH: &'static str = "x86_64";
    }
    pub mod arm {
        pub static ARCH: &'static str = "arm";
    }
    pub mod mips {
        pub static ARCH: &'static str = "mips";
    }
}

#[cfg(test)]
mod tests {
    use c_str::ToCStr;
    use option::Some;
    use option;
    use os::{env, getcwd, getenv, make_absolute, args};
    use os::{setenv, unsetenv};
    use os;
    use path::Path;
    use rand::Rng;
    use rand;
    use str::StrSlice;


    #[test]
    pub fn last_os_error() {
        debug!("{}", os::last_os_error());
    }

    #[test]
    pub fn test_args() {
        let a = args();
        assert!(a.len() >= 1);
    }

    fn make_rand_name() -> ~str {
        let mut rng = rand::rng();
        let n = ~"TEST" + rng.gen_ascii_str(10u);
        assert!(getenv(n).is_none());
        n
    }

    #[test]
    fn test_setenv() {
        let n = make_rand_name();
        setenv(n, "VALUE");
        assert_eq!(getenv(n), option::Some(~"VALUE"));
    }

    #[test]
    fn test_unsetenv() {
        let n = make_rand_name();
        setenv(n, "VALUE");
        unsetenv(n);
        assert_eq!(getenv(n), option::None);
    }

    #[test]
    #[ignore]
    fn test_setenv_overwrite() {
        let n = make_rand_name();
        setenv(n, "1");
        setenv(n, "2");
        assert_eq!(getenv(n), option::Some(~"2"));
        setenv(n, "");
        assert_eq!(getenv(n), option::Some(~""));
    }

    // Windows GetEnvironmentVariable requires some extra work to make sure
    // the buffer the variable is copied into is the right size
    #[test]
    #[ignore]
    fn test_getenv_big() {
        let mut s = ~"";
        let mut i = 0;
        while i < 100 {
            s = s + "aaaaaaaaaa";
            i += 1;
        }
        let n = make_rand_name();
        setenv(n, s);
        debug!("{}", s.clone());
        assert_eq!(getenv(n), option::Some(s));
    }

    #[test]
    fn test_self_exe_path() {
        let path = os::self_exe_path();
        assert!(path.is_some());
        let path = path.unwrap();
        debug!("{:?}", path.clone());

        // Hard to test this function
        assert!(path.is_absolute());
    }

    #[test]
    #[ignore]
    fn test_env_getenv() {
        let e = env();
        assert!(e.len() > 0u);
        for p in e.iter() {
            let (n, v) = (*p).clone();
            debug!("{:?}", n.clone());
            let v2 = getenv(n);
            // MingW seems to set some funky environment variables like
            // "=C:=C:\MinGW\msys\1.0\bin" and "!::=::\" that are returned
            // from env() but not visible from getenv().
            assert!(v2.is_none() || v2 == option::Some(v));
        }
    }

    #[test]
    fn test_env_setenv() {
        let n = make_rand_name();

        let mut e = env();
        setenv(n, "VALUE");
        assert!(!e.contains(&(n.clone(), ~"VALUE")));

        e = env();
        assert!(e.contains(&(n, ~"VALUE")));
    }

    #[test]
    fn test() {
        assert!((!Path::new("test-path").is_absolute()));

        let cwd = getcwd();
        debug!("Current working directory: {}", cwd.display());

        debug!("{:?}", make_absolute(&Path::new("test-path")));
        debug!("{:?}", make_absolute(&Path::new("/usr/bin")));
    }

    #[test]
    #[cfg(unix)]
    fn homedir() {
        let oldhome = getenv("HOME");

        setenv("HOME", "/home/MountainView");
        assert_eq!(os::homedir(), Some(Path::new("/home/MountainView")));

        setenv("HOME", "");
        assert!(os::homedir().is_none());

        for s in oldhome.iter() { setenv("HOME", *s) }
    }

    #[test]
    #[cfg(windows)]
    fn homedir() {

        let oldhome = getenv("HOME");
        let olduserprofile = getenv("USERPROFILE");

        setenv("HOME", "");
        setenv("USERPROFILE", "");

        assert!(os::homedir().is_none());

        setenv("HOME", "/home/MountainView");
        assert_eq!(os::homedir(), Some(Path::new("/home/MountainView")));

        setenv("HOME", "");

        setenv("USERPROFILE", "/home/MountainView");
        assert_eq!(os::homedir(), Some(Path::new("/home/MountainView")));

        setenv("HOME", "/home/MountainView");
        setenv("USERPROFILE", "/home/PaloAlto");
        assert_eq!(os::homedir(), Some(Path::new("/home/MountainView")));

        for s in oldhome.iter() { setenv("HOME", *s) }
        for s in olduserprofile.iter() { setenv("USERPROFILE", *s) }
    }

    #[test]
    fn memory_map_rw() {
        use result::{Ok, Err};

        let chunk = match os::MemoryMap::new(16, [
            os::MapReadable,
            os::MapWritable
        ]) {
            Ok(chunk) => chunk,
            Err(msg) => fail!(msg.to_str())
        };
        assert!(chunk.len >= 16);

        unsafe {
            *chunk.data = 0xBE;
            assert!(*chunk.data == 0xBE);
        }
    }

    #[test]
    fn memory_map_file() {
        #[fixed_stack_segment]; #[inline(never)];

        use result::{Ok, Err};
        use os::*;
        use libc::*;
        use rt::io::file;

        #[cfg(unix)]
        #[fixed_stack_segment]
        #[inline(never)]
        fn lseek_(fd: c_int, size: uint) {
            unsafe {
                assert!(lseek(fd, size as off_t, SEEK_SET) == size as off_t);
            }
        }
        #[cfg(windows)]
        #[fixed_stack_segment]
        #[inline(never)]
        fn lseek_(fd: c_int, size: uint) {
           unsafe {
               assert!(lseek(fd, size as c_long, SEEK_SET) == size as c_long);
           }
        }

        let mut path = tmpdir();
        path.push("mmap_file.tmp");
        let size = MemoryMap::granularity() * 2;

        let fd = unsafe {
            let fd = do path.with_c_str |path| {
                open(path, O_CREAT | O_RDWR | O_TRUNC, S_IRUSR | S_IWUSR)
            };
            lseek_(fd, size);
            do "x".with_c_str |x| {
                assert!(write(fd, x as *c_void, 1) == 1);
            }
            fd
        };
        let chunk = match MemoryMap::new(size / 2, [
            MapReadable,
            MapWritable,
            MapFd(fd),
            MapOffset(size / 2)
        ]) {
            Ok(chunk) => chunk,
            Err(msg) => fail!(msg.to_str())
        };
        assert!(chunk.len > 0);

        unsafe {
            *chunk.data = 0xbe;
            assert!(*chunk.data == 0xbe);
            close(fd);
        }
        file::unlink(&path);
    }

    // More recursive_mkdir tests are in extra::tempfile
}
