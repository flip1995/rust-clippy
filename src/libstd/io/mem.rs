// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Readers and Writers for in-memory buffers
//!
//! # XXX
//!
//! * Should probably have something like this for strings.
//! * Should they implement Closable? Would take extra state.

use cmp::min;
use prelude::*;
use super::*;
use vec;

/// Writes to an owned, growable byte vector
pub struct MemWriter {
    priv buf: ~[u8],
    priv pos: uint,
}

impl MemWriter {
    pub fn new() -> MemWriter {
        MemWriter { buf: vec::with_capacity(128), pos: 0 }
    }
}

impl Writer for MemWriter {
    fn write(&mut self, buf: &[u8]) {
        // Make sure the internal buffer is as least as big as where we
        // currently are
        let difference = self.pos as i64 - self.buf.len() as i64;
        if difference > 0 {
            self.buf.grow(difference as uint, &0);
        }

        // Figure out what bytes will be used to overwrite what's currently
        // there (left), and what will be appended on the end (right)
        let cap = self.buf.len() - self.pos;
        let (left, right) = if cap <= buf.len() {
            (buf.slice_to(cap), buf.slice_from(cap))
        } else {
            (buf, &[])
        };

        // Do the necessary writes
        if left.len() > 0 {
            vec::bytes::copy_memory(self.buf.mut_slice_from(self.pos),
                                    left, left.len());
        }
        if right.len() > 0 {
            self.buf.push_all(right);
        }

        // Bump us forward
        self.pos += buf.len();
    }
}

impl Seek for MemWriter {
    fn tell(&self) -> u64 { self.pos as u64 }

    fn seek(&mut self, pos: i64, style: SeekStyle) {
        match style {
            SeekSet => { self.pos = pos as uint; }
            SeekEnd => { self.pos = self.buf.len() + pos as uint; }
            SeekCur => { self.pos += pos as uint; }
        }
    }
}

impl Decorator<~[u8]> for MemWriter {
    fn inner(self) -> ~[u8] { self.buf }
    fn inner_ref<'a>(&'a self) -> &'a ~[u8] { &self.buf }
    fn inner_mut_ref<'a>(&'a mut self) -> &'a mut ~[u8] { &mut self.buf }
}

/// Reads from an owned byte vector
pub struct MemReader {
    priv buf: ~[u8],
    priv pos: uint
}

impl MemReader {
    pub fn new(buf: ~[u8]) -> MemReader {
        MemReader {
            buf: buf,
            pos: 0
        }
    }
}

impl Reader for MemReader {
    fn read(&mut self, buf: &mut [u8]) -> Option<uint> {
        { if self.eof() { return None; } }

        let write_len = min(buf.len(), self.buf.len() - self.pos);
        {
            let input = self.buf.slice(self.pos, self.pos + write_len);
            let output = buf.mut_slice(0, write_len);
            assert_eq!(input.len(), output.len());
            vec::bytes::copy_memory(output, input, write_len);
        }
        self.pos += write_len;
        assert!(self.pos <= self.buf.len());

        return Some(write_len);
    }

    fn eof(&mut self) -> bool { self.pos == self.buf.len() }
}

impl Seek for MemReader {
    fn tell(&self) -> u64 { self.pos as u64 }
    fn seek(&mut self, _pos: i64, _style: SeekStyle) { fail!() }
}

impl Buffer for MemReader {
    fn fill<'a>(&'a mut self) -> &'a [u8] { self.buf.slice_from(self.pos) }
    fn consume(&mut self, amt: uint) { self.pos += amt; }
}

impl Decorator<~[u8]> for MemReader {
    fn inner(self) -> ~[u8] { self.buf }
    fn inner_ref<'a>(&'a self) -> &'a ~[u8] { &self.buf }
    fn inner_mut_ref<'a>(&'a mut self) -> &'a mut ~[u8] { &mut self.buf }
}


/// Writes to a fixed-size byte slice
pub struct BufWriter<'self> {
    priv buf: &'self mut [u8],
    priv pos: uint
}

impl<'self> BufWriter<'self> {
    pub fn new<'a>(buf: &'a mut [u8]) -> BufWriter<'a> {
        BufWriter {
            buf: buf,
            pos: 0
        }
    }
}

impl<'self> Writer for BufWriter<'self> {
    fn write(&mut self, _buf: &[u8]) { fail!() }

    fn flush(&mut self) { fail!() }
}

impl<'self> Seek for BufWriter<'self> {
    fn tell(&self) -> u64 { fail!() }

    fn seek(&mut self, _pos: i64, _style: SeekStyle) { fail!() }
}


/// Reads from a fixed-size byte slice
pub struct BufReader<'self> {
    priv buf: &'self [u8],
    priv pos: uint
}

impl<'self> BufReader<'self> {
    pub fn new<'a>(buf: &'a [u8]) -> BufReader<'a> {
        BufReader {
            buf: buf,
            pos: 0
        }
    }
}

