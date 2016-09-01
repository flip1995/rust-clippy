// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::mem;
use std::io::prelude::*;
use std::io::{self, SeekFrom, Cursor};

use rustc_serialize::opaque;

pub type EncodeResult = io::Result<()>;

// rbml writing
pub struct Encoder<'a> {
    pub opaque: opaque::Encoder<'a>,
    size_positions: Vec<usize>,
    relax_limit: usize, // do not move encoded bytes before this position
}

const NUM_TAGS: usize = 0x1000;

fn write_tag<W: Write>(w: &mut W, n: usize) -> EncodeResult {
    if n < 0xf0 {
        w.write_all(&[n as u8])
    } else if 0x100 <= n && n < NUM_TAGS {
        w.write_all(&[0xf0 | (n >> 8) as u8, n as u8])
    } else {
        Err(io::Error::new(io::ErrorKind::Other, &format!("invalid tag: {}", n)[..]))
    }
}

fn write_sized_vuint<W: Write>(w: &mut W, n: usize, size: usize) -> EncodeResult {
    match size {
        1 => w.write_all(&[0x80 | (n as u8)]),
        2 => w.write_all(&[0x40 | ((n >> 8) as u8), n as u8]),
        3 => w.write_all(&[0x20 | ((n >> 16) as u8), (n >> 8) as u8, n as u8]),
        4 => w.write_all(&[0x10 | ((n >> 24) as u8), (n >> 16) as u8, (n >> 8) as u8, n as u8]),
        _ => Err(io::Error::new(io::ErrorKind::Other, &format!("isize too big: {}", n)[..])),
    }
}

pub fn write_vuint<W: Write>(w: &mut W, n: usize) -> EncodeResult {
    if n < 0x7f {
        return write_sized_vuint(w, n, 1);
    }
    if n < 0x4000 {
        return write_sized_vuint(w, n, 2);
    }
    if n < 0x200000 {
        return write_sized_vuint(w, n, 3);
    }
    if n < 0x10000000 {
        return write_sized_vuint(w, n, 4);
    }
    Err(io::Error::new(io::ErrorKind::Other, &format!("isize too big: {}", n)[..]))
}

impl<'a> Encoder<'a> {
    pub fn new(cursor: &'a mut Cursor<Vec<u8>>) -> Encoder<'a> {
        Encoder {
            opaque: opaque::Encoder::new(cursor),
            size_positions: vec![],
            relax_limit: 0,
        }
    }

    pub fn start_tag(&mut self, tag_id: usize) -> EncodeResult {
        debug!("Start tag {:?}", tag_id);

        // Write the enum ID:
        write_tag(&mut self.opaque.cursor, tag_id)?;

        // Write a placeholder four-byte size.
        let cur_pos = self.position();
        self.size_positions.push(cur_pos);
        self.opaque.cursor.write_all(&[0, 0, 0, 0])
    }

    pub fn end_tag(&mut self) -> EncodeResult {
        let last_size_pos = self.size_positions.pop().unwrap();
        let cur_pos = self.position();
        self.opaque.cursor.seek(SeekFrom::Start(last_size_pos as u64))?;
        let size = cur_pos - last_size_pos - 4;

        // relax the size encoding for small tags (bigger tags are costly to move).
        // we should never try to move the stable positions, however.
        const RELAX_MAX_SIZE: usize = 0x100;
        if size <= RELAX_MAX_SIZE && last_size_pos >= self.relax_limit {
            // we can't alter the buffer in place, so have a temporary buffer
            let mut buf = [0u8; RELAX_MAX_SIZE];
            {
                let data = &self.opaque.cursor.get_ref()[last_size_pos + 4..cur_pos];
                buf[..size].copy_from_slice(data);
            }

            // overwrite the size and data and continue
            write_vuint(&mut self.opaque.cursor, size)?;
            self.opaque.cursor.write_all(&buf[..size])?;
        } else {
            // overwrite the size with an overlong encoding and skip past the data
            write_sized_vuint(&mut self.opaque.cursor, size, 4)?;
            self.opaque.cursor.seek(SeekFrom::Start(cur_pos as u64))?;
        }

        debug!("End tag (size = {:?})", size);
        Ok(())
    }

    pub fn wr_tag<F>(&mut self, tag_id: usize, blk: F) -> EncodeResult
        where F: FnOnce() -> EncodeResult
    {
        self.start_tag(tag_id)?;
        blk()?;
        self.end_tag()
    }

    pub fn wr_tagged_bytes(&mut self, tag_id: usize, b: &[u8]) -> EncodeResult {
        write_tag(&mut self.opaque.cursor, tag_id)?;
        write_vuint(&mut self.opaque.cursor, b.len())?;
        self.opaque.cursor.write_all(b)
    }

    pub fn wr_tagged_u64(&mut self, tag_id: usize, v: u64) -> EncodeResult {
        let bytes: [u8; 8] = unsafe { mem::transmute(v.to_be()) };
        // tagged integers are emitted in big-endian, with no
        // leading zeros.
        let leading_zero_bytes = v.leading_zeros() / 8;
        self.wr_tagged_bytes(tag_id, &bytes[leading_zero_bytes as usize..])
    }

    #[inline]
    pub fn wr_tagged_u32(&mut self, tag_id: usize, v: u32) -> EncodeResult {
        self.wr_tagged_u64(tag_id, v as u64)
    }

    #[inline]
    pub fn wr_tagged_u8(&mut self, tag_id: usize, v: u8) -> EncodeResult {
        self.wr_tagged_bytes(tag_id, &[v])
    }

    pub fn wr_tagged_str(&mut self, tag_id: usize, v: &str) -> EncodeResult {
        self.wr_tagged_bytes(tag_id, v.as_bytes())
    }

    pub fn wr_bytes(&mut self, b: &[u8]) -> EncodeResult {
        debug!("Write {:?} bytes", b.len());
        self.opaque.cursor.write_all(b)
    }

    pub fn wr_str(&mut self, s: &str) -> EncodeResult {
        debug!("Write str: {:?}", s);
        self.opaque.cursor.write_all(s.as_bytes())
    }

    pub fn position(&mut self) -> usize {
        self.opaque.position() as usize
    }

    /// Returns the current position while marking it stable, i.e.
    /// generated bytes so far wouldn't be affected by relaxation.
    pub fn mark_stable_position(&mut self) -> usize {
        let pos = self.position();
        if self.relax_limit < pos {
            self.relax_limit = pos;
        }
        let meta_start = 8 + ::common::metadata_encoding_version.len();
        pos - meta_start
    }
}
