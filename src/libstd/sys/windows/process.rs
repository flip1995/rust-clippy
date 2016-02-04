// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use prelude::v1::*;

use ascii::*;
use collections::HashMap;
use collections;
use env::split_paths;
use env;
use ffi::{OsString, OsStr};
use fmt;
use fs;
use io::{self, Error, ErrorKind};
use libc::c_void;
use mem;
use os::windows::ffi::OsStrExt;
use path::Path;
use ptr;
use sync::StaticMutex;
use sys::c;

use sys::fs::{OpenOptions, File};
use sys::handle::{Handle, RawHandle};
use sys::stdio;
use sys::{self, cvt};
use sys_common::{AsInner, FromInner};

////////////////////////////////////////////////////////////////////////////////
// Command
////////////////////////////////////////////////////////////////////////////////

fn mk_key(s: &OsStr) -> OsString {
    FromInner::from_inner(sys::os_str::Buf {
        inner: s.as_inner().inner.to_ascii_uppercase()
    })
}

fn ensure_no_nuls<T: AsRef<OsStr>>(str: T) -> io::Result<T> {
    if str.as_ref().encode_wide().any(|b| b == 0) {
        Err(io::Error::new(ErrorKind::InvalidInput, "nul byte found in provided data"))
    } else {
        Ok(str)
    }
}

#[derive(Clone)]
pub struct Command {
    program: OsString,
    args: Vec<OsString>,
    env: Option<HashMap<OsString, OsString>>,
    cwd: Option<OsString>,
    detach: bool, // not currently exposed in std::process
}

impl Command {
    pub fn new(program: &OsStr) -> Command {
        Command {
            program: program.to_os_string(),
            args: Vec::new(),
            env: None,
            cwd: None,
            detach: false,
        }
    }

    pub fn arg(&mut self, arg: &OsStr) {
        self.args.push(arg.to_os_string())
    }
    fn init_env_map(&mut self){
        if self.env.is_none() {
            self.env = Some(env::vars_os().map(|(key, val)| {
                (mk_key(&key), val)
            }).collect());
        }
    }
    pub fn env(&mut self, key: &OsStr, val: &OsStr) {
        self.init_env_map();
        self.env.as_mut().unwrap().insert(mk_key(key), val.to_os_string());
    }
    pub fn env_remove(&mut self, key: &OsStr) {
        self.init_env_map();
        self.env.as_mut().unwrap().remove(&mk_key(key));
    }
    pub fn env_clear(&mut self) {
        self.env = Some(HashMap::new())
    }
    pub fn cwd(&mut self, dir: &OsStr) {
        self.cwd = Some(dir.to_os_string())
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{:?}", self.program));
        for arg in &self.args {
            try!(write!(f, " {:?}", arg));
        }
        Ok(())
    }
}

////////////////////////////////////////////////////////////////////////////////
// Processes
////////////////////////////////////////////////////////////////////////////////

/// A value representing a child process.
///
/// The lifetime of this value is linked to the lifetime of the actual
/// process - the Process destructor calls self.finish() which waits
/// for the process to terminate.
pub struct Process {
    handle: Handle,
}

pub enum Stdio {
    Inherit,
    None,
    Raw(c::HANDLE),
}

pub type RawStdio = Handle;

impl Process {
    pub fn spawn(cfg: &Command,
                 in_handle: Stdio,
                 out_handle: Stdio,
                 err_handle: Stdio) -> io::Result<Process>
    {
        // To have the spawning semantics of unix/windows stay the same, we need
        // to read the *child's* PATH if one is provided. See #15149 for more
        // details.
        let program = cfg.env.as_ref().and_then(|env| {
            for (key, v) in env {
                if OsStr::new("PATH") != &**key { continue }

                // Split the value and test each path to see if the
                // program exists.
                for path in split_paths(&v) {
                    let path = path.join(cfg.program.to_str().unwrap())
                                   .with_extension(env::consts::EXE_EXTENSION);
                    if fs::metadata(&path).is_ok() {
                        return Some(path.into_os_string())
                    }
                }
                break
            }
            None
        });

        let mut si = zeroed_startupinfo();
        si.cb = mem::size_of::<c::STARTUPINFO>() as c::DWORD;
        si.dwFlags = c::STARTF_USESTDHANDLES;

        let stdin = try!(in_handle.to_handle(c::STD_INPUT_HANDLE));
        let stdout = try!(out_handle.to_handle(c::STD_OUTPUT_HANDLE));
        let stderr = try!(err_handle.to_handle(c::STD_ERROR_HANDLE));

        si.hStdInput = stdin.raw();
        si.hStdOutput = stdout.raw();
        si.hStdError = stderr.raw();

        let program = program.as_ref().unwrap_or(&cfg.program);
        let mut cmd_str = try!(make_command_line(program, &cfg.args));
        cmd_str.push(0); // add null terminator

        // stolen from the libuv code.
        let mut flags = c::CREATE_UNICODE_ENVIRONMENT;
        if cfg.detach {
            flags |= c::DETACHED_PROCESS | c::CREATE_NEW_PROCESS_GROUP;
        }

        let (envp, _data) = try!(make_envp(cfg.env.as_ref()));
        let (dirp, _data) = try!(make_dirp(cfg.cwd.as_ref()));
        let mut pi = zeroed_process_information();
        try!(unsafe {
            // `CreateProcess` is racy!
            // http://support.microsoft.com/kb/315939
            static CREATE_PROCESS_LOCK: StaticMutex = StaticMutex::new();
            let _lock = CREATE_PROCESS_LOCK.lock();

            cvt(c::CreateProcessW(ptr::null(),
                                  cmd_str.as_mut_ptr(),
                                  ptr::null_mut(),
                                  ptr::null_mut(),
                                  c::TRUE, flags, envp, dirp,
                                  &mut si, &mut pi))
        });

        // We close the thread handle because we don't care about keeping
        // the thread id valid, and we aren't keeping the thread handle
        // around to be able to close it later.
        drop(Handle::new(pi.hThread));

        Ok(Process { handle: Handle::new(pi.hProcess) })
    }

