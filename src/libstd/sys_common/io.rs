// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub const DEFAULT_BUF_SIZE: usize = 8 * 1024;

#[cfg(test)]
#[allow(dead_code)] // not used on emscripten
pub mod test {
    use path::{Path, PathBuf};
    use env;
    use rand::{self, RngCore};
    use fs;

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn join(&self, path: &str) -> PathBuf {
            let TempDir(ref p) = *self;
            p.join(path)
        }

        pub fn path<'a>(&'a self) -> &'a Path {
            let TempDir(ref p) = *self;
            p
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // Gee, seeing how we're testing the fs module I sure hope that we
            // at least implement this correctly!
            let TempDir(ref p) = *self;
            fs::remove_dir_all(p).unwrap();
        }
    }

    pub fn tmpdir() -> TempDir {
        let p = env::temp_dir();
        let mut r = rand::thread_rng();
        let ret = p.join(&format!("rust-{}", r.next_u32()));
        fs::create_dir(&ret).unwrap();
        TempDir(ret)
    }
}
