// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ffi::CStr;
use io;
use libc::{c_char, c_ulong};
use mem;
use sys::backtrace::BacktraceContext;
use sys::backtrace::StackWalkVariant;
use sys::c;
use sys::dynamic_lib::DynamicLibrary;
use sys_common::backtrace::Frame;

// Structs holding printing functions and loaders for them
// Two versions depending on whether dbghelp.dll has StackWalkEx or not
// (the former being in newer Windows versions, the older being in Win7 and before)
pub struct PrintingFnsEx {
    resolve_symname: SymFromInlineContextFn,
    sym_get_line: SymGetLineFromInlineContextFn,
}
pub struct PrintingFns64 {
    resolve_symname: SymFromAddrFn,
    sym_get_line: SymGetLineFromAddr64Fn,
}

pub fn load_printing_fns_ex(dbghelp: &DynamicLibrary) -> io::Result<PrintingFnsEx> {
    Ok(PrintingFnsEx {
        resolve_symname: sym!(dbghelp, "SymFromInlineContext", SymFromInlineContextFn)?,
        sym_get_line: sym!(
            dbghelp,
            "SymGetLineFromInlineContext",
            SymGetLineFromInlineContextFn
        )?,
    })
}
pub fn load_printing_fns_64(dbghelp: &DynamicLibrary) -> io::Result<PrintingFns64> {
    Ok(PrintingFns64 {
        resolve_symname: sym!(dbghelp, "SymFromAddr", SymFromAddrFn)?,
        sym_get_line: sym!(dbghelp, "SymGetLineFromAddr64", SymGetLineFromAddr64Fn)?,
    })
}

type SymFromAddrFn =
    unsafe extern "system" fn(c::HANDLE, u64, *mut u64, *mut c::SYMBOL_INFO) -> c::BOOL;
type SymFromInlineContextFn =
    unsafe extern "system" fn(c::HANDLE, u64, c::ULONG, *mut u64, *mut c::SYMBOL_INFO) -> c::BOOL;

type SymGetLineFromAddr64Fn =
    unsafe extern "system" fn(c::HANDLE, u64, *mut u32, *mut c::IMAGEHLP_LINE64) -> c::BOOL;
type SymGetLineFromInlineContextFn = unsafe extern "system" fn(
    c::HANDLE,
    u64,
    c::ULONG,
    u64,
    *mut c::DWORD,
    *mut c::IMAGEHLP_LINE64,
) -> c::BOOL;

/// Converts a pointer to symbol to its string value.
pub fn resolve_symname<F>(frame: Frame, callback: F, context: &BacktraceContext) -> io::Result<()>
where
    F: FnOnce(Option<&str>) -> io::Result<()>,
{
    match context.StackWalkVariant {
        StackWalkVariant::StackWalkEx(_, ref fns) => {
            resolve_symname_internal(fns.resolve_symname, frame, callback, context)
        }
        StackWalkVariant::StackWalk64(_, ref fns) => {
            resolve_symname_internal(fns.resolve_symname, frame, callback, context)
        }
    }
}

fn resolve_symname_internal<F, R>(
    symbol_resolver: R,
    frame: Frame,
    callback: F,
    context: &BacktraceContext,
) -> io::Result<()>
where
    F: FnOnce(Option<&str>) -> io::Result<()>,
    R: SymbolResolver,
{
    unsafe {
        let mut info: c::SYMBOL_INFO = mem::zeroed();
        info.MaxNameLen = c::MAX_SYM_NAME as c_ulong;
        // the struct size in C.  the value is different to
        // `size_of::<SYMBOL_INFO>() - MAX_SYM_NAME + 1` (== 81)
        // due to struct alignment.
        info.SizeOfStruct = 88;

        let ret = symbol_resolver.resolve_symbol(
            context.handle,
            frame.symbol_addr as u64,
            frame.inline_context,
            &mut info,
        );
        let valid_range = if ret == c::TRUE && frame.symbol_addr as usize >= info.Address as usize {
            if info.Size != 0 {
                (frame.symbol_addr as usize) < info.Address as usize + info.Size as usize
            } else {
                true
            }
        } else {
            false
        };
        let symname = if valid_range {
            let ptr = info.Name.as_ptr() as *const c_char;
            CStr::from_ptr(ptr).to_str().ok()
        } else {
            None
        };
        callback(symname)
    }
}

