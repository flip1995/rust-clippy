// Copyright 2012-2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Unicode string slices.
//!
//! *[See also the `str` primitive type](../../std/primitive.str.html).*
//!
//! The `&str` type is one of the two main string types, the other being `String`.
//! Unlike its `String` counterpart, its contents are borrowed.
//!
//! # Basic Usage
//!
//! A basic string declaration of `&str` type:
//!
//! ```
//! let hello_world = "Hello, World!";
//! ```
//!
//! Here we have declared a string literal, also known as a string slice.
//! String literals have a static lifetime, which means the string `hello_world`
//! is guaranteed to be valid for the duration of the entire program.
//! We can explicitly specify `hello_world`'s lifetime as well:
//!
//! ```
//! let hello_world: &'static str = "Hello, world!";
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

// Many of the usings in this module are only used in the test configuration.
// It's cleaner to just turn off the unused_imports warning than to fix them.
#![allow(unused_imports)]

use core::fmt;
use core::str as core_str;
use core::str::pattern::Pattern;
use core::str::pattern::{Searcher, ReverseSearcher, DoubleEndedSearcher};
use core::mem;
use core::ptr;
use core::iter::FusedIterator;
use core::unicode::conversions;

use borrow::{Borrow, ToOwned};
use boxed::Box;
use slice::{SliceConcatExt, SliceIndex};
use string::String;
use vec::Vec;

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{FromStr, Utf8Error};
#[allow(deprecated)]
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{Lines, LinesAny};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{Split, RSplit};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{SplitN, RSplitN};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{SplitTerminator, RSplitTerminator};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{Matches, RMatches};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{MatchIndices, RMatchIndices};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{from_utf8, from_utf8_mut, Chars, CharIndices, Bytes};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::{from_utf8_unchecked, from_utf8_unchecked_mut, ParseBoolError};
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::SplitWhitespace;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::str::pattern;
#[stable(feature = "encode_utf16", since = "1.8.0")]
pub use core::str::EncodeUtf16;
#[unstable(feature = "split_ascii_whitespace", issue = "48656")]
pub use core::str::SplitAsciiWhitespace;

#[unstable(feature = "slice_concat_ext",
           reason = "trait should not have to exist",
           issue = "27747")]
impl<S: Borrow<str>> SliceConcatExt<str> for [S] {
    type Output = String;

    fn concat(&self) -> String {
        self.join("")
    }

    fn join(&self, sep: &str) -> String {
        unsafe {
            String::from_utf8_unchecked( join_generic_copy(self, sep.as_bytes()) )
        }
    }

    fn connect(&self, sep: &str) -> String {
        self.join(sep)
    }
}

macro_rules! spezialize_for_lengths {
    ($separator:expr, $target:expr, $iter:expr; $($num:expr),*) => {
        let mut target = $target;
        let iter = $iter;
        let sep_bytes = $separator;
        match $separator.len() {
            $(
                // loops with hardcoded sizes run much faster
                // specialize the cases with small separator lengths
                $num => {
                    for s in iter {
                        copy_slice_and_advance!(target, sep_bytes);
                        copy_slice_and_advance!(target, s.borrow().as_ref());
                    }
                },
            )*
            _ => {
                // arbitrary non-zero size fallback
                for s in iter {
                    copy_slice_and_advance!(target, sep_bytes);
                    copy_slice_and_advance!(target, s.borrow().as_ref());
                }
            }
        }
    };
}

macro_rules! copy_slice_and_advance {
    ($target:expr, $bytes:expr) => {
        let len = $bytes.len();
        let (head, tail) = {$target}.split_at_mut(len);
        head.copy_from_slice($bytes);
        $target = tail;
    }
}