impl<'self> Reader for BufReader<'self> {
    fn read(&mut self, buf: &mut [u8]) -> Option<uint> {
        { if self.eof() { return None; } }

        let write_len = min(buf.len(), self.buf.len() - self.pos);
        {
            let input = self.buf.slice(self.pos, self.pos + write_len);
            let output = buf.mut_slice(0, write_len);
            assert_eq!(input.len(), output.len());
            vec::bytes::copy_memory(output, input, write_len);
        }
        self.pos += write_len;
        assert!(self.pos <= self.buf.len());

        return Some(write_len);
     }

    fn eof(&mut self) -> bool { self.pos == self.buf.len() }
}

impl<'self> Seek for BufReader<'self> {
    fn tell(&self) -> u64 { self.pos as u64 }

    fn seek(&mut self, _pos: i64, _style: SeekStyle) { fail!() }
}

impl<'self> Buffer for BufReader<'self> {
    fn fill<'a>(&'a mut self) -> &'a [u8] { self.buf.slice_from(self.pos) }
    fn consume(&mut self, amt: uint) { self.pos += amt; }
}

///Calls a function with a MemWriter and returns
///the writer's stored vector.
pub fn with_mem_writer(writeFn:&fn(&mut MemWriter)) -> ~[u8] {
    let mut writer = MemWriter::new();
    writeFn(&mut writer);
    writer.inner()
}

#[cfg(test)]
mod test {
    use prelude::*;
    use super::*;
    use io::*;

    #[test]
    fn test_mem_writer() {
        let mut writer = MemWriter::new();
        assert_eq!(writer.tell(), 0);
        writer.write([0]);
        assert_eq!(writer.tell(), 1);
        writer.write([1, 2, 3]);
        writer.write([4, 5, 6, 7]);
        assert_eq!(writer.tell(), 8);
        assert_eq!(*writer.inner_ref(), ~[0, 1, 2, 3, 4, 5, 6, 7]);

        writer.seek(0, SeekSet);
        assert_eq!(writer.tell(), 0);
        writer.write([3, 4]);
        assert_eq!(*writer.inner_ref(), ~[3, 4, 2, 3, 4, 5, 6, 7]);

        writer.seek(1, SeekCur);
        writer.write([0, 1]);
        assert_eq!(*writer.inner_ref(), ~[3, 4, 2, 0, 1, 5, 6, 7]);

        writer.seek(-1, SeekEnd);
        writer.write([1, 2]);
        assert_eq!(*writer.inner_ref(), ~[3, 4, 2, 0, 1, 5, 6, 1, 2]);

        writer.seek(1, SeekEnd);
        writer.write([1]);
        assert_eq!(*writer.inner_ref(), ~[3, 4, 2, 0, 1, 5, 6, 1, 2, 0, 1]);
    }

    #[test]
    fn test_mem_reader() {
        let mut reader = MemReader::new(~[0, 1, 2, 3, 4, 5, 6, 7]);
        let mut buf = [];
        assert_eq!(reader.read(buf), Some(0));
        assert_eq!(reader.tell(), 0);
        let mut buf = [0];
        assert_eq!(reader.read(buf), Some(1));
        assert_eq!(reader.tell(), 1);
        assert_eq!(buf, [0]);
        let mut buf = [0, ..4];
        assert_eq!(reader.read(buf), Some(4));
        assert_eq!(reader.tell(), 5);
        assert_eq!(buf, [1, 2, 3, 4]);
        assert_eq!(reader.read(buf), Some(3));
        assert_eq!(buf.slice(0, 3), [5, 6, 7]);
        assert!(reader.eof());
        assert_eq!(reader.read(buf), None);
        assert!(reader.eof());
    }

    #[test]
    fn test_buf_reader() {
        let in_buf = ~[0, 1, 2, 3, 4, 5, 6, 7];
        let mut reader = BufReader::new(in_buf);
        let mut buf = [];
        assert_eq!(reader.read(buf), Some(0));
        assert_eq!(reader.tell(), 0);
        let mut buf = [0];
        assert_eq!(reader.read(buf), Some(1));
        assert_eq!(reader.tell(), 1);
        assert_eq!(buf, [0]);
        let mut buf = [0, ..4];
        assert_eq!(reader.read(buf), Some(4));
        assert_eq!(reader.tell(), 5);
        assert_eq!(buf, [1, 2, 3, 4]);
        assert_eq!(reader.read(buf), Some(3));
        assert_eq!(buf.slice(0, 3), [5, 6, 7]);
        assert!(reader.eof());
        assert_eq!(reader.read(buf), None);
        assert!(reader.eof());
    }

    #[test]
    fn test_with_mem_writer() {
        let buf = with_mem_writer(|wr| wr.write([1,2,3,4,5,6,7]));
        assert_eq!(buf, ~[1,2,3,4,5,6,7]);
    }

    #[test]
    fn test_read_char() {
        let mut r = BufReader::new(bytes!("Việt"));
        assert_eq!(r.read_char(), Some('V'));
        assert_eq!(r.read_char(), Some('i'));
        assert_eq!(r.read_char(), Some('ệ'));
        assert_eq!(r.read_char(), Some('t'));
        assert_eq!(r.read_char(), None);
    }

    #[test]
    fn test_read_bad_char() {
        let mut r = BufReader::new(bytes!(0x80));
        assert_eq!(r.read_char(), None);
    }
}
