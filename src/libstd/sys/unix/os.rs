// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of `std::os` functionality for unix systems

#![allow(unused_imports)] // lots of cfg code here

use prelude::v1::*;
use os::unix::prelude::*;

use error::Error as StdError;
use ffi::{CString, CStr, OsString, OsStr};
use fmt;
use io;
use iter;
use libc::{self, c_int, c_char, c_void};
use mem;
use memchr;
use path::{self, PathBuf};
use ptr;
use slice;
use str;
use sync::StaticMutex;
use sys::cvt;
use sys::fd;
use vec;

const TMPBUF_SZ: usize = 128;
static ENV_LOCK: StaticMutex = StaticMutex::new();

/// Returns the platform-specific value of errno
pub fn errno() -> i32 {
    extern {
        #[cfg_attr(any(target_os = "linux"), link_name = "__errno_location")]
        #[cfg_attr(any(target_os = "bitrig",
                       target_os = "netbsd",
                       target_os = "openbsd",
                       target_os = "android",
                       target_env = "newlib"),
                   link_name = "__errno")]
        #[cfg_attr(target_os = "solaris", link_name = "___errno")]
        #[cfg_attr(target_os = "dragonfly", link_name = "__dfly_error")]
        #[cfg_attr(any(target_os = "macos",
                       target_os = "ios",
                       target_os = "freebsd"),
                   link_name = "__error")]
        fn errno_location() -> *const c_int;
    }

    unsafe {
        (*errno_location()) as i32
    }
}

/// Gets a detailed string description for the given error number.
pub fn error_string(errno: i32) -> String {
    extern {
        #[cfg_attr(any(target_os = "linux", target_env = "newlib"),
                   link_name = "__xpg_strerror_r")]
        fn strerror_r(errnum: c_int, buf: *mut c_char,
                      buflen: libc::size_t) -> c_int;
    }

    let mut buf = [0 as c_char; TMPBUF_SZ];

    let p = buf.as_mut_ptr();
    unsafe {
        if strerror_r(errno as c_int, p, buf.len() as libc::size_t) < 0 {
            panic!("strerror_r failure");
        }

        let p = p as *const _;
        str::from_utf8(CStr::from_ptr(p).to_bytes()).unwrap().to_owned()
    }
}

pub fn getcwd() -> io::Result<PathBuf> {
    let mut buf = Vec::with_capacity(512);
    loop {
        unsafe {
            let ptr = buf.as_mut_ptr() as *mut libc::c_char;
            if !libc::getcwd(ptr, buf.capacity() as libc::size_t).is_null() {
                let len = CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_bytes().len();
                buf.set_len(len);
                buf.shrink_to_fit();
                return Ok(PathBuf::from(OsString::from_vec(buf)));
            } else {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ERANGE) {
                    return Err(error);
                }
            }

            // Trigger the internal buffer resizing logic of `Vec` by requiring
            // more space than the current capacity.
            let cap = buf.capacity();
            buf.set_len(cap);
            buf.reserve(1);
        }
    }
}

pub fn chdir(p: &path::Path) -> io::Result<()> {
    let p: &OsStr = p.as_ref();
    let p = try!(CString::new(p.as_bytes()));
    unsafe {
        match libc::chdir(p.as_ptr()) == (0 as c_int) {
            true => Ok(()),
            false => Err(io::Error::last_os_error()),
        }
    }
}

pub struct SplitPaths<'a> {
    iter: iter::Map<slice::Split<'a, u8, fn(&u8) -> bool>,
                    fn(&'a [u8]) -> PathBuf>,
}

pub fn split_paths(unparsed: &OsStr) -> SplitPaths {
    fn bytes_to_path(b: &[u8]) -> PathBuf {
        PathBuf::from(<OsStr as OsStrExt>::from_bytes(b))
    }
    fn is_colon(b: &u8) -> bool { *b == b':' }
    let unparsed = unparsed.as_bytes();
    SplitPaths {
        iter: unparsed.split(is_colon as fn(&u8) -> bool)
                      .map(bytes_to_path as fn(&[u8]) -> PathBuf)
    }
}

impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> { self.iter.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}

#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
    where I: Iterator<Item=T>, T: AsRef<OsStr>
{
    let mut joined = Vec::new();
    let sep = b':';

    for (i, path) in paths.enumerate() {
        let path = path.as_ref().as_bytes();
        if i > 0 { joined.push(sep) }
        if path.contains(&sep) {
            return Err(JoinPathsError)
        }
        joined.extend_from_slice(path);
    }
    Ok(OsStringExt::from_vec(joined))
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        "path segment contains separator `:`".fmt(f)
    }
}

