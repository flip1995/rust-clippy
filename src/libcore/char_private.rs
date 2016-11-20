// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// NOTE: The following code was generated by "src/etc/char_private.py",
//       do not edit directly!

use slice::SliceExt;

fn check(x: u16, singletons: &[u16], normal: &[u16]) -> bool {
    for &s in singletons {
        if x == s {
            return false;
        } else if x < s {
            break;
        }
    }
    for w in normal.chunks(2) {
        let start = w[0];
        let len = w[1];
        let difference = (x as i32) - (start as i32);
        if 0 <= difference {
            if difference < len as i32 {
                return false;
            }
        } else {
            break;
        }
    }
    true
}

pub fn is_printable(x: char) -> bool {
    let x = x as u32;
    let lower = x as u16;
    if x < 0x10000 {
        check(lower, SINGLETONS0, NORMAL0)
    } else if x < 0x20000 {
        check(lower, SINGLETONS1, NORMAL1)
    } else {
        if 0x2a6d7 <= x && x < 0x2a700 {
            return false;
        }
        if 0x2b735 <= x && x < 0x2b740 {
            return false;
        }
        if 0x2b81e <= x && x < 0x2b820 {
            return false;
        }
        if 0x2cea2 <= x && x < 0x2f800 {
            return false;
        }
        if 0x2fa1e <= x && x < 0xe0100 {
            return false;
        }
        if 0xe01f0 <= x && x < 0x110000 {
            return false;
        }
        true
    }
}

