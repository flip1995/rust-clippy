// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A thin wrapper around `Command` in the standard library which allows us to
//! read the arguments that are built up.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::mem;
use std::process::{self, Output};

#[derive(Clone)]
pub struct Command {
    program: Program,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

#[derive(Clone)]
enum Program {
    Normal(OsString),
    CmdBatScript(OsString),
}

impl Command {
    pub fn new<P: AsRef<OsStr>>(program: P) -> Command {
        Command::_new(Program::Normal(program.as_ref().to_owned()))
    }

    pub fn bat_script<P: AsRef<OsStr>>(program: P) -> Command {
        Command::_new(Program::CmdBatScript(program.as_ref().to_owned()))
    }

    fn _new(program: Program) -> Command {
        Command {
            program,
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    pub fn arg<P: AsRef<OsStr>>(&mut self, arg: P) -> &mut Command {
        self._arg(arg.as_ref());
        self
    }

    pub fn args<I>(&mut self, args: I) -> &mut Command
        where I: IntoIterator,
              I::Item: AsRef<OsStr>,
    {
        for arg in args {
            self._arg(arg.as_ref());
        }
        self
    }

    fn _arg(&mut self, arg: &OsStr) {
        self.args.push(arg.to_owned());
    }

    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut Command
        where K: AsRef<OsStr>,
              V: AsRef<OsStr>
    {
        self._env(key.as_ref(), value.as_ref());
        self
    }

    pub fn envs<I, K, V>(&mut self, envs: I) -> &mut Command
        where I: IntoIterator<Item=(K, V)>,
              K: AsRef<OsStr>,
              V: AsRef<OsStr>
    {
        for (key, value) in envs {
            self._env(key.as_ref(), value.as_ref());
        }
        self
    }

    fn _env(&mut self, key: &OsStr, value: &OsStr) {
        self.env.push((key.to_owned(), value.to_owned()));
    }

    pub fn output(&mut self) -> io::Result<Output> {
        self.command().output()
    }

    pub fn command(&self) -> process::Command {
        let mut ret = match self.program {
            Program::Normal(ref p) => process::Command::new(p),
            Program::CmdBatScript(ref p) => {
                let mut c = process::Command::new("cmd");
                c.arg("/c").arg(p);
                c
            }
        };
        ret.args(&self.args);
        ret.envs(self.env.clone());
        return ret
    }

    // extensions

    pub fn take_args(&mut self) -> Vec<OsString> {
        mem::replace(&mut self.args, Vec::new())
    }

    /// Returns a `true` if we're pretty sure that this'll blow OS spawn limits,
    /// or `false` if we should attempt to spawn and see what the OS says.
    pub fn very_likely_to_exceed_some_spawn_limit(&self) -> bool {
        // We mostly only care about Windows in this method, on Unix the limits
        // can be gargantuan anyway so we're pretty unlikely to hit them
        if cfg!(unix) {
            return false
        }

        // Ok so on Windows to spawn a process is 32,768 characters in its
        // command line [1]. Unfortunately we don't actually have access to that
        // as it's calculated just before spawning. Instead we perform a
        // poor-man's guess as to how long our command line will be. We're
        // assuming here that we don't have to escape every character...
        //
        // Turns out though that `cmd.exe` has even smaller limits, 8192
        // characters [2]. Linkers can often be batch scripts (for example
        // Emscripten, Gecko's current build system) which means that we're
        // running through batch scripts. These linkers often just forward
        // arguments elsewhere (and maybe tack on more), so if we blow 8192
        // bytes we'll typically cause them to blow as well.
        //
        // Basically as a result just perform an inflated estimate of what our
        // command line will look like and test if it's > 8192 (we actually
        // test against 6k to artificially inflate our estimate). If all else
        // fails we'll fall back to the normal unix logic of testing the OS
        // error code if we fail to spawn and automatically re-spawning the
        // linker with smaller arguments.
        //
        // [1]: https://msdn.microsoft.com/en-us/library/windows/desktop/ms682425(v=vs.85).aspx
        // [2]: https://blogs.msdn.microsoft.com/oldnewthing/20031210-00/?p=41553

        let estimated_command_line_len =
            self.args.iter().map(|a| a.len()).sum::<usize>();
        estimated_command_line_len > 1024 * 6
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.command().fmt(f)
    }
}
