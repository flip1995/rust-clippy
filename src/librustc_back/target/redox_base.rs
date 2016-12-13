// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use PanicStrategy;
use target::TargetOptions;
use std::default::Default;

pub fn opts() -> TargetOptions {
    TargetOptions {
        pre_link_args: vec![
            // We want to be able to strip as much executable code as possible
            // from the linker command line, and this flag indicates to the
            // linker that it can avoid linking in dynamic libraries that don't
            // actually satisfy any symbols up to that point (as with many other
            // resolutions the linker does). This option only applies to all
            // following libraries so we're sure to pass it as one of the first
            // arguments.
            "-Wl,--as-needed".to_string(),

            // Always enable NX protection when it is available
            "-Wl,-z,noexecstack".to_string(),

            // Do not link libc
            "-nostdlib".to_string(),

            // Static link
            "-static".to_string()
        ],
        executables: true,
        relocation_model: "static".to_string(),
        disable_redzone: true,
        eliminate_frame_pointer: false,
        target_family: Some("redox".to_string()),
        linker_is_gnu: true,
        no_default_libraries: true,
        has_elf_tls: true,
        panic_strategy: PanicStrategy::Abort,
        .. Default::default()
    }
}