const SINGLETONS0: &'static [u16] = &[
    0xad,
    0x378,
    0x379,
    0x38b,
    0x38d,
    0x3a2,
    0x530,
    0x557,
    0x558,
    0x560,
    0x588,
    0x58b,
    0x58c,
    0x590,
    0x61c,
    0x61d,
    0x6dd,
    0x70e,
    0x70f,
    0x74b,
    0x74c,
    0x82e,
    0x82f,
    0x83f,
    0x85c,
    0x85d,
    0x8b5,
    0x8e2,
    0x984,
    0x98d,
    0x98e,
    0x991,
    0x992,
    0x9a9,
    0x9b1,
    0x9ba,
    0x9bb,
    0x9c5,
    0x9c6,
    0x9c9,
    0x9ca,
    0x9de,
    0x9e4,
    0x9e5,
    0xa04,
    0xa11,
    0xa12,
    0xa29,
    0xa31,
    0xa34,
    0xa37,
    0xa3a,
    0xa3b,
    0xa3d,
    0xa49,
    0xa4a,
    0xa5d,
    0xa84,
    0xa8e,
    0xa92,
    0xaa9,
    0xab1,
    0xab4,
    0xaba,
    0xabb,
    0xac6,
    0xaca,
    0xace,
    0xacf,
    0xae4,
    0xae5,
    0xb04,
    0xb0d,
    0xb0e,
    0xb11,
    0xb12,
    0xb29,
    0xb31,
    0xb34,
    0xb3a,
    0xb3b,
    0xb45,
    0xb46,
    0xb49,
    0xb4a,
    0xb5e,
    0xb64,
    0xb65,
    0xb84,
    0xb91,
    0xb9b,
    0xb9d,
    0xbc9,
    0xbce,
    0xbcf,
    0xc04,
    0xc0d,
    0xc11,
    0xc29,
    0xc45,
    0xc49,
    0xc57,
    0xc64,
    0xc65,
    0xc84,
    0xc8d,
    0xc91,
    0xca9,
    0xcb4,
    0xcba,
    0xcbb,
    0xcc5,
    0xcc9,
    0xcdf,
    0xce4,
    0xce5,
    0xcf0,
    0xd04,
    0xd0d,
    0xd11,
    0xd3b,
    0xd3c,
    0xd45,
    0xd49,
    0xd64,
    0xd65,
    0xd80,
    0xd81,
    0xd84,
    0xdb2,
    0xdbc,
    0xdbe,
    0xdbf,
    0xdd5,
    0xdd7,
    0xdf0,
    0xdf1,
    0xe83,
    0xe85,
    0xe86,
    0xe89,
    0xe8b,
    0xe8c,
    0xe98,
    0xea0,
    0xea4,
    0xea6,
    0xea8,
    0xea9,
    0xeac,
    0xeba,
    0xebe,
    0xebf,
    0xec5,
    0xec7,
    0xece,
    0xecf,
    0xeda,
    0xedb,
    0xf48,
    0xf98,
    0xfbd,
    0xfcd,
    0x10c6,
    0x10ce,
    0x10cf,
    0x1249,
    0x124e,
    0x124f,
    0x1257,
    0x1259,
    0x125e,
    0x125f,
    0x1289,
    0x128e,
    0x128f,
    0x12b1,
    0x12b6,
    0x12b7,
    0x12bf,
    0x12c1,
    0x12c6,
    0x12c7,
    0x12d7,
    0x1311,
    0x1316,
    0x1317,
    0x135b,
    0x135c,
    0x13f6,
    0x13f7,
    0x13fe,
    0x13ff,
    0x1680,
    0x170d,
    0x176d,
    0x1771,
    0x17de,
    0x17df,
    0x180e,
    0x180f,
    0x191f,
    0x196e,
    0x196f,
    0x1a1c,
    0x1a1d,
    0x1a5f,
    0x1a7d,
    0x1a7e,
    0x1aae,
    0x1aaf,
    0x1cf7,
    0x1f16,
    0x1f17,
    0x1f1e,
    0x1f1f,
    0x1f46,
    0x1f47,
    0x1f4e,
    0x1f4f,
    0x1f58,
    0x1f5a,
    0x1f5c,
    0x1f5e,
    0x1f7e,
    0x1f7f,
    0x1fb5,
    0x1fc5,
    0x1fd4,
    0x1fd5,
    0x1fdc,
    0x1ff0,
    0x1ff1,
    0x1ff5,
    0x2072,
    0x2073,
    0x208f,
    0x23ff,
    0x2b74,
    0x2b75,
    0x2b96,
    0x2b97,
    0x2bc9,
    0x2c2f,
    0x2c5f,
    0x2d26,
    0x2d2e,
    0x2d2f,
    0x2da7,
    0x2daf,
    0x2db7,
    0x2dbf,
    0x2dc7,
    0x2dcf,
    0x2dd7,
    0x2ddf,
    0x2e9a,
    0x3040,
    0x3097,
    0x3098,
    0x318f,
    0x321f,
    0x32ff,
    0xa7af,
    0xa8fe,
    0xa8ff,
    0xa9ce,
    0xa9ff,
    0xaa4e,
    0xaa4f,
    0xaa5a,
    0xaa5b,
    0xab07,
    0xab08,
    0xab0f,
    0xab10,
    0xab27,
    0xab2f,
    0xabee,
    0xabef,
    0xfa6e,
    0xfa6f,
    0xfb37,
    0xfb3d,
    0xfb3f,
    0xfb42,
    0xfb45,
    0xfd90,
    0xfd91,
    0xfdfe,
    0xfdff,
    0xfe53,
    0xfe67,
    0xfe75,
    0xffc8,
    0xffc9,
    0xffd0,
    0xffd1,
    0xffd8,
    0xffd9,
    0xffe7,
    0xfffe,
    0xffff,
];
const SINGLETONS1: &'static [u16] = &[
    0xc,
    0x27,
    0x3b,
    0x3e,
    0x4e,
    0x4f,
    0x18f,
    0x39e,
    0x49e,
    0x49f,
    0x806,
    0x807,
    0x809,
    0x836,
    0x83d,
    0x83e,
    0x856,
    0x8f3,
    0x9d0,
    0x9d1,
    0xa04,
    0xa14,
    0xa18,
    0xb56,
    0xb57,
    0x10bd,
    0x1135,
    0x11ce,
    0x11cf,
    0x11e0,
    0x1212,
    0x1287,
    0x1289,
    0x128e,
    0x129e,
    0x1304,
    0x130d,
    0x130e,
    0x1311,
    0x1312,
    0x1329,
    0x1331,
    0x1334,
    0x133a,
    0x133b,
    0x1345,
    0x1346,
    0x1349,
    0x134a,
    0x134e,
    0x134f,
    0x1364,
    0x1365,
    0x145a,
    0x145c,
    0x15b6,
    0x15b7,
    0x1c09,
    0x1c37,
    0x1c90,
    0x1c91,
    0x1ca8,
    0x246f,
    0x6a5f,
    0x6aee,
    0x6aef,
    0x6b5a,
    0x6b62,
    0xbc9a,
    0xbc9b,
    0xd127,
    0xd128,
    0xd455,
    0xd49d,
    0xd4a0,
    0xd4a1,
    0xd4a3,
    0xd4a4,
    0xd4a7,
    0xd4a8,
    0xd4ad,
    0xd4ba,
    0xd4bc,
    0xd4c4,
    0xd506,
    0xd50b,
    0xd50c,
    0xd515,
    0xd51d,
    0xd53a,
    0xd53f,
    0xd545,
    0xd551,
    0xd6a6,
    0xd6a7,
    0xd7cc,
    0xd7cd,
    0xdaa0,
    0xe007,
    0xe019,
    0xe01a,
    0xe022,
    0xe025,
    0xe8c5,
    0xe8c6,
    0xee04,
    0xee20,
    0xee23,
    0xee25,
    0xee26,
    0xee28,
    0xee33,
    0xee38,
    0xee3a,
    0xee48,
    0xee4a,
    0xee4c,
    0xee50,
    0xee53,
    0xee55,
    0xee56,
    0xee58,
    0xee5a,
    0xee5c,
    0xee5e,
    0xee60,
    0xee63,
    0xee65,
    0xee66,
    0xee6b,
    0xee73,
    0xee78,
    0xee7d,
    0xee7f,
    0xee8a,
    0xeea4,
    0xeeaa,
    0xf0af,
    0xf0b0,
    0xf0c0,
    0xf0d0,
    0xf12f,
    0xf91f,
    0xf931,
    0xf932,
    0xf93f,
];
const NORMAL0: &'static [u16] = &[
    0x0, 0x20,
    0x7f, 0x22,
    0x380, 0x4,
    0x5c8, 0x8,
    0x5eb, 0x5,
    0x5f5, 0x11,
    0x7b2, 0xe,
    0x7fb, 0x5,
    0x85f, 0x41,
    0x8be, 0x16,
    0x9b3, 0x3,
    0x9cf, 0x8,
    0x9d8, 0x4,
    0x9fc, 0x5,
    0xa0b, 0x4,
    0xa43, 0x4,
    0xa4e, 0x3,
    0xa52, 0x7,
    0xa5f, 0x7,
    0xa76, 0xb,
    0xad1, 0xf,
    0xaf2, 0x7,
    0xafa, 0x7,
    0xb4e, 0x8,
    0xb58, 0x4,
    0xb78, 0xa,
    0xb8b, 0x3,
    0xb96, 0x3,
    0xba0, 0x3,
    0xba5, 0x3,
    0xbab, 0x3,
    0xbba, 0x4,
    0xbc3, 0x3,
    0xbd1, 0x6,
    0xbd8, 0xe,
    0xbfb, 0x5,
    0xc3a, 0x3,
    0xc4e, 0x7,
    0xc5b, 0x5,
    0xc70, 0x8,
    0xcce, 0x7,
    0xcd7, 0x7,
    0xcf3, 0xe,
    0xd50, 0x4,
    0xd97, 0x3,
    0xdc7, 0x3,
    0xdcb, 0x4,
    0xde0, 0x6,
    0xdf5, 0xc,
    0xe3b, 0x4,
    0xe5c, 0x25,
    0xe8e, 0x6,
    0xee0, 0x20,
    0xf6d, 0x4,
    0xfdb, 0x25,
    0x10c8, 0x5,
    0x137d, 0x3,
    0x139a, 0x6,
    0x169d, 0x3,
    0x16f9, 0x7,
    0x1715, 0xb,
    0x1737, 0x9,
    0x1754, 0xc,
    0x1774, 0xc,
    0x17ea, 0x6,
    0x17fa, 0x6,
    0x181a, 0x6,
    0x1878, 0x8,
    0x18ab, 0x5,
    0x18f6, 0xa,
    0x192c, 0x4,
    0x193c, 0x4,
    0x1941, 0x3,
    0x1975, 0xb,
    0x19ac, 0x4,
    0x19ca, 0x6,
    0x19db, 0x3,
    0x1a8a, 0x6,
    0x1a9a, 0x6,
    0x1abf, 0x41,
    0x1b4c, 0x4,
    0x1b7d, 0x3,
    0x1bf4, 0x8,
    0x1c38, 0x3,
    0x1c4a, 0x3,
    0x1c89, 0x37,
    0x1cc8, 0x8,
    0x1cfa, 0x6,
    0x1df6, 0x5,
    0x1fff, 0x11,
    0x2028, 0x8,
    0x205f, 0x11,
    0x209d, 0x3,
    0x20bf, 0x11,
    0x20f1, 0xf,
    0x218c, 0x4,
    0x2427, 0x19,
    0x244b, 0x15,
    0x2bba, 0x3,
    0x2bd2, 0x1a,
    0x2bf0, 0x10,
    0x2cf4, 0x5,
    0x2d28, 0x5,
    0x2d68, 0x7,
    0x2d71, 0xe,
    0x2d97, 0x9,
    0x2e45, 0x3b,
    0x2ef4, 0xc,
    0x2fd6, 0x1a,
    0x2ffc, 0x5,
    0x3100, 0x5,
    0x312e, 0x3,
    0x31bb, 0x5,
    0x31e4, 0xc,
    0x4db6, 0xa,
    0x9fd6, 0x2a,
    0xa48d, 0x3,
    0xa4c7, 0x9,
    0xa62c, 0x14,
    0xa6f8, 0x8,
    0xa7b8, 0x3f,
    0xa82c, 0x4,
    0xa83a, 0x6,
    0xa878, 0x8,
    0xa8c6, 0x8,
    0xa8da, 0x6,
    0xa954, 0xb,
    0xa97d, 0x3,
    0xa9da, 0x4,
    0xaa37, 0x9,
    0xaac3, 0x18,
    0xaaf7, 0xa,
    0xab17, 0x9,
    0xab66, 0xa,
    0xabfa, 0x6,
    0xd7a4, 0xc,
    0xd7c7, 0x4,
    0xd7fc, 0x2104,
    0xfada, 0x26,
    0xfb07, 0xc,
    0xfb18, 0x5,
    0xfbc2, 0x11,
    0xfd40, 0x10,
    0xfdc8, 0x28,
    0xfe1a, 0x6,
    0xfe6c, 0x4,
    0xfefd, 0x4,
    0xffbf, 0x3,
    0xffdd, 0x3,
    0xffef, 0xd,
];
const NORMAL1: &'static [u16] = &[
    0x5e, 0x22,
    0xfb, 0x5,
    0x103, 0x4,
    0x134, 0x3,
    0x19c, 0x4,
    0x1a1, 0x2f,
    0x1fe, 0x82,
    0x29d, 0x3,
    0x2d1, 0xf,
    0x2fc, 0x4,
    0x324, 0xc,
    0x34b, 0x5,
    0x37b, 0x5,
    0x3c4, 0x4,
    0x3d6, 0x2a,
    0x4aa, 0x6,
    0x4d4, 0x4,
    0x4fc, 0x4,
    0x528, 0x8,
    0x564, 0xb,
    0x570, 0x90,
    0x737, 0x9,
    0x756, 0xa,
    0x768, 0x98,
    0x839, 0x3,
    0x89f, 0x8,
    0x8b0, 0x30,
    0x8f6, 0x5,
    0x91c, 0x3,
    0x93a, 0x5,
    0x940, 0x40,
    0x9b8, 0x4,
    0xa07, 0x5,
    0xa34, 0x4,
    0xa3b, 0x4,
    0xa48, 0x8,
    0xa59, 0x7,
    0xaa0, 0x20,
    0xae7, 0x4,
    0xaf7, 0x9,
    0xb36, 0x3,
    0xb73, 0x5,
    0xb92, 0x7,
    0xb9d, 0xc,
    0xbb0, 0x50,
    0xc49, 0x37,
    0xcb3, 0xd,
    0xcf3, 0x7,
    0xd00, 0x160,
    0xe7f, 0x181,
    0x104e, 0x4,
    0x1070, 0xf,
    0x10c2, 0xe,
    0x10e9, 0x7,
    0x10fa, 0x6,
    0x1144, 0xc,
    0x1177, 0x9,
    0x11f5, 0xb,
    0x123f, 0x41,
    0x12aa, 0x6,
    0x12eb, 0x5,
    0x12fa, 0x6,
    0x1351, 0x6,
    0x1358, 0x5,
    0x136d, 0x3,
    0x1375, 0x8b,
    0x145e, 0x22,
    0x14c8, 0x8,
    0x14da, 0xa6,
    0x15de, 0x22,
    0x1645, 0xb,
    0x165a, 0x6,
    0x166d, 0x13,
    0x16b8, 0x8,
    0x16ca, 0x36,
    0x171a, 0x3,
    0x172c, 0x4,
    0x1740, 0x160,
    0x18f3, 0xc,
    0x1900, 0x1c0,
    0x1af9, 0x107,
    0x1c46, 0xa,
    0x1c6d, 0x3,
    0x1cb7, 0x349,
    0x239a, 0x66,
    0x2475, 0xb,
    0x2544, 0xabc,
    0x342f, 0xfd1,
    0x4647, 0x21b9,
    0x6a39, 0x7,
    0x6a6a, 0x4,
    0x6a70, 0x60,
    0x6af6, 0xa,
    0x6b46, 0xa,
    0x6b78, 0x5,
    0x6b90, 0x370,
    0x6f45, 0xb,
    0x6f7f, 0x10,
    0x6fa0, 0x40,
    0x6fe1, 0x1f,
    0x87ed, 0x13,
    0x8af3, 0x250d,
    0xb002, 0xbfe,
    0xbc6b, 0x5,
    0xbc7d, 0x3,
    0xbc89, 0x7,
    0xbca0, 0x1360,
    0xd0f6, 0xa,
    0xd173, 0x8,
    0xd1e9, 0x17,
    0xd246, 0xba,
    0xd357, 0x9,
    0xd372, 0x8e,
    0xd547, 0x3,
    0xda8c, 0xf,
    0xdab0, 0x550,
    0xe02b, 0x7d5,
    0xe8d7, 0x29,
    0xe94b, 0x5,
    0xe95a, 0x4,
    0xe960, 0x4a0,
    0xee3c, 0x6,
    0xee43, 0x4,
    0xee9c, 0x5,
    0xeebc, 0x34,
    0xeef2, 0x10e,
    0xf02c, 0x4,
    0xf094, 0xc,
    0xf0f6, 0xa,
    0xf10d, 0x3,
    0xf16c, 0x4,
    0xf1ad, 0x39,
    0xf203, 0xd,
    0xf23c, 0x4,
    0xf249, 0x7,
    0xf252, 0xae,
    0xf6d3, 0xd,
    0xf6ed, 0x3,
    0xf6f7, 0x9,
    0xf774, 0xc,
    0xf7d5, 0x2b,
    0xf80c, 0x4,
    0xf848, 0x8,
    0xf85a, 0x6,
    0xf888, 0x8,
    0xf8ae, 0x62,
    0xf928, 0x8,
    0xf94c, 0x4,
    0xf95f, 0x21,
    0xf992, 0x2e,
    0xf9c1, 0x63f,
];