trait SymbolResolver {
    fn resolve_symbol(
        &self,
        process: c::HANDLE,
        symbol_address: u64,
        inline_context: c::ULONG,
        info: *mut c::SYMBOL_INFO,
    ) -> c::BOOL;
}

impl SymbolResolver for SymFromAddrFn {
    fn resolve_symbol(
        &self,
        process: c::HANDLE,
        symbol_address: u64,
        _inline_context: c::ULONG,
        info: *mut c::SYMBOL_INFO,
    ) -> c::BOOL {
        unsafe {
            let mut displacement = 0u64;
            self(process, symbol_address, &mut displacement, info)
        }
    }
}

impl SymbolResolver for SymFromInlineContextFn {
    fn resolve_symbol(
        &self,
        process: c::HANDLE,
        symbol_address: u64,
        inline_context: c::ULONG,
        info: *mut c::SYMBOL_INFO,
    ) -> c::BOOL {
        unsafe {
            let mut displacement = 0u64;
            self(
                process,
                symbol_address,
                inline_context,
                &mut displacement,
                info,
            )
        }
    }
}

pub fn foreach_symbol_fileline<F>(
    frame: Frame,
    callback: F,
    context: &BacktraceContext,
) -> io::Result<bool>
where
    F: FnMut(&[u8], u32) -> io::Result<()>,
{
    match context.StackWalkVariant {
        StackWalkVariant::StackWalkEx(_, ref fns) => {
            foreach_symbol_fileline_iternal(fns.sym_get_line, frame, callback, context)
        }
        StackWalkVariant::StackWalk64(_, ref fns) => {
            foreach_symbol_fileline_iternal(fns.sym_get_line, frame, callback, context)
        }
    }
}

fn foreach_symbol_fileline_iternal<F, G>(
    line_getter: G,
    frame: Frame,
    mut callback: F,
    context: &BacktraceContext,
) -> io::Result<bool>
where
    F: FnMut(&[u8], u32) -> io::Result<()>,
    G: LineGetter,
{
    unsafe {
        let mut line: c::IMAGEHLP_LINE64 = mem::zeroed();
        line.SizeOfStruct = ::mem::size_of::<c::IMAGEHLP_LINE64>() as u32;

        let ret = line_getter.get_line(
            context.handle,
            frame.exact_position as u64,
            frame.inline_context,
            &mut line,
        );
        if ret == c::TRUE {
            let name = CStr::from_ptr(line.Filename).to_bytes();
            callback(name, line.LineNumber as u32)?;
        }
        Ok(false)
    }
}

trait LineGetter {
    fn get_line(
        &self,
        process: c::HANDLE,
        frame_address: u64,
        inline_context: c::ULONG,
        line: *mut c::IMAGEHLP_LINE64,
    ) -> c::BOOL;
}

impl LineGetter for SymGetLineFromAddr64Fn {
    fn get_line(
        &self,
        process: c::HANDLE,
        frame_address: u64,
        _inline_context: c::ULONG,
        line: *mut c::IMAGEHLP_LINE64,
    ) -> c::BOOL {
        unsafe {
            let mut displacement = 0u32;
            self(process, frame_address, &mut displacement, line)
        }
    }
}

impl LineGetter for SymGetLineFromInlineContextFn {
    fn get_line(
        &self,
        process: c::HANDLE,
        frame_address: u64,
        inline_context: c::ULONG,
        line: *mut c::IMAGEHLP_LINE64,
    ) -> c::BOOL {
        unsafe {
            let mut displacement = 0u32;
            self(
                process,
                frame_address,
                inline_context,
                0,
                &mut displacement,
                line,
            )
        }
    }
}