    pub fn kill(&mut self) -> io::Result<()> {
        try!(cvt(unsafe {
            c::TerminateProcess(self.handle.raw(), 1)
        }));
        Ok(())
    }

    pub fn id(&self) -> u32 {
        unsafe {
            c::GetProcessId(self.handle.raw()) as u32
        }
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        unsafe {
            let res = c::WaitForSingleObject(self.handle.raw(), c::INFINITE);
            if res != c::WAIT_OBJECT_0 {
                return Err(Error::last_os_error())
            }
            let mut status = 0;
            try!(cvt(c::GetExitCodeProcess(self.handle.raw(), &mut status)));
            Ok(ExitStatus(status))
        }
    }

    pub fn handle(&self) -> &Handle { &self.handle }

    pub fn into_handle(self) -> Handle { self.handle }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitStatus(c::DWORD);

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.0 == 0
    }
    pub fn code(&self) -> Option<i32> {
        Some(self.0 as i32)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exit code: {}", self.0)
    }
}

fn zeroed_startupinfo() -> c::STARTUPINFO {
    c::STARTUPINFO {
        cb: 0,
        lpReserved: ptr::null_mut(),
        lpDesktop: ptr::null_mut(),
        lpTitle: ptr::null_mut(),
        dwX: 0,
        dwY: 0,
        dwXSize: 0,
        dwYSize: 0,
        dwXCountChars: 0,
        dwYCountCharts: 0,
        dwFillAttribute: 0,
        dwFlags: 0,
        wShowWindow: 0,
        cbReserved2: 0,
        lpReserved2: ptr::null_mut(),
        hStdInput: c::INVALID_HANDLE_VALUE,
        hStdOutput: c::INVALID_HANDLE_VALUE,
        hStdError: c::INVALID_HANDLE_VALUE,
    }
}

fn zeroed_process_information() -> c::PROCESS_INFORMATION {
    c::PROCESS_INFORMATION {
        hProcess: ptr::null_mut(),
        hThread: ptr::null_mut(),
        dwProcessId: 0,
        dwThreadId: 0
    }
}

// Produces a wide string *without terminating null*; returns an error if
// `prog` or any of the `args` contain a nul.
fn make_command_line(prog: &OsStr, args: &[OsString]) -> io::Result<Vec<u16>> {
    // Encode the command and arguments in a command line string such
    // that the spawned process may recover them using CommandLineToArgvW.
    let mut cmd: Vec<u16> = Vec::new();
    try!(append_arg(&mut cmd, prog));
    for arg in args {
        cmd.push(' ' as u16);
        try!(append_arg(&mut cmd, arg));
    }
    return Ok(cmd);

    fn append_arg(cmd: &mut Vec<u16>, arg: &OsStr) -> io::Result<()> {
        // If an argument has 0 characters then we need to quote it to ensure
        // that it actually gets passed through on the command line or otherwise
        // it will be dropped entirely when parsed on the other end.
        try!(ensure_no_nuls(arg));
        let arg_bytes = &arg.as_inner().inner.as_inner();
        let quote = arg_bytes.iter().any(|c| *c == b' ' || *c == b'\t')
            || arg_bytes.is_empty();
        if quote {
            cmd.push('"' as u16);
        }

        let mut iter = arg.encode_wide();
        let mut backslashes: usize = 0;
        while let Some(x) = iter.next() {
            if x == '\\' as u16 {
                backslashes += 1;
            } else {
                if x == '"' as u16 {
                    // Add n+1 backslashes to total 2n+1 before internal '"'.
                    for _ in 0..(backslashes+1) {
                        cmd.push('\\' as u16);
                    }
                }
                backslashes = 0;
            }
            cmd.push(x);
        }

        if quote {
            // Add n backslashes to total 2n before ending '"'.
            for _ in 0..backslashes {
                cmd.push('\\' as u16);
            }
            cmd.push('"' as u16);
        }
        Ok(())
    }
}

