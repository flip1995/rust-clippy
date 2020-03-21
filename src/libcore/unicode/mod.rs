#![unstable(feature = "unicode_internals", issue = "none")]
#![allow(missing_docs)]

pub(crate) mod printable;
mod unicode_data;
pub(crate) mod version;

use version::UnicodeVersion;

/// The version of [Unicode](http://www.unicode.org/) that the Unicode parts of
/// `char` and `str` methods are based on.
#[unstable(feature = "unicode_version", issue = "49726")]
pub const UNICODE_VERSION: UnicodeVersion = UnicodeVersion {
    major: unicode_data::UNICODE_VERSION.0,
    minor: unicode_data::UNICODE_VERSION.1,
    micro: unicode_data::UNICODE_VERSION.2,
    _priv: (),
};

// For use in liballoc, not re-exported in libstd.
pub mod derived_property {
    pub use super::{Case_Ignorable, Cased};
}

pub use unicode_data::alphabetic::lookup as Alphabetic;
pub use unicode_data::case_ignorable::lookup as Case_Ignorable;
pub use unicode_data::cased::lookup as Cased;
pub use unicode_data::cc::lookup as Cc;
pub use unicode_data::conversions;
pub use unicode_data::grapheme_extend::lookup as Grapheme_Extend;
pub use unicode_data::lowercase::lookup as Lowercase;
pub use unicode_data::n::lookup as N;
pub use unicode_data::uppercase::lookup as Uppercase;
pub use unicode_data::white_space::lookup as White_Space;

#[inline(always)]
fn range_search<
    const N: usize,
    const CHUNK_SIZE: usize,
    const N1: usize,
    const CANONICAL: usize,
    const CANONICALIZED: usize,
>(
    needle: u32,
    chunk_idx_map: &[u8; N],
    (last_chunk_idx, last_chunk_mapping): (u16, u8),
    bitset_chunk_idx: &[[u8; CHUNK_SIZE]; N1],
    bitset_canonical: &[u64; CANONICAL],
    bitset_canonicalized: &[(u8, u8); CANONICALIZED],
) -> bool {
    let bucket_idx = (needle / 64) as usize;
    let chunk_map_idx = bucket_idx / CHUNK_SIZE;
    let chunk_piece = bucket_idx % CHUNK_SIZE;
    let chunk_idx = if chunk_map_idx >= N {
        if chunk_map_idx == last_chunk_idx as usize {
            last_chunk_mapping
        } else {
            return false;
        }
    } else {
        chunk_idx_map[chunk_map_idx]
    };
    let idx = bitset_chunk_idx[(chunk_idx as usize)][chunk_piece] as usize;
    let word = if idx < CANONICAL {
        bitset_canonical[idx]
    } else {
        let (real_idx, mapping) = bitset_canonicalized[idx - CANONICAL];
        let mut word = bitset_canonical[real_idx as usize];
        let should_invert = mapping & (1 << 7) != 0;
        if should_invert {
            word = !word;
        }
        // Unset the inversion bit
        let rotate_by = mapping & !(1 << 7);
        word = word.rotate_left(rotate_by as u32);
        word
    };
    (word & (1 << (needle % 64) as u64)) != 0
}
