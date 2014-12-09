// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate libc;

use std::io::process::Command;
use std::iter::IteratorExt;

use libc::funcs::posix88::unistd;


// The output from "ps -A -o pid,ppid,args" should look like this:
//   PID  PPID COMMAND
//     1     0 /sbin/init
//     2     0 [kthreadd]
// ...
//  6076  9064 /bin/zsh
// ...
//  7164  6076 ./spawn-failure
//  7165  7164 [spawn-failure] <defunct>
//  7166  7164 [spawn-failure] <defunct>
// ...
//  7197  7164 [spawn-failure] <defunct>
//  7198  7164 ps -A -o pid,ppid,command
// ...

#[cfg(unix)]
fn find_zombies() {
    let my_pid = unsafe { unistd::getpid() };

    // http://pubs.opengroup.org/onlinepubs/9699919799/utilities/ps.html
    let ps_cmd_output = Command::new("ps").args(&["-A", "-o", "pid,ppid,args"]).output().unwrap();
    let ps_output = String::from_utf8_lossy(ps_cmd_output.output.as_slice());

    for (line_no, line) in ps_output.split('\n').enumerate() {
        if 0 < line_no && 0 < line.len() &&
           my_pid == from_str(line.split(' ').filter(|w| 0 < w.len()).nth(1)
               .expect("1st column should be PPID")
               ).expect("PPID string into integer") &&
           line.contains("defunct") {
            panic!("Zombie child {}", line);
        }
    }
}

#[cfg(windows)]
fn find_zombies() { }

fn main() {
    let too_long = format!("/NoSuchCommand{:0300}", 0u8);

    let _failures = Vec::from_fn(100, |_i| {
        let cmd = Command::new(too_long.as_slice());
        let failed = cmd.spawn();
        assert!(failed.is_err(), "Make sure the command fails to spawn(): {}", cmd);
        failed
    });

    find_zombies();
    // then _failures goes out of scope
}