impl StdError for JoinPathsError {
    fn description(&self) -> &str { "failed to join paths" }
}

#[cfg(target_os = "freebsd")]
pub fn current_exe() -> io::Result<PathBuf> {
    unsafe {
        let mut mib = [libc::CTL_KERN as c_int,
                       libc::KERN_PROC as c_int,
                       libc::KERN_PROC_PATHNAME as c_int,
                       -1 as c_int];
        let mut sz: libc::size_t = 0;
        try!(cvt(libc::sysctl(mib.as_mut_ptr(), mib.len() as ::libc::c_uint,
                              ptr::null_mut(), &mut sz, ptr::null_mut(),
                              0 as libc::size_t)));
        if sz == 0 {
            return Err(io::Error::last_os_error())
        }
        let mut v: Vec<u8> = Vec::with_capacity(sz as usize);
        try!(cvt(libc::sysctl(mib.as_mut_ptr(), mib.len() as ::libc::c_uint,
                              v.as_mut_ptr() as *mut libc::c_void, &mut sz,
                              ptr::null_mut(), 0 as libc::size_t)));
        if sz == 0 {
            return Err(io::Error::last_os_error());
        }
        v.set_len(sz as usize - 1); // chop off trailing NUL
        Ok(PathBuf::from(OsString::from_vec(v)))
    }
}

#[cfg(target_os = "dragonfly")]
pub fn current_exe() -> io::Result<PathBuf> {
    ::fs::read_link("/proc/curproc/file")
}

#[cfg(target_os = "netbsd")]
pub fn current_exe() -> io::Result<PathBuf> {
    ::fs::read_link("/proc/curproc/exe")
}