fn make_envp(env: Option<&collections::HashMap<OsString, OsString>>)
             -> io::Result<(*mut c_void, Vec<u16>)> {
    // On Windows we pass an "environment block" which is not a char**, but
    // rather a concatenation of null-terminated k=v\0 sequences, with a final
    // \0 to terminate.
    match env {
        Some(env) => {
            let mut blk = Vec::new();

            for pair in env {
                blk.extend(try!(ensure_no_nuls(pair.0)).encode_wide());
                blk.push('=' as u16);
                blk.extend(try!(ensure_no_nuls(pair.1)).encode_wide());
                blk.push(0);
            }
            blk.push(0);
            Ok((blk.as_mut_ptr() as *mut c_void, blk))
        }
        _ => Ok((ptr::null_mut(), Vec::new()))
    }
}

fn make_dirp(d: Option<&OsString>) -> io::Result<(*const u16, Vec<u16>)> {

    match d {
        Some(dir) => {
            let mut dir_str: Vec<u16> = try!(ensure_no_nuls(dir)).encode_wide().collect();
            dir_str.push(0);
            Ok((dir_str.as_ptr(), dir_str))
        },
        None => Ok((ptr::null(), Vec::new()))
    }
}

impl Stdio {
    fn to_handle(&self, stdio_id: c::DWORD) -> io::Result<Handle> {
        match *self {
            // If no stdio handle is available, then inherit means that it
            // should still be unavailable so propagate the
            // INVALID_HANDLE_VALUE.
            Stdio::Inherit => {
                match stdio::get(stdio_id) {
                    Ok(io) => io.handle().duplicate(0, true,
                                                    c::DUPLICATE_SAME_ACCESS),
                    Err(..) => Ok(Handle::new(c::INVALID_HANDLE_VALUE)),
                }
            }
            Stdio::Raw(handle) => {
                RawHandle::new(handle).duplicate(0, true, c::DUPLICATE_SAME_ACCESS)
            }

            // Similarly to unix, we don't actually leave holes for the
            // stdio file descriptors, but rather open up /dev/null
            // equivalents. These equivalents are drawn from libuv's
            // windows process spawning.
            Stdio::None => {
                let size = mem::size_of::<c::SECURITY_ATTRIBUTES>();
                let mut sa = c::SECURITY_ATTRIBUTES {
                    nLength: size as c::DWORD,
                    lpSecurityDescriptor: ptr::null_mut(),
                    bInheritHandle: 1,
                };
                let mut opts = OpenOptions::new();
                opts.read(stdio_id == c::STD_INPUT_HANDLE);
                opts.write(stdio_id != c::STD_INPUT_HANDLE);
                opts.security_attributes(&mut sa);
                File::open(Path::new("NUL"), &opts).map(|file| {
                    file.into_handle()
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use prelude::v1::*;
    use ffi::{OsStr, OsString};
    use super::make_command_line;

    #[test]
    fn test_make_command_line() {
        fn test_wrapper(prog: &str, args: &[&str]) -> String {
            let command_line = &make_command_line(OsStr::new(prog),
                                                  &args.iter()
                                                       .map(|a| OsString::from(a))
                                                       .collect::<Vec<OsString>>())
                                    .unwrap();
            String::from_utf16(command_line).unwrap()
        }

        assert_eq!(
            test_wrapper("prog", &["aaa", "bbb", "ccc"]),
            "prog aaa bbb ccc"
        );

        assert_eq!(
            test_wrapper("C:\\Program Files\\blah\\blah.exe", &["aaa"]),
            "\"C:\\Program Files\\blah\\blah.exe\" aaa"
        );
        assert_eq!(
            test_wrapper("C:\\Program Files\\test", &["aa\"bb"]),
            "\"C:\\Program Files\\test\" aa\\\"bb"
        );
        assert_eq!(
            test_wrapper("echo", &["a b c"]),
            "echo \"a b c\""
        );
        assert_eq!(
            test_wrapper("echo", &["\" \\\" \\", "\\"]),
            "echo \"\\\" \\\\\\\" \\\\\" \\"
        );
        assert_eq!(
            test_wrapper("\u{03c0}\u{042f}\u{97f3}\u{00e6}\u{221e}", &[]),
            "\u{03c0}\u{042f}\u{97f3}\u{00e6}\u{221e}"
        );
    }
}