// Optimized join implementation that works for both Vec<T> (T: Copy) and String's inner vec
// Currently (2018-05-13) there is a bug with type inference and specialization (see issue #36262)
// For this reason SliceConcatExt<T> is not specialized for T: Copy and SliceConcatExt<str> is the
// only user of this function. It is left in place for the time when that is fixed.
//
// the bounds for String-join are S: Borrow<str> and for Vec-join Borrow<[T]>
// [T] and str both impl AsRef<[T]> for some T
// => s.borrow().as_ref() and we always have slices
fn join_generic_copy<B, T, S>(slice: &[S], sep: &[T]) -> Vec<T>
where
    T: Copy,
    B: AsRef<[T]> + ?Sized,
    S: Borrow<B>,
{
    let sep_len = sep.len();
    let mut iter = slice.iter();

    // the first slice is the only one without a separator preceding it
    let first = match iter.next() {
        Some(first) => first,
        None => return vec![],
    };

    // compute the exact total length of the joined Vec
    // if the `len` calculation overflows, we'll panic
    // we would have run out of memory anyway and the rest of the function requires
    // the entire Vec pre-allocated for safety
    let len =  sep_len.checked_mul(iter.len()).and_then(|n| {
            slice.iter()
                .map(|s| s.borrow().as_ref().len())
                .try_fold(n, usize::checked_add)
        }).expect("attempt to join into collection with len > usize::MAX");

    // crucial for safety
    let mut result = Vec::with_capacity(len);
    assert!(result.capacity() >= len);

    result.extend_from_slice(first.borrow().as_ref());

    unsafe {
        {
            let pos = result.len();
            let target = result.get_unchecked_mut(pos..len);

            // copy separator and slices over without bounds checks
            // generate loops with hardcoded offsets for small separators
            // massive improvements possible (~ x2)
            spezialize_for_lengths!(sep, target, iter; 0, 1, 2, 3, 4);
        }
        result.set_len(len);
    }
    result
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Borrow<str> for String {
    #[inline]
    fn borrow(&self) -> &str {
        &self[..]
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToOwned for str {
    type Owned = String;
    fn to_owned(&self) -> String {
        unsafe { String::from_utf8_unchecked(self.as_bytes().to_owned()) }
    }

    fn clone_into(&self, target: &mut String) {
        let mut b = mem::replace(target, String::new()).into_bytes();
        self.as_bytes().clone_into(&mut b);
        *target = unsafe { String::from_utf8_unchecked(b) }
    }
}

/// Methods for string slices.
#[lang = "str_alloc"]
#[cfg(not(test))]
impl str {
    /// Converts a `Box<str>` into a `Box<[u8]>` without copying or allocating.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let s = "this is a string";
    /// let boxed_str = s.to_owned().into_boxed_str();
    /// let boxed_bytes = boxed_str.into_boxed_bytes();
    /// assert_eq!(*boxed_bytes, *s.as_bytes());
    /// ```
    #[stable(feature = "str_box_extras", since = "1.20.0")]
    #[inline]
    pub fn into_boxed_bytes(self: Box<str>) -> Box<[u8]> {
        self.into()
    }

    /// Replaces all matches of a pattern with another string.
    ///
    /// `replace` creates a new [`String`], and copies the data from this string slice into it.
    /// While doing so, it attempts to find matches of a pattern. If it finds any, it
    /// replaces them with the replacement string slice.
    ///
    /// [`String`]: string/struct.String.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let s = "this is old";
    ///
    /// assert_eq!("this is new", s.replace("old", "new"));
    /// ```
    ///
    /// When the pattern doesn't match:
    ///
    /// ```
    /// let s = "this is old";
    /// assert_eq!(s, s.replace("cookie monster", "little lamb"));
    /// ```
    #[must_use = "this returns the replaced string as a new allocation, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn replace<'a, P: Pattern<'a>>(&'a self, from: P, to: &str) -> String {
        let mut result = String::new();
        let mut last_end = 0;
        for (start, part) in self.match_indices(from) {
            result.push_str(unsafe { self.get_unchecked(last_end..start) });
            result.push_str(to);
            last_end = start + part.len();
        }
        result.push_str(unsafe { self.get_unchecked(last_end..self.len()) });
        result
    }

    /// Replaces first N matches of a pattern with another string.
    ///
    /// `replacen` creates a new [`String`], and copies the data from this string slice into it.
    /// While doing so, it attempts to find matches of a pattern. If it finds any, it
    /// replaces them with the replacement string slice at most `count` times.
    ///
    /// [`String`]: string/struct.String.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let s = "foo foo 123 foo";
    /// assert_eq!("new new 123 foo", s.replacen("foo", "new", 2));
    /// assert_eq!("faa fao 123 foo", s.replacen('o', "a", 3));
    /// assert_eq!("foo foo new23 foo", s.replacen(char::is_numeric, "new", 1));
    /// ```
    ///
    /// When the pattern doesn't match:
    ///
    /// ```
    /// let s = "this is old";
    /// assert_eq!(s, s.replacen("cookie monster", "little lamb", 10));
    /// ```
    #[must_use = "this returns the replaced string as a new allocation, \
                  without modifying the original"]
    #[stable(feature = "str_replacen", since = "1.16.0")]
    pub fn replacen<'a, P: Pattern<'a>>(&'a self, pat: P, to: &str, count: usize) -> String {
        // Hope to reduce the times of re-allocation
        let mut result = String::with_capacity(32);
        let mut last_end = 0;
        for (start, part) in self.match_indices(pat).take(count) {
            result.push_str(unsafe { self.get_unchecked(last_end..start) });
            result.push_str(to);
            last_end = start + part.len();
        }
        result.push_str(unsafe { self.get_unchecked(last_end..self.len()) });
        result
    }

    /// Returns the lowercase equivalent of this string slice, as a new [`String`].
    ///
    /// 'Lowercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Lowercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`String`] instead of modifying the
    /// parameter in-place.
    ///
    /// [`String`]: string/struct.String.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let s = "HELLO";
    ///
    /// assert_eq!("hello", s.to_lowercase());
    /// ```
    ///
    /// A tricky example, with sigma:
    ///
    /// ```
    /// let sigma = "Σ";
    ///
    /// assert_eq!("σ", sigma.to_lowercase());
    ///
    /// // but at the end of a word, it's ς, not σ:
    /// let odysseus = "ὈΔΥΣΣΕΎΣ";
    ///
    /// assert_eq!("ὀδυσσεύς", odysseus.to_lowercase());
    /// ```
    ///
    /// Languages without case are not changed:
    ///
    /// ```
    /// let new_year = "农历新年";
    ///
    /// assert_eq!(new_year, new_year.to_lowercase());
    /// ```
    #[stable(feature = "unicode_case_mapping", since = "1.2.0")]
    pub fn to_lowercase(&self) -> String {
        let mut s = String::with_capacity(self.len());
        for (i, c) in self[..].char_indices() {
            if c == 'Σ' {
                // Σ maps to σ, except at the end of a word where it maps to ς.
                // This is the only conditional (contextual) but language-independent mapping
                // in `SpecialCasing.txt`,
                // so hard-code it rather than have a generic "condition" mechanism.
                // See https://github.com/rust-lang/rust/issues/26035
                map_uppercase_sigma(self, i, &mut s)
            } else {
                match conversions::to_lower(c) {
                    [a, '\0', _] => s.push(a),
                    [a, b, '\0'] => {
                        s.push(a);
                        s.push(b);
                    }
                    [a, b, c] => {
                        s.push(a);
                        s.push(b);
                        s.push(c);
                    }
                }
            }
        }
        return s;

        fn map_uppercase_sigma(from: &str, i: usize, to: &mut String) {
            // See http://www.unicode.org/versions/Unicode7.0.0/ch03.pdf#G33992
            // for the definition of `Final_Sigma`.
            debug_assert!('Σ'.len_utf8() == 2);
            let is_word_final = case_ignoreable_then_cased(from[..i].chars().rev()) &&
                                !case_ignoreable_then_cased(from[i + 2..].chars());
            to.push_str(if is_word_final { "ς" } else { "σ" });
        }

        fn case_ignoreable_then_cased<I: Iterator<Item = char>>(iter: I) -> bool {
            use core::unicode::derived_property::{Cased, Case_Ignorable};
            match iter.skip_while(|&c| Case_Ignorable(c)).next() {
                Some(c) => Cased(c),
                None => false,
            }
        }
    }

    /// Returns the uppercase equivalent of this string slice, as a new [`String`].
    ///
    /// 'Uppercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Uppercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`String`] instead of modifying the
    /// parameter in-place.
    ///
    /// [`String`]: string/struct.String.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let s = "hello";
    ///
    /// assert_eq!("HELLO", s.to_uppercase());
    /// ```
    ///
    /// Scripts without case are not changed:
    ///
    /// ```
    /// let new_year = "农历新年";
    ///
    /// assert_eq!(new_year, new_year.to_uppercase());
    /// ```
    #[stable(feature = "unicode_case_mapping", since = "1.2.0")]
    pub fn to_uppercase(&self) -> String {
        let mut s = String::with_capacity(self.len());
        for c in self[..].chars() {
            match conversions::to_upper(c) {
                [a, '\0', _] => s.push(a),
                [a, b, '\0'] => {
                    s.push(a);
                    s.push(b);
                }
                [a, b, c] => {
                    s.push(a);
                    s.push(b);
                    s.push(c);
                }
            }
        }
        return s;
    }

    /// Escapes each char in `s` with [`char::escape_debug`].
    ///
    /// Note: only extended grapheme codepoints that begin the string will be
    /// escaped.
    ///
    /// [`char::escape_debug`]: primitive.char.html#method.escape_debug
    #[unstable(feature = "str_escape",
               reason = "return type may change to be an iterator",
               issue = "27791")]
    pub fn escape_debug(&self) -> String {
        let mut string = String::with_capacity(self.len());
        let mut chars = self.chars();
        if let Some(first) = chars.next() {
            string.extend(first.escape_debug_ext(true))
        }
        string.extend(chars.flat_map(|c| c.escape_debug_ext(false)));
        string
    }

    /// Escapes each char in `s` with [`char::escape_default`].
    ///
    /// [`char::escape_default`]: primitive.char.html#method.escape_default
    #[unstable(feature = "str_escape",
               reason = "return type may change to be an iterator",
               issue = "27791")]
    pub fn escape_default(&self) -> String {
        self.chars().flat_map(|c| c.escape_default()).collect()
    }

    /// Escapes each char in `s` with [`char::escape_unicode`].
    ///
    /// [`char::escape_unicode`]: primitive.char.html#method.escape_unicode
    #[unstable(feature = "str_escape",
               reason = "return type may change to be an iterator",
               issue = "27791")]
    pub fn escape_unicode(&self) -> String {
        self.chars().flat_map(|c| c.escape_unicode()).collect()
    }

    /// Converts a [`Box<str>`] into a [`String`] without copying or allocating.
    ///
    /// [`String`]: string/struct.String.html
    /// [`Box<str>`]: boxed/struct.Box.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// let string = String::from("birthday gift");
    /// let boxed_str = string.clone().into_boxed_str();
    ///
    /// assert_eq!(boxed_str.into_string(), string);
    /// ```
    #[stable(feature = "box_str", since = "1.4.0")]
    #[inline]
    pub fn into_string(self: Box<str>) -> String {
        let slice = Box::<[u8]>::from(self);
        unsafe { String::from_utf8_unchecked(slice.into_vec()) }
    }

    /// Creates a new [`String`] by repeating a string `n` times.
    ///
    /// [`String`]: string/struct.String.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// assert_eq!("abc".repeat(4), String::from("abcabcabcabc"));
    /// ```
    #[stable(feature = "repeat_str", since = "1.16.0")]
    pub fn repeat(&self, n: usize) -> String {
        unsafe { String::from_utf8_unchecked(self.as_bytes().repeat(n)) }
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII upper case equivalent.
    ///
    /// ASCII letters 'a' to 'z' are mapped to 'A' to 'Z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To uppercase the value in-place, use [`make_ascii_uppercase`].
    ///
    /// To uppercase ASCII characters in addition to non-ASCII characters, use
    /// [`to_uppercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// let s = "Grüße, Jürgen ❤";
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s.to_ascii_uppercase());
    /// ```
    ///
    /// [`make_ascii_uppercase`]: #method.make_ascii_uppercase
    /// [`to_uppercase`]: #method.to_uppercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[inline]
    pub fn to_ascii_uppercase(&self) -> String {
        let mut bytes = self.as_bytes().to_vec();
        bytes.make_ascii_uppercase();
        // make_ascii_uppercase() preserves the UTF-8 invariant.
        unsafe { String::from_utf8_unchecked(bytes) }
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII lower case equivalent.
    ///
    /// ASCII letters 'A' to 'Z' are mapped to 'a' to 'z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To lowercase the value in-place, use [`make_ascii_lowercase`].
    ///
    /// To lowercase ASCII characters in addition to non-ASCII characters, use
    /// [`to_lowercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// let s = "Grüße, Jürgen ❤";
    ///
    /// assert_eq!("grüße, jürgen ❤", s.to_ascii_lowercase());
    /// ```
    ///
    /// [`make_ascii_lowercase`]: #method.make_ascii_lowercase
    /// [`to_lowercase`]: #method.to_lowercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[inline]
    pub fn to_ascii_lowercase(&self) -> String {
        let mut bytes = self.as_bytes().to_vec();
        bytes.make_ascii_lowercase();
        // make_ascii_lowercase() preserves the UTF-8 invariant.
        unsafe { String::from_utf8_unchecked(bytes) }
    }
}

/// Converts a boxed slice of bytes to a boxed string slice without checking
/// that the string contains valid UTF-8.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let smile_utf8 = Box::new([226, 152, 186]);
/// let smile = unsafe { std::str::from_boxed_utf8_unchecked(smile_utf8) };
///
/// assert_eq!("☺", &*smile);
/// ```
#[stable(feature = "str_box_extras", since = "1.20.0")]
#[inline]
pub unsafe fn from_boxed_utf8_unchecked(v: Box<[u8]>) -> Box<str> {
    Box::from_raw(Box::into_raw(v) as *mut str)
}
