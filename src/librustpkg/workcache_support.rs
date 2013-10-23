// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use extra::sha1::Sha1;
use extra::digest::Digest;
use extra::workcache;
use std::io;

/// Hashes the file contents along with the last-modified time
pub fn digest_file_with_date(path: &Path) -> ~str {
    use conditions::bad_path::cond;
    use cond1 = conditions::bad_stat::cond;

    let s = io::read_whole_file_str(path);
    match s {
        Ok(s) => {
            let mut sha = Sha1::new();
            sha.input_str(s);
            let st = match path.stat() {
                Some(st) => st,
                None => cond1.raise((path.clone(), format!("Couldn't get file access time")))
            };
            sha.input_str(st.modified.to_str());
            sha.result_str()
        }
        Err(e) => {
            let path = cond.raise((path.clone(), format!("Couldn't read file: {}", e)));
            // FIXME (#9639): This needs to handle non-utf8 paths
            // XXX: I'm pretty sure this is the wrong return value
            path.as_str().unwrap().to_owned()
        }
    }
}

/// Hashes only the last-modified time
pub fn digest_only_date(path: &Path) -> ~str {
    use cond = conditions::bad_stat::cond;

    let mut sha = Sha1::new();
    let st = match path.stat() {
                Some(st) => st,
                None => cond.raise((path.clone(), format!("Couldn't get file access time")))
    };
    sha.input_str(st.modified.to_str());
    sha.result_str()
}

/// Adds multiple discovered outputs
pub fn discover_outputs(e: &mut workcache::Exec, outputs: ~[Path]) {
    debug!("Discovering {:?} outputs", outputs.len());
    for p in outputs.iter() {
        debug!("Discovering output! {}", p.display());
        // For now, assume that all discovered outputs are binaries
        // FIXME (#9639): This needs to handle non-utf8 paths
        e.discover_output("binary", p.as_str().unwrap(), digest_only_date(p));
    }
}

/// Returns the function name for building a crate
pub fn crate_tag(p: &Path) -> ~str {
    // FIXME (#9639): This needs to handle non-utf8 paths
    p.as_str().unwrap().to_owned() // implicitly, it's "build(p)"...
}