#[cfg(any(target_os = "bitrig", target_os = "openbsd"))]
pub fn current_exe() -> io::Result<PathBuf> {
    unsafe {
        let mut mib = [libc::CTL_KERN,
                       libc::KERN_PROC_ARGS,
                       libc::getpid(),
                       libc::KERN_PROC_ARGV];
        let mib = mib.as_mut_ptr();
        let mut argv_len = 0;
        try!(cvt(libc::sysctl(mib, 4, 0 as *mut _, &mut argv_len,
                              0 as *mut _, 0)));
        let mut argv = Vec::<*const libc::c_char>::with_capacity(argv_len as usize);
        try!(cvt(libc::sysctl(mib, 4, argv.as_mut_ptr() as *mut _,
                              &mut argv_len, 0 as *mut _, 0)));
        argv.set_len(argv_len as usize);
        if argv[0].is_null() {
            return Err(io::Error::new(io::ErrorKind::Other,
                                      "no current exe available"))
        }
        let argv0 = CStr::from_ptr(argv[0]).to_bytes();
        if argv0[0] == b'.' || argv0.iter().any(|b| *b == b'/') {
            ::fs::canonicalize(OsStr::from_bytes(argv0))
        } else {
            Ok(PathBuf::from(OsStr::from_bytes(argv0)))
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn current_exe() -> io::Result<PathBuf> {
    ::fs::read_link("/proc/self/exe")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn current_exe() -> io::Result<PathBuf> {
    extern {
        fn _NSGetExecutablePath(buf: *mut libc::c_char,
                                bufsize: *mut u32) -> libc::c_int;
    }
    unsafe {
        let mut sz: u32 = 0;
        _NSGetExecutablePath(ptr::null_mut(), &mut sz);
        if sz == 0 { return Err(io::Error::last_os_error()); }
        let mut v: Vec<u8> = Vec::with_capacity(sz as usize);
        let err = _NSGetExecutablePath(v.as_mut_ptr() as *mut i8, &mut sz);
        if err != 0 { return Err(io::Error::last_os_error()); }
        v.set_len(sz as usize - 1); // chop off trailing NUL
        Ok(PathBuf::from(OsString::from_vec(v)))
    }
}

#[cfg(any(target_os = "solaris"))]
pub fn current_exe() -> io::Result<PathBuf> {
    extern {
        fn getexecname() -> *const c_char;
    }
    unsafe {
        let path = getexecname();
        if path.is_null() {
            Err(io::Error::last_os_error())
        } else {
            let filename = CStr::from_ptr(path).to_bytes();
            let path = PathBuf::from(<OsStr as OsStrExt>::from_bytes(filename));

            // Prepend a current working directory to the path if
            // it doesn't contain an absolute pathname.
            if filename[0] == b'/' {
                Ok(path)
            } else {
                getcwd().map(|cwd| cwd.join(path))
            }
        }
    }
}

pub struct Args {
    iter: vec::IntoIter<OsString>,
    _dont_send_or_sync_me: *mut (),
}

impl Iterator for Args {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> { self.iter.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize { self.iter.len() }
}

/// Returns the command line arguments
///
/// Returns a list of the command line arguments.
#[cfg(target_os = "macos")]
pub fn args() -> Args {
    extern {
        // These functions are in crt_externs.h.
        fn _NSGetArgc() -> *mut c_int;
        fn _NSGetArgv() -> *mut *mut *mut c_char;
    }

    let vec = unsafe {
        let (argc, argv) = (*_NSGetArgc() as isize,
                            *_NSGetArgv() as *const *const c_char);
        (0.. argc as isize).map(|i| {
            let bytes = CStr::from_ptr(*argv.offset(i)).to_bytes().to_vec();
            OsStringExt::from_vec(bytes)
        }).collect::<Vec<_>>()
    };
    Args {
        iter: vec.into_iter(),
        _dont_send_or_sync_me: ptr::null_mut(),
    }
}

// As _NSGetArgc and _NSGetArgv aren't mentioned in iOS docs
// and use underscores in their names - they're most probably
// are considered private and therefore should be avoided
// Here is another way to get arguments using Objective C
// runtime
//
// In general it looks like:
// res = Vec::new()
// let args = [[NSProcessInfo processInfo] arguments]
// for i in (0..[args count])
//      res.push([args objectAtIndex:i])
// res
#[cfg(target_os = "ios")]
pub fn args() -> Args {
    use mem;

    #[link(name = "objc")]
    extern {
        fn sel_registerName(name: *const libc::c_uchar) -> Sel;
        fn objc_msgSend(obj: NsId, sel: Sel, ...) -> NsId;
        fn objc_getClass(class_name: *const libc::c_uchar) -> NsId;
    }

    #[link(name = "Foundation", kind = "framework")]
    extern {}

    type Sel = *const libc::c_void;
    type NsId = *const libc::c_void;

    let mut res = Vec::new();

    unsafe {
        let process_info_sel = sel_registerName("processInfo\0".as_ptr());
        let arguments_sel = sel_registerName("arguments\0".as_ptr());
        let utf8_sel = sel_registerName("UTF8String\0".as_ptr());
        let count_sel = sel_registerName("count\0".as_ptr());
        let object_at_sel = sel_registerName("objectAtIndex:\0".as_ptr());

        let klass = objc_getClass("NSProcessInfo\0".as_ptr());
        let info = objc_msgSend(klass, process_info_sel);
        let args = objc_msgSend(info, arguments_sel);

        let cnt: usize = mem::transmute(objc_msgSend(args, count_sel));
        for i in 0..cnt {
            let tmp = objc_msgSend(args, object_at_sel, i);
            let utf_c_str: *const libc::c_char =
                mem::transmute(objc_msgSend(tmp, utf8_sel));
            let bytes = CStr::from_ptr(utf_c_str).to_bytes();
            res.push(OsString::from(str::from_utf8(bytes).unwrap()))
        }
    }

    Args { iter: res.into_iter(), _dont_send_or_sync_me: ptr::null_mut() }
}

#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "netbsd",
          target_os = "openbsd",
          target_os = "solaris",
          target_os = "nacl"))]
pub fn args() -> Args {
    use sys_common;
    let bytes = sys_common::args::clone().unwrap_or(Vec::new());
    let v: Vec<OsString> = bytes.into_iter().map(|v| {
        OsStringExt::from_vec(v)
    }).collect();
    Args { iter: v.into_iter(), _dont_send_or_sync_me: ptr::null_mut() }
}

pub struct Env {
    iter: vec::IntoIter<(OsString, OsString)>,
    _dont_send_or_sync_me: *mut (),
}

impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> { self.iter.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}

#[cfg(target_os = "macos")]
pub unsafe fn environ() -> *mut *const *const c_char {
    extern { fn _NSGetEnviron() -> *mut *const *const c_char; }
    _NSGetEnviron()
}

#[cfg(not(target_os = "macos"))]
pub unsafe fn environ() -> *mut *const *const c_char {
    extern { static mut environ: *const *const c_char; }
    &mut environ
}

/// Returns a vector of (variable, value) byte-vector pairs for all the
/// environment variables of the current process.
pub fn env() -> Env {
    let _g = ENV_LOCK.lock();
    return unsafe {
        let mut environ = *environ();
        if environ == ptr::null() {
            panic!("os::env() failure getting env string from OS: {}",
                   io::Error::last_os_error());
        }
        let mut result = Vec::new();
        while *environ != ptr::null() {
            if let Some(key_value) = parse(CStr::from_ptr(*environ).to_bytes()) {
                result.push(key_value);
            }
            environ = environ.offset(1);
        }
        Env { iter: result.into_iter(), _dont_send_or_sync_me: ptr::null_mut() }
    };

    fn parse(input: &[u8]) -> Option<(OsString, OsString)> {
        // Strategy (copied from glibc): Variable name and value are separated
        // by an ASCII equals sign '='. Since a variable name must not be
        // empty, allow variable names starting with an equals sign. Skip all
        // malformed lines.
        if input.is_empty() {
            return None;
        }
        let pos = memchr::memchr(b'=', &input[1..]).map(|p| p + 1);
        pos.map(|p| (
            OsStringExt::from_vec(input[..p].to_vec()),
            OsStringExt::from_vec(input[p+1..].to_vec()),
        ))
    }
}

pub fn getenv(k: &OsStr) -> io::Result<Option<OsString>> {
    // environment variables with a nul byte can't be set, so their value is
    // always None as well
    let k = try!(CString::new(k.as_bytes()));
    let _g = ENV_LOCK.lock();
    Ok(unsafe {
        let s = libc::getenv(k.as_ptr()) as *const _;
        if s.is_null() {
            None
        } else {
            Some(OsStringExt::from_vec(CStr::from_ptr(s).to_bytes().to_vec()))
        }
    })
}

pub fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    let k = try!(CString::new(k.as_bytes()));
    let v = try!(CString::new(v.as_bytes()));
    let _g = ENV_LOCK.lock();
    cvt(unsafe {
        libc::setenv(k.as_ptr(), v.as_ptr(), 1)
    }).map(|_| ())
}

pub fn unsetenv(n: &OsStr) -> io::Result<()> {
    let nbuf = try!(CString::new(n.as_bytes()));
    let _g = ENV_LOCK.lock();
    cvt(unsafe {
        libc::unsetenv(nbuf.as_ptr())
    }).map(|_| ())
}

pub fn page_size() -> usize {
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
}

pub fn temp_dir() -> PathBuf {
    ::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| {
        if cfg!(target_os = "android") {
            PathBuf::from("/data/local/tmp")
        } else {
            PathBuf::from("/tmp")
        }
    })
}

pub fn home_dir() -> Option<PathBuf> {
    return ::env::var_os("HOME").or_else(|| unsafe {
        fallback()
    }).map(PathBuf::from);

    #[cfg(any(target_os = "android",
              target_os = "ios",
              target_os = "nacl"))]
    unsafe fn fallback() -> Option<OsString> { None }
    #[cfg(not(any(target_os = "android",
                  target_os = "ios",
                  target_os = "nacl")))]
    unsafe fn fallback() -> Option<OsString> {
        #[cfg(not(target_os = "solaris"))]
        unsafe fn getpwduid_r(me: libc::uid_t, passwd: &mut libc::passwd,
                              buf: &mut Vec<c_char>) -> Option<()> {
            let mut result = ptr::null_mut();
            match libc::getpwuid_r(me, passwd, buf.as_mut_ptr(),
                                   buf.capacity() as libc::size_t,
                                   &mut result) {
                0 if !result.is_null() => Some(()),
                _ => None
            }
        }

        #[cfg(target_os = "solaris")]
        unsafe fn getpwduid_r(me: libc::uid_t, passwd: &mut libc::passwd,
                              buf: &mut Vec<c_char>) -> Option<()> {
            // getpwuid_r semantics is different on Illumos/Solaris:
            // http://illumos.org/man/3c/getpwuid_r
            let result = libc::getpwuid_r(me, passwd, buf.as_mut_ptr(),
                                          buf.capacity() as libc::size_t);
            if result.is_null() { None } else { Some(()) }
        }

        let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512 as usize,
            n => n as usize,
        };
        let me = libc::getuid();
        loop {
            let mut buf = Vec::with_capacity(amt);
            let mut passwd: libc::passwd = mem::zeroed();

            if getpwduid_r(me, &mut passwd, &mut buf).is_some() {
                let ptr = passwd.pw_dir as *const _;
                let bytes = CStr::from_ptr(ptr).to_bytes().to_vec();
                return Some(OsStringExt::from_vec(bytes))
            } else {
                return None;
            }
        }
    }
}

pub fn exit(code: i32) -> ! {
    unsafe { libc::exit(code as c_int) }
}
