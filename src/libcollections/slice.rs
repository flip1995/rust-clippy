// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for slice manipulation
//!
//! The `slice` module contains useful code to help work with slice values.
//! Slices are a view into a block of memory represented as a pointer and a length.
//!
//! ```rust
//! // slicing a Vec
//! let vec = vec!(1, 2, 3);
//! let int_slice = vec.as_slice();
//! // coercing an array to a slice
//! let str_slice: &[&str] = &["one", "two", "three"];
//! ```
//!
//! Slices are either mutable or shared. The shared slice type is `&[T]`,
//! while the mutable slice type is `&mut[T]`. For example, you can mutate the
//! block of memory that a mutable slice points to:
//!
//! ```rust
//! let x: &mut[i32] = &mut [1, 2, 3];
//! x[1] = 7;
//! assert_eq!(x[0], 1);
//! assert_eq!(x[1], 7);
//! assert_eq!(x[2], 3);
//! ```
//!
//! Here are some of the things this module contains:
//!
//! ## Structs
//!
//! There are several structs that are useful for slices, such as `Iter`, which
//! represents iteration over a slice.
//!
//! ## Traits
//!
//! A number of traits add methods that allow you to accomplish tasks
//! with slices, the most important being `SliceExt`. Other traits
//! apply only to slices of elements satisfying certain bounds (like
//! `Ord`).
//!
//! An example is the `slice` method which enables slicing syntax `[a..b]` that
//! returns an immutable "view" into a `Vec` or another slice from the index
//! interval `[a, b)`:
//!
//! ```rust
//! fn main() {
//!     let numbers = [0, 1, 2];
//!     let last_numbers = &numbers[1..3];
//!     // last_numbers is now &[1, 2]
//! }
//! ```
//!
//! ## Implementations of other traits
//!
//! There are several implementations of common traits for slices. Some examples
//! include:
//!
//! * `Clone`
//! * `Eq`, `Ord` - for immutable slices whose element type are `Eq` or `Ord`.
//! * `Hash` - for slices whose element type is `Hash`
//!
//! ## Iteration
//!
//! The method `iter()` returns an iteration value for a slice. The iterator
//! yields references to the slice's elements, so if the element
//! type of the slice is `isize`, the element type of the iterator is `&isize`.
//!
//! ```rust
//! let numbers = [0, 1, 2];
//! for &x in numbers.iter() {
//!     println!("{} is a number!", x);
//! }
//! ```
//!
//! * `.iter_mut()` returns an iterator that allows modifying each value.
//! * Further iterators exist that split, chunk or permute the slice.

#![doc(primitive = "slice")]
#![stable(feature = "rust1", since = "1.0.0")]

use alloc::boxed::Box;
use core::clone::Clone;
use core::cmp::Ordering::{self, Greater, Less};
use core::cmp::{self, Ord, PartialEq};
use core::iter::{Iterator, IteratorExt};
use core::iter::MultiplicativeIterator;
use core::marker::Sized;
use core::mem::size_of;
use core::mem;
use core::num::wrapping::WrappingOps;
use core::ops::FnMut;
use core::option::Option::{self, Some, None};
#[cfg(stage0)]
use core::ptr::PtrExt;
use core::ptr;
use core::result::Result;
use core::slice as core_slice;
use self::Direction::*;

use borrow::{Borrow, BorrowMut, ToOwned};
use vec::Vec;

pub use core::slice::{Chunks, AsSlice, Windows};
pub use core::slice::{Iter, IterMut};
pub use core::slice::{IntSliceExt, SplitMut, ChunksMut, Split};
pub use core::slice::{SplitN, RSplitN, SplitNMut, RSplitNMut};
pub use core::slice::{bytes, mut_ref_slice, ref_slice};
pub use core::slice::{from_raw_parts, from_raw_parts_mut};
pub use core::slice::{from_raw_buf, from_raw_mut_buf};

////////////////////////////////////////////////////////////////////////////////
// Basic slice extension methods
////////////////////////////////////////////////////////////////////////////////

#[cfg(stage0)]
/// Allocating extension methods for slices.
#[stable(feature = "rust1", since = "1.0.0")]
pub trait SliceExt {
    #[stable(feature = "rust1", since = "1.0.0")]
    type Item;

    /// Sorts the slice, in place, using `compare` to compare
    /// elements.
    ///
    /// This sort is `O(n log n)` worst-case and stable, but allocates
    /// approximately `2 * n`, where `n` is the length of `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [5, 4, 1, 3, 2];
    /// v.sort_by(|a, b| a.cmp(b));
    /// assert!(v == [1, 2, 3, 4, 5]);
    ///
    /// // reverse sorting
    /// v.sort_by(|a, b| b.cmp(a));
    /// assert!(v == [5, 4, 3, 2, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn sort_by<F>(&mut self, compare: F) where F: FnMut(&Self::Item, &Self::Item) -> Ordering;

    /// Consumes `src` and moves as many elements as it can into `self`
    /// from the range [start,end).
    ///
    /// Returns the number of elements copied (the shorter of `self.len()`
    /// and `end - start`).
    ///
    /// # Arguments
    ///
    /// * src - A mutable vector of `T`
    /// * start - The index into `src` to start copying from
    /// * end - The index into `src` to stop copying from
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut a = [1, 2, 3, 4, 5];
    /// let b = vec![6, 7, 8];
    /// let num_moved = a.move_from(b, 0, 3);
    /// assert_eq!(num_moved, 3);
    /// assert!(a == [6, 7, 8, 4, 5]);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain about this API approach")]
    fn move_from(&mut self, src: Vec<Self::Item>, start: usize, end: usize) -> usize;

    /// Deprecated: use `&s[start .. end]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[start .. end] instead")]
    fn slice(&self, start: usize, end: usize) -> &[Self::Item];

    /// Deprecated: use `&s[start..]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[start..] instead")]
    fn slice_from(&self, start: usize) -> &[Self::Item];

    /// Deprecated: use `&s[..end]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[..end] instead")]
    fn slice_to(&self, end: usize) -> &[Self::Item];

    /// Divides one slice into two at an index.
    ///
    /// The first will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself) and the second will contain all
    /// indices from `[mid, len)` (excluding the index `len` itself).
    ///
    /// Panics if `mid > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 50];
    /// let (v1, v2) = v.split_at(2);
    /// assert_eq!([10, 40], v1);
    /// assert_eq!([30, 20, 50], v2);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split_at(&self, mid: usize) -> (&[Self::Item], &[Self::Item]);

    /// Returns an iterator over the slice.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn iter(&self) -> Iter<Self::Item>;

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`.  The matched element is not contained in the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split by numbers divisible by 3 (i.e. `[10, 40]`,
    /// `[20]`, `[50]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.split(|num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split<F>(&self, pred: F) -> Split<Self::Item, F>
                where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, limited to splitting at most `n` times.  The matched element is
    /// not contained in the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split once by numbers divisible by 3 (i.e. `[10, 40]`,
    /// `[20, 60, 50]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.splitn(1, |num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn splitn<F>(&self, n: usize, pred: F) -> SplitN<Self::Item, F>
                 where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to splitting at most `n` times. This starts at the end of
    /// the slice and works backwards.  The matched element is not contained in
    /// the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split once, starting from the end, by numbers divisible
    /// by 3 (i.e. `[50]`, `[10, 40, 30, 20]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.rsplitn(1, |num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<Self::Item, F>
                  where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over all contiguous windows of length
    /// `size`. The windows overlap. If the slice is shorter than
    /// `size`, the iterator returns no values.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    ///
    /// # Examples
    ///
    /// Print the adjacent pairs of a slice (i.e. `[1,2]`, `[2,3]`,
    /// `[3,4]`):
    ///
    /// ```rust
    /// let v = &[1, 2, 3, 4];
    /// for win in v.windows(2) {
    ///     println!("{:?}", win);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn windows(&self, size: usize) -> Windows<Self::Item>;

    /// Returns an iterator over `size` elements of the slice at a
    /// time. The chunks do not overlap. If `size` does not divide the
    /// length of the slice, then the last chunk will not have length
    /// `size`.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    ///
    /// # Examples
    ///
    /// Print the slice two elements at a time (i.e. `[1,2]`,
    /// `[3,4]`, `[5]`):
    ///
    /// ```rust
    /// let v = &[1, 2, 3, 4, 5];
    /// for win in v.chunks(2) {
    ///     println!("{:?}", win);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn chunks(&self, size: usize) -> Chunks<Self::Item>;

    /// Returns the element of a slice at the given index, or `None` if the
    /// index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&40), v.get(1));
    /// assert_eq!(None, v.get(3));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn get(&self, index: usize) -> Option<&Self::Item>;

    /// Returns the first element of a slice, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&10), v.first());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.first());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn first(&self) -> Option<&Self::Item>;

    /// Returns all but the first element of a slice.
    #[unstable(feature = "collections", reason = "likely to be renamed")]
    fn tail(&self) -> &[Self::Item];

    /// Returns all but the last element of a slice.
    #[unstable(feature = "collections", reason = "likely to be renamed")]
    fn init(&self) -> &[Self::Item];

    /// Returns the last element of a slice, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&30), v.last());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.last());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn last(&self) -> Option<&Self::Item>;

    /// Returns a pointer to the element at the given index, without doing
    /// bounds checking.
    #[stable(feature = "rust1", since = "1.0.0")]
    unsafe fn get_unchecked(&self, index: usize) -> &Self::Item;

    /// Returns an unsafe pointer to the slice's buffer
    ///
    /// The caller must ensure that the slice outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the slice may cause its buffer to be reallocated, which
    /// would also make any pointers to it invalid.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_ptr(&self) -> *const Self::Item;

    /// Binary search a sorted slice with a comparator function.
    ///
    /// The comparator function should implement an order consistent
    /// with the sort order of the underlying slice, returning an
    /// order code that indicates whether its argument is `Less`,
    /// `Equal` or `Greater` the desired target.
    ///
    /// If a matching value is found then returns `Ok`, containing
    /// the index for the matched element; if no match is found then
    /// `Err` is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// # Examples
    ///
    /// Looks up a series of four elements. The first is found, with a
    /// uniquely determined position; the second and third are not
    /// found; the fourth could match any position in `[1,4]`.
    ///
    /// ```rust
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let s = s.as_slice();
    ///
    /// let seek = 13;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Ok(9));
    /// let seek = 4;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(7));
    /// let seek = 100;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(13));
    /// let seek = 1;
    /// let r = s.binary_search_by(|probe| probe.cmp(&seek));
    /// assert!(match r { Ok(1...4) => true, _ => false, });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn binary_search_by<F>(&self, f: F) -> Result<usize, usize> where
        F: FnMut(&Self::Item) -> Ordering;

    /// Return the number of elements in the slice
    ///
    /// # Examples
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.len(), 3);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn len(&self) -> usize;

    /// Returns true if the slice has a length of 0
    ///
    /// # Examples
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert!(!a.is_empty());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn is_empty(&self) -> bool { self.len() == 0 }
    /// Returns a mutable reference to the element at the given index,
    /// or `None` if the index is out of bounds
    #[stable(feature = "rust1", since = "1.0.0")]
    fn get_mut(&mut self, index: usize) -> Option<&mut Self::Item>;

    /// Work with `self` as a mut slice.
    /// Primarily intended for getting a &mut [T] from a [T; N].
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_mut_slice(&mut self) -> &mut [Self::Item];

    /// Deprecated: use `&mut s[start .. end]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[start .. end] instead")]
    fn slice_mut(&mut self, start: usize, end: usize) -> &mut [Self::Item];

    /// Deprecated: use `&mut s[start ..]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[start ..] instead")]
    fn slice_from_mut(&mut self, start: usize) -> &mut [Self::Item];

    /// Deprecated: use `&mut s[.. end]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[.. end] instead")]
    fn slice_to_mut(&mut self, end: usize) -> &mut [Self::Item];

    /// Returns an iterator that allows modifying each value
    #[stable(feature = "rust1", since = "1.0.0")]
    fn iter_mut(&mut self) -> IterMut<Self::Item>;

    /// Returns a mutable pointer to the first element of a slice, or `None` if it is empty
    #[stable(feature = "rust1", since = "1.0.0")]
    fn first_mut(&mut self) -> Option<&mut Self::Item>;

    /// Returns all but the first element of a mutable slice
    #[unstable(feature = "collections",
               reason = "likely to be renamed or removed")]
    fn tail_mut(&mut self) -> &mut [Self::Item];

    /// Returns all but the last element of a mutable slice
    #[unstable(feature = "collections",
               reason = "likely to be renamed or removed")]
    fn init_mut(&mut self) -> &mut [Self::Item];

    /// Returns a mutable pointer to the last item in the slice.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn last_mut(&mut self) -> Option<&mut Self::Item>;

    /// Returns an iterator over mutable subslices separated by elements that
    /// match `pred`.  The matched element is not contained in the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split_mut<F>(&mut self, pred: F) -> SplitMut<Self::Item, F>
                    where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, limited to splitting at most `n` times.  The matched element is
    /// not contained in the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<Self::Item, F>
                     where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to splitting at most `n` times. This starts at the end of
    /// the slice and works backwards.  The matched element is not contained in
    /// the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn rsplitn_mut<F>(&mut self,  n: usize, pred: F) -> RSplitNMut<Self::Item, F>
                      where F: FnMut(&Self::Item) -> bool;

    /// Returns an iterator over `chunk_size` elements of the slice at a time.
    /// The chunks are mutable and do not overlap. If `chunk_size` does
    /// not divide the length of the slice, then the last chunk will not
    /// have length `chunk_size`.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_size` is 0.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<Self::Item>;

    /// Swaps two elements in a slice.
    ///
    /// # Arguments
    ///
    /// * a - The index of the first element
    /// * b - The index of the second element
    ///
    /// # Panics
    ///
    /// Panics if `a` or `b` are out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = ["a", "b", "c", "d"];
    /// v.swap(1, 3);
    /// assert!(v == ["a", "d", "c", "b"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn swap(&mut self, a: usize, b: usize);

    /// Divides one `&mut` into two at an index.
    ///
    /// The first will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself) and the second will contain all
    /// indices from `[mid, len)` (excluding the index `len` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > len`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [1, 2, 3, 4, 5, 6];
    ///
    /// // scoped to restrict the lifetime of the borrows
    /// {
    ///    let (left, right) = v.split_at_mut(0);
    ///    assert!(left == []);
    ///    assert!(right == [1, 2, 3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_mut(2);
    ///     assert!(left == [1, 2]);
    ///     assert!(right == [3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_mut(6);
    ///     assert!(left == [1, 2, 3, 4, 5, 6]);
    ///     assert!(right == []);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split_at_mut(&mut self, mid: usize) -> (&mut [Self::Item], &mut [Self::Item]);

    /// Reverse the order of elements in a slice, in place.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [1, 2, 3];
    /// v.reverse();
    /// assert!(v == [3, 2, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn reverse(&mut self);

    /// Returns an unsafe mutable pointer to the element in index
    #[stable(feature = "rust1", since = "1.0.0")]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut Self::Item;

    /// Return an unsafe mutable pointer to the slice's buffer.
    ///
    /// The caller must ensure that the slice outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the slice may cause its buffer to be reallocated, which
    /// would also make any pointers to it invalid.
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_mut_ptr(&mut self) -> *mut Self::Item;

    /// Copies `self` into a new `Vec`.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn to_vec(&self) -> Vec<Self::Item> where Self::Item: Clone;

    /// Creates an iterator that yields every possible permutation of the
    /// vector in succession.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let v = [1, 2, 3];
    /// let mut perms = v.permutations();
    ///
    /// for p in perms {
    ///   println!("{:?}", p);
    /// }
    /// ```
    ///
    /// Iterating through permutations one by one.
    ///
    /// ```rust
    /// let v = [1, 2, 3];
    /// let mut perms = v.permutations();
    ///
    /// assert_eq!(Some(vec![1, 2, 3]), perms.next());
    /// assert_eq!(Some(vec![1, 3, 2]), perms.next());
    /// assert_eq!(Some(vec![3, 1, 2]), perms.next());
    /// ```
    #[unstable(feature = "collections")]
    fn permutations(&self) -> Permutations<Self::Item> where Self::Item: Clone;

    /// Copies as many elements from `src` as it can into `self` (the
    /// shorter of `self.len()` and `src.len()`). Returns the number
    /// of elements copied.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut dst = [0, 0, 0];
    /// let src = [1, 2];
    ///
    /// assert!(dst.clone_from_slice(&src) == 2);
    /// assert!(dst == [1, 2, 0]);
    ///
    /// let src2 = [3, 4, 5, 6];
    /// assert!(dst.clone_from_slice(&src2) == 3);
    /// assert!(dst == [3, 4, 5]);
    /// ```
    #[unstable(feature = "collections")]
    fn clone_from_slice(&mut self, &[Self::Item]) -> usize where Self::Item: Clone;

    /// Sorts the slice, in place.
    ///
    /// This is equivalent to `self.sort_by(|a, b| a.cmp(b))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [-5, 4, 1, -3, 2];
    ///
    /// v.sort();
    /// assert!(v == [-5, -3, 1, 2, 4]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn sort(&mut self) where Self::Item: Ord;

    /// Binary search a sorted slice for a given element.
    ///
    /// If the value is found then `Ok` is returned, containing the
    /// index of the matching element; if the value is not found then
    /// `Err` is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// # Examples
    ///
    /// Looks up a series of four elements. The first is found, with a
    /// uniquely determined position; the second and third are not
    /// found; the fourth could match any position in `[1,4]`.
    ///
    /// ```rust
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let s = s.as_slice();
    ///
    /// assert_eq!(s.binary_search(&13),  Ok(9));
    /// assert_eq!(s.binary_search(&4),   Err(7));
    /// assert_eq!(s.binary_search(&100), Err(13));
    /// let r = s.binary_search(&1);
    /// assert!(match r { Ok(1...4) => true, _ => false, });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn binary_search(&self, x: &Self::Item) -> Result<usize, usize> where Self::Item: Ord;

    /// Deprecated: use `binary_search` instead.
    #[unstable(feature = "collections")]
    #[deprecated(since = "1.0.0", reason = "use binary_search instead")]
    fn binary_search_elem(&self, x: &Self::Item) -> Result<usize, usize> where Self::Item: Ord {
        self.binary_search(x)
    }

    /// Mutates the slice to the next lexicographic permutation.
    ///
    /// Returns `true` if successful and `false` if the slice is at the
    /// last-ordered permutation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let v: &mut [_] = &mut [0, 1, 2];
    /// v.next_permutation();
    /// let b: &mut [_] = &mut [0, 2, 1];
    /// assert!(v == b);
    /// v.next_permutation();
    /// let b: &mut [_] = &mut [1, 0, 2];
    /// assert!(v == b);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain if this merits inclusion in std")]
    fn next_permutation(&mut self) -> bool where Self::Item: Ord;

    /// Mutates the slice to the previous lexicographic permutation.
    ///
    /// Returns `true` if successful and `false` if the slice is at the
    /// first-ordered permutation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let v: &mut [_] = &mut [1, 0, 2];
    /// v.prev_permutation();
    /// let b: &mut [_] = &mut [0, 2, 1];
    /// assert!(v == b);
    /// v.prev_permutation();
    /// let b: &mut [_] = &mut [0, 1, 2];
    /// assert!(v == b);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain if this merits inclusion in std")]
    fn prev_permutation(&mut self) -> bool where Self::Item: Ord;

    /// Find the first index containing a matching value.
    #[unstable(feature = "collections")]
    fn position_elem(&self, t: &Self::Item) -> Option<usize> where Self::Item: PartialEq;

    /// Find the last index containing a matching value.
    #[unstable(feature = "collections")]
    fn rposition_elem(&self, t: &Self::Item) -> Option<usize> where Self::Item: PartialEq;

    /// Returns true if the slice contains an element with the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.contains(&30));
    /// assert!(!v.contains(&50));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn contains(&self, x: &Self::Item) -> bool where Self::Item: PartialEq;

    /// Returns true if `needle` is a prefix of the slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.starts_with(&[10]));
    /// assert!(v.starts_with(&[10, 40]));
    /// assert!(!v.starts_with(&[50]));
    /// assert!(!v.starts_with(&[10, 50]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn starts_with(&self, needle: &[Self::Item]) -> bool where Self::Item: PartialEq;

    /// Returns true if `needle` is a suffix of the slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.ends_with(&[30]));
    /// assert!(v.ends_with(&[40, 30]));
    /// assert!(!v.ends_with(&[50]));
    /// assert!(!v.ends_with(&[50, 30]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn ends_with(&self, needle: &[Self::Item]) -> bool where Self::Item: PartialEq;

    /// Convert `self` into a vector without clones or allocation.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn into_vec(self: Box<Self>) -> Vec<Self::Item>;
}

#[cfg(stage0)]
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> SliceExt for [T] {
    type Item = T;

    #[inline]
    fn sort_by<F>(&mut self, compare: F) where F: FnMut(&T, &T) -> Ordering {
        merge_sort(self, compare)
    }

    #[inline]
    fn move_from(&mut self, mut src: Vec<T>, start: usize, end: usize) -> usize {
        for (a, b) in self.iter_mut().zip(src[start .. end].iter_mut()) {
            mem::swap(a, b);
        }
        cmp::min(self.len(), end-start)
    }

    #[inline]
    fn slice(&self, start: usize, end: usize) -> &[T] {
        &self[start .. end]
    }

    #[inline]
    fn slice_from(&self, start: usize) -> &[T] {
        &self[start ..]
    }

    #[inline]
    fn slice_to(&self, end: usize) -> &[T] {
        &self[.. end]
    }

    #[inline]
    fn split_at(&self, mid: usize) -> (&[T], &[T]) {
        core_slice::SliceExt::split_at(self, mid)
    }

    #[inline]
    fn iter(&self) -> Iter<T> {
        core_slice::SliceExt::iter(self)
    }

    #[inline]
    fn split<F>(&self, pred: F) -> Split<T, F>
                where F: FnMut(&T) -> bool {
        core_slice::SliceExt::split(self, pred)
    }

    #[inline]
    fn splitn<F>(&self, n: usize, pred: F) -> SplitN<T, F>
                 where F: FnMut(&T) -> bool {
        core_slice::SliceExt::splitn(self, n, pred)
    }

    #[inline]
    fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<T, F>
                  where F: FnMut(&T) -> bool {
        core_slice::SliceExt::rsplitn(self, n, pred)
    }

    #[inline]
    fn windows(&self, size: usize) -> Windows<T> {
        core_slice::SliceExt::windows(self, size)
    }

    #[inline]
    fn chunks(&self, size: usize) -> Chunks<T> {
        core_slice::SliceExt::chunks(self, size)
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&T> {
        core_slice::SliceExt::get(self, index)
    }

    #[inline]
    fn first(&self) -> Option<&T> {
        core_slice::SliceExt::first(self)
    }

    #[inline]
    fn tail(&self) -> &[T] {
        core_slice::SliceExt::tail(self)
    }

    #[inline]
    fn init(&self) -> &[T] {
        core_slice::SliceExt::init(self)
    }

    #[inline]
    fn last(&self) -> Option<&T> {
        core_slice::SliceExt::last(self)
    }

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        core_slice::SliceExt::get_unchecked(self, index)
    }

    #[inline]
    fn as_ptr(&self) -> *const T {
        core_slice::SliceExt::as_ptr(self)
    }

    #[inline]
    fn binary_search_by<F>(&self, f: F) -> Result<usize, usize>
                        where F: FnMut(&T) -> Ordering {
        core_slice::SliceExt::binary_search_by(self, f)
    }

    #[inline]
    fn len(&self) -> usize {
        core_slice::SliceExt::len(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        core_slice::SliceExt::is_empty(self)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        core_slice::SliceExt::get_mut(self, index)
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [T] {
        core_slice::SliceExt::as_mut_slice(self)
    }

    #[inline]
    fn slice_mut(&mut self, start: usize, end: usize) -> &mut [T] {
        &mut self[start .. end]
    }

    #[inline]
    fn slice_from_mut(&mut self, start: usize) -> &mut [T] {
        &mut self[start ..]
    }

    #[inline]
    fn slice_to_mut(&mut self, end: usize) -> &mut [T] {
        &mut self[.. end]
    }

    #[inline]
    fn iter_mut(&mut self) -> IterMut<T> {
        core_slice::SliceExt::iter_mut(self)
    }

    #[inline]
    fn first_mut(&mut self) -> Option<&mut T> {
        core_slice::SliceExt::first_mut(self)
    }

    #[inline]
    fn tail_mut(&mut self) -> &mut [T] {
        core_slice::SliceExt::tail_mut(self)
    }

    #[inline]
    fn init_mut(&mut self) -> &mut [T] {
        core_slice::SliceExt::init_mut(self)
    }

    #[inline]
    fn last_mut(&mut self) -> Option<&mut T> {
        core_slice::SliceExt::last_mut(self)
    }

    #[inline]
    fn split_mut<F>(&mut self, pred: F) -> SplitMut<T, F>
                    where F: FnMut(&T) -> bool {
        core_slice::SliceExt::split_mut(self, pred)
    }

    #[inline]
    fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<T, F>
                     where F: FnMut(&T) -> bool {
        core_slice::SliceExt::splitn_mut(self, n, pred)
    }

    #[inline]
    fn rsplitn_mut<F>(&mut self,  n: usize, pred: F) -> RSplitNMut<T, F>
                      where F: FnMut(&T) -> bool {
        core_slice::SliceExt::rsplitn_mut(self, n, pred)
    }

    #[inline]
    fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<T> {
        core_slice::SliceExt::chunks_mut(self, chunk_size)
    }

    #[inline]
    fn swap(&mut self, a: usize, b: usize) {
        core_slice::SliceExt::swap(self, a, b)
    }

    #[inline]
    fn split_at_mut(&mut self, mid: usize) -> (&mut [T], &mut [T]) {
        core_slice::SliceExt::split_at_mut(self, mid)
    }

    #[inline]
    fn reverse(&mut self) {
        core_slice::SliceExt::reverse(self)
    }

    #[inline]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        core_slice::SliceExt::get_unchecked_mut(self, index)
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut T {
        core_slice::SliceExt::as_mut_ptr(self)
    }

    /// Returns a copy of `v`.
    #[inline]
    fn to_vec(&self) -> Vec<T> where T: Clone {
        let mut vector = Vec::with_capacity(self.len());
        vector.push_all(self);
        vector
    }

    /// Returns an iterator over all permutations of a vector.
    fn permutations(&self) -> Permutations<T> where T: Clone {
        Permutations{
            swaps: ElementSwaps::new(self.len()),
            v: self.to_vec(),
        }
    }

    fn clone_from_slice(&mut self, src: &[T]) -> usize where T: Clone {
        core_slice::SliceExt::clone_from_slice(self, src)
    }

    #[inline]
    fn sort(&mut self) where T: Ord {
        self.sort_by(|a, b| a.cmp(b))
    }

    fn binary_search(&self, x: &T) -> Result<usize, usize> where T: Ord {
        core_slice::SliceExt::binary_search(self, x)
    }

    fn next_permutation(&mut self) -> bool where T: Ord {
        core_slice::SliceExt::next_permutation(self)
    }

    fn prev_permutation(&mut self) -> bool where T: Ord {
        core_slice::SliceExt::prev_permutation(self)
    }

    fn position_elem(&self, t: &T) -> Option<usize> where T: PartialEq {
        core_slice::SliceExt::position_elem(self, t)
    }

    fn rposition_elem(&self, t: &T) -> Option<usize> where T: PartialEq {
        core_slice::SliceExt::rposition_elem(self, t)
    }

    fn contains(&self, x: &T) -> bool where T: PartialEq {
        core_slice::SliceExt::contains(self, x)
    }

    fn starts_with(&self, needle: &[T]) -> bool where T: PartialEq {
        core_slice::SliceExt::starts_with(self, needle)
    }

    fn ends_with(&self, needle: &[T]) -> bool where T: PartialEq {
        core_slice::SliceExt::ends_with(self, needle)
    }

    fn into_vec(mut self: Box<Self>) -> Vec<T> {
        unsafe {
            let xs = Vec::from_raw_parts(self.as_mut_ptr(), self.len(), self.len());
            mem::forget(self);
            xs
        }
    }
}

#[cfg(not(stage0))]
/// Allocating extension methods for slices.
#[lang = "slice"]
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> [T] {
    /// Sorts the slice, in place, using `compare` to compare
    /// elements.
    ///
    /// This sort is `O(n log n)` worst-case and stable, but allocates
    /// approximately `2 * n`, where `n` is the length of `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [5, 4, 1, 3, 2];
    /// v.sort_by(|a, b| a.cmp(b));
    /// assert!(v == [1, 2, 3, 4, 5]);
    ///
    /// // reverse sorting
    /// v.sort_by(|a, b| b.cmp(a));
    /// assert!(v == [5, 4, 3, 2, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sort_by<F>(&mut self, compare: F) where F: FnMut(&T, &T) -> Ordering {
        merge_sort(self, compare)
    }

    /// Consumes `src` and moves as many elements as it can into `self`
    /// from the range [start,end).
    ///
    /// Returns the number of elements copied (the shorter of `self.len()`
    /// and `end - start`).
    ///
    /// # Arguments
    ///
    /// * src - A mutable vector of `T`
    /// * start - The index into `src` to start copying from
    /// * end - The index into `src` to stop copying from
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut a = [1, 2, 3, 4, 5];
    /// let b = vec![6, 7, 8];
    /// let num_moved = a.move_from(b, 0, 3);
    /// assert_eq!(num_moved, 3);
    /// assert!(a == [6, 7, 8, 4, 5]);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain about this API approach")]
    #[inline]
    pub fn move_from(&mut self, mut src: Vec<T>, start: usize, end: usize) -> usize {
        for (a, b) in self.iter_mut().zip(src[start .. end].iter_mut()) {
            mem::swap(a, b);
        }
        cmp::min(self.len(), end-start)
    }

    /// Deprecated: use `&s[start .. end]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[start .. end] instead")]
    #[inline]
    pub fn slice(&self, start: usize, end: usize) -> &[T] {
        &self[start .. end]
    }

    /// Deprecated: use `&s[start..]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[start..] instead")]
    #[inline]
    pub fn slice_from(&self, start: usize) -> &[T] {
        &self[start ..]
    }

    /// Deprecated: use `&s[..end]` notation instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &s[..end] instead")]
    #[inline]
    pub fn slice_to(&self, end: usize) -> &[T] {
        &self[.. end]
    }

    /// Divides one slice into two at an index.
    ///
    /// The first will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself) and the second will contain all
    /// indices from `[mid, len)` (excluding the index `len` itself).
    ///
    /// Panics if `mid > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 50];
    /// let (v1, v2) = v.split_at(2);
    /// assert_eq!([10, 40], v1);
    /// assert_eq!([30, 20, 50], v2);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split_at(&self, mid: usize) -> (&[T], &[T]) {
        core_slice::SliceExt::split_at(self, mid)
    }

    /// Returns an iterator over the slice.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        core_slice::SliceExt::iter(self)
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`.  The matched element is not contained in the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split by numbers divisible by 3 (i.e. `[10, 40]`,
    /// `[20]`, `[50]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.split(|num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split<F>(&self, pred: F) -> Split<T, F> where F: FnMut(&T) -> bool {
        core_slice::SliceExt::split(self, pred)
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, limited to splitting at most `n` times.  The matched element is
    /// not contained in the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split once by numbers divisible by 3 (i.e. `[10, 40]`,
    /// `[20, 60, 50]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.splitn(1, |num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn splitn<F>(&self, n: usize, pred: F) -> SplitN<T, F> where F: FnMut(&T) -> bool {
        core_slice::SliceExt::splitn(self, n, pred)
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to splitting at most `n` times. This starts at the end of
    /// the slice and works backwards.  The matched element is not contained in
    /// the subslices.
    ///
    /// # Examples
    ///
    /// Print the slice split once, starting from the end, by numbers divisible
    /// by 3 (i.e. `[50]`, `[10, 40, 30, 20]`):
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    /// for group in v.rsplitn(1, |num| *num % 3 == 0) {
    ///     println!("{:?}", group);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<T, F> where F: FnMut(&T) -> bool {
        core_slice::SliceExt::rsplitn(self, n, pred)
    }

    /// Returns an iterator over all contiguous windows of length
    /// `size`. The windows overlap. If the slice is shorter than
    /// `size`, the iterator returns no values.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    ///
    /// # Example
    ///
    /// Print the adjacent pairs of a slice (i.e. `[1,2]`, `[2,3]`,
    /// `[3,4]`):
    ///
    /// ```rust
    /// let v = &[1, 2, 3, 4];
    /// for win in v.windows(2) {
    ///     println!("{:?}", win);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn windows(&self, size: usize) -> Windows<T> {
        core_slice::SliceExt::windows(self, size)
    }

    /// Returns an iterator over `size` elements of the slice at a
    /// time. The chunks do not overlap. If `size` does not divide the
    /// length of the slice, then the last chunk will not have length
    /// `size`.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    ///
    /// # Example
    ///
    /// Print the slice two elements at a time (i.e. `[1,2]`,
    /// `[3,4]`, `[5]`):
    ///
    /// ```rust
    /// let v = &[1, 2, 3, 4, 5];
    /// for win in v.chunks(2) {
    ///     println!("{:?}", win);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn chunks(&self, size: usize) -> Chunks<T> {
        core_slice::SliceExt::chunks(self, size)
    }

    /// Returns the element of a slice at the given index, or `None` if the
    /// index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&40), v.get(1));
    /// assert_eq!(None, v.get(3));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        core_slice::SliceExt::get(self, index)
    }

    /// Returns the first element of a slice, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&10), v.first());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.first());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn first(&self) -> Option<&T> {
        core_slice::SliceExt::first(self)
    }

    /// Returns all but the first element of a slice.
    #[unstable(feature = "collections", reason = "likely to be renamed")]
    #[inline]
    pub fn tail(&self) -> &[T] {
        core_slice::SliceExt::tail(self)
    }

    /// Returns all but the last element of a slice.
    #[unstable(feature = "collections", reason = "likely to be renamed")]
    #[inline]
    pub fn init(&self) -> &[T] {
        core_slice::SliceExt::init(self)
    }

    /// Returns the last element of a slice, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&30), v.last());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.last());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn last(&self) -> Option<&T> {
        core_slice::SliceExt::last(self)
    }

    /// Returns a pointer to the element at the given index, without doing
    /// bounds checking.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        core_slice::SliceExt::get_unchecked(self, index)
    }

    /// Returns an unsafe pointer to the slice's buffer
    ///
    /// The caller must ensure that the slice outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the slice may cause its buffer to be reallocated, which
    /// would also make any pointers to it invalid.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        core_slice::SliceExt::as_ptr(self)
    }

    /// Binary search a sorted slice with a comparator function.
    ///
    /// The comparator function should implement an order consistent
    /// with the sort order of the underlying slice, returning an
    /// order code that indicates whether its argument is `Less`,
    /// `Equal` or `Greater` the desired target.
    ///
    /// If a matching value is found then returns `Ok`, containing
    /// the index for the matched element; if no match is found then
    /// `Err` is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// # Example
    ///
    /// Looks up a series of four elements. The first is found, with a
    /// uniquely determined position; the second and third are not
    /// found; the fourth could match any position in `[1,4]`.
    ///
    /// ```rust
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let s = s.as_slice();
    ///
    /// let seek = 13;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Ok(9));
    /// let seek = 4;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(7));
    /// let seek = 100;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(13));
    /// let seek = 1;
    /// let r = s.binary_search_by(|probe| probe.cmp(&seek));
    /// assert!(match r { Ok(1...4) => true, _ => false, });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn binary_search_by<F>(&self, f: F) -> Result<usize, usize> where F: FnMut(&T) -> Ordering {
        core_slice::SliceExt::binary_search_by(self, f)
    }

    /// Return the number of elements in the slice
    ///
    /// # Example
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.len(), 3);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn len(&self) -> usize {
        core_slice::SliceExt::len(self)
    }

    /// Returns true if the slice has a length of 0
    ///
    /// # Example
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert!(!a.is_empty());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is_empty(&self) -> bool {
        core_slice::SliceExt::is_empty(self)
    }

    /// Returns a mutable reference to the element at the given index,
    /// or `None` if the index is out of bounds
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        core_slice::SliceExt::get_mut(self, index)
    }

    /// Work with `self` as a mut slice.
    /// Primarily intended for getting a &mut [T] from a [T; N].
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        core_slice::SliceExt::as_mut_slice(self)
    }

    /// Deprecated: use `&mut s[start .. end]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[start .. end] instead")]
    #[inline]
    pub fn slice_mut(&mut self, start: usize, end: usize) -> &mut [T] {
        &mut self[start .. end]
    }

    /// Deprecated: use `&mut s[start ..]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[start ..] instead")]
    #[inline]
    pub fn slice_from_mut(&mut self, start: usize) -> &mut [T] {
        &mut self[start ..]
    }

    /// Deprecated: use `&mut s[.. end]` instead.
    #[unstable(feature = "collections",
               reason = "will be replaced by slice syntax")]
    #[deprecated(since = "1.0.0", reason = "use &mut s[.. end] instead")]
    #[inline]
    pub fn slice_to_mut(&mut self, end: usize) -> &mut [T] {
        &mut self[.. end]
    }

    /// Returns an iterator that allows modifying each value
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        core_slice::SliceExt::iter_mut(self)
    }

    /// Returns a mutable pointer to the first element of a slice, or `None` if it is empty
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut T> {
        core_slice::SliceExt::first_mut(self)
    }

    /// Returns all but the first element of a mutable slice
    #[unstable(feature = "collections",
               reason = "likely to be renamed or removed")]
    #[inline]
    pub fn tail_mut(&mut self) -> &mut [T] {
        core_slice::SliceExt::tail_mut(self)
    }

    /// Returns all but the last element of a mutable slice
    #[unstable(feature = "collections",
               reason = "likely to be renamed or removed")]
    #[inline]
    pub fn init_mut(&mut self) -> &mut [T] {
        core_slice::SliceExt::init_mut(self)
    }

    /// Returns a mutable pointer to the last item in the slice.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        core_slice::SliceExt::last_mut(self)
    }

    /// Returns an iterator over mutable subslices separated by elements that
    /// match `pred`.  The matched element is not contained in the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split_mut<F>(&mut self, pred: F) -> SplitMut<T, F> where F: FnMut(&T) -> bool {
        core_slice::SliceExt::split_mut(self, pred)
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, limited to splitting at most `n` times.  The matched element is
    /// not contained in the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<T, F>
                         where F: FnMut(&T) -> bool {
        core_slice::SliceExt::splitn_mut(self, n, pred)
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to splitting at most `n` times. This starts at the end of
    /// the slice and works backwards.  The matched element is not contained in
    /// the subslices.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplitn_mut<F>(&mut self,  n: usize, pred: F) -> RSplitNMut<T, F>
                      where F: FnMut(&T) -> bool {
        core_slice::SliceExt::rsplitn_mut(self, n, pred)
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time.
    /// The chunks are mutable and do not overlap. If `chunk_size` does
    /// not divide the length of the slice, then the last chunk will not
    /// have length `chunk_size`.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_size` is 0.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<T> {
        core_slice::SliceExt::chunks_mut(self, chunk_size)
    }

    /// Swaps two elements in a slice.
    ///
    /// # Arguments
    ///
    /// * a - The index of the first element
    /// * b - The index of the second element
    ///
    /// # Panics
    ///
    /// Panics if `a` or `b` are out of bounds.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut v = ["a", "b", "c", "d"];
    /// v.swap(1, 3);
    /// assert!(v == ["a", "d", "c", "b"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        core_slice::SliceExt::swap(self, a, b)
    }

    /// Divides one `&mut` into two at an index.
    ///
    /// The first will contain all indices from `[0, mid)` (excluding
    /// the index `mid` itself) and the second will contain all
    /// indices from `[mid, len)` (excluding the index `len` itself).
    ///
    /// # Panics
    ///
    /// Panics if `mid > len`.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut v = [1, 2, 3, 4, 5, 6];
    ///
    /// // scoped to restrict the lifetime of the borrows
    /// {
    ///    let (left, right) = v.split_at_mut(0);
    ///    assert!(left == []);
    ///    assert!(right == [1, 2, 3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_mut(2);
    ///     assert!(left == [1, 2]);
    ///     assert!(right == [3, 4, 5, 6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_mut(6);
    ///     assert!(left == [1, 2, 3, 4, 5, 6]);
    ///     assert!(right == []);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split_at_mut(&mut self, mid: usize) -> (&mut [T], &mut [T]) {
        core_slice::SliceExt::split_at_mut(self, mid)
    }

    /// Reverse the order of elements in a slice, in place.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut v = [1, 2, 3];
    /// v.reverse();
    /// assert!(v == [3, 2, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn reverse(&mut self) {
        core_slice::SliceExt::reverse(self)
    }

    /// Returns an unsafe mutable pointer to the element in index
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        core_slice::SliceExt::get_unchecked_mut(self, index)
    }

    /// Return an unsafe mutable pointer to the slice's buffer.
    ///
    /// The caller must ensure that the slice outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the slice may cause its buffer to be reallocated, which
    /// would also make any pointers to it invalid.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        core_slice::SliceExt::as_mut_ptr(self)
    }

    /// Copies `self` into a new `Vec`.
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn to_vec(&self) -> Vec<T> where T: Clone {
        let mut vector = Vec::with_capacity(self.len());
        vector.push_all(self);
        vector
    }

    /// Creates an iterator that yields every possible permutation of the
    /// vector in succession.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let v = [1, 2, 3];
    /// let mut perms = v.permutations();
    ///
    /// for p in perms {
    ///   println!("{:?}", p);
    /// }
    /// ```
    ///
    /// Iterating through permutations one by one.
    ///
    /// ```rust
    /// let v = [1, 2, 3];
    /// let mut perms = v.permutations();
    ///
    /// assert_eq!(Some(vec![1, 2, 3]), perms.next());
    /// assert_eq!(Some(vec![1, 3, 2]), perms.next());
    /// assert_eq!(Some(vec![3, 1, 2]), perms.next());
    /// ```
    #[unstable(feature = "collections")]
    pub fn permutations(&self) -> Permutations<T> where T: Clone {
        Permutations{
            swaps: ElementSwaps::new(self.len()),
            v: self.to_vec(),
        }
    }

    /// Copies as many elements from `src` as it can into `self` (the
    /// shorter of `self.len()` and `src.len()`). Returns the number
    /// of elements copied.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut dst = [0, 0, 0];
    /// let src = [1, 2];
    ///
    /// assert!(dst.clone_from_slice(&src) == 2);
    /// assert!(dst == [1, 2, 0]);
    ///
    /// let src2 = [3, 4, 5, 6];
    /// assert!(dst.clone_from_slice(&src2) == 3);
    /// assert!(dst == [3, 4, 5]);
    /// ```
    #[unstable(feature = "collections")]
    pub fn clone_from_slice(&mut self, src: &[T]) -> usize where T: Clone {
        core_slice::SliceExt::clone_from_slice(self, src)
    }

    /// Sorts the slice, in place.
    ///
    /// This is equivalent to `self.sort_by(|a, b| a.cmp(b))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut v = [-5, 4, 1, -3, 2];
    ///
    /// v.sort();
    /// assert!(v == [-5, -3, 1, 2, 4]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sort(&mut self) where T: Ord {
        self.sort_by(|a, b| a.cmp(b))
    }

    /// Binary search a sorted slice for a given element.
    ///
    /// If the value is found then `Ok` is returned, containing the
    /// index of the matching element; if the value is not found then
    /// `Err` is returned, containing the index where a matching
    /// element could be inserted while maintaining sorted order.
    ///
    /// # Example
    ///
    /// Looks up a series of four elements. The first is found, with a
    /// uniquely determined position; the second and third are not
    /// found; the fourth could match any position in `[1,4]`.
    ///
    /// ```rust
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let s = s.as_slice();
    ///
    /// assert_eq!(s.binary_search(&13),  Ok(9));
    /// assert_eq!(s.binary_search(&4),   Err(7));
    /// assert_eq!(s.binary_search(&100), Err(13));
    /// let r = s.binary_search(&1);
    /// assert!(match r { Ok(1...4) => true, _ => false, });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize> where T: Ord {
        core_slice::SliceExt::binary_search(self, x)
    }

    /// Deprecated: use `binary_search` instead.
    #[unstable(feature = "collections")]
    #[deprecated(since = "1.0.0", reason = "use binary_search instead")]
    pub fn binary_search_elem(&self, x: &T) -> Result<usize, usize> where T: Ord {
        self.binary_search(x)
    }

    /// Mutates the slice to the next lexicographic permutation.
    ///
    /// Returns `true` if successful and `false` if the slice is at the
    /// last-ordered permutation.
    ///
    /// # Example
    ///
    /// ```rust
    /// let v: &mut [_] = &mut [0, 1, 2];
    /// v.next_permutation();
    /// let b: &mut [_] = &mut [0, 2, 1];
    /// assert!(v == b);
    /// v.next_permutation();
    /// let b: &mut [_] = &mut [1, 0, 2];
    /// assert!(v == b);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain if this merits inclusion in std")]
    pub fn next_permutation(&mut self) -> bool where T: Ord {
        core_slice::SliceExt::next_permutation(self)
    }

    /// Mutates the slice to the previous lexicographic permutation.
    ///
    /// Returns `true` if successful and `false` if the slice is at the
    /// first-ordered permutation.
    ///
    /// # Example
    ///
    /// ```rust
    /// let v: &mut [_] = &mut [1, 0, 2];
    /// v.prev_permutation();
    /// let b: &mut [_] = &mut [0, 2, 1];
    /// assert!(v == b);
    /// v.prev_permutation();
    /// let b: &mut [_] = &mut [0, 1, 2];
    /// assert!(v == b);
    /// ```
    #[unstable(feature = "collections",
               reason = "uncertain if this merits inclusion in std")]
    pub fn prev_permutation(&mut self) -> bool where T: Ord {
        core_slice::SliceExt::prev_permutation(self)
    }

    /// Find the first index containing a matching value.
    #[unstable(feature = "collections")]
    pub fn position_elem(&self, t: &T) -> Option<usize> where T: PartialEq {
        core_slice::SliceExt::position_elem(self, t)
    }

    /// Find the last index containing a matching value.
    #[unstable(feature = "collections")]
    pub fn rposition_elem(&self, t: &T) -> Option<usize> where T: PartialEq {
        core_slice::SliceExt::rposition_elem(self, t)
    }

    /// Returns true if the slice contains an element with the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.contains(&30));
    /// assert!(!v.contains(&50));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn contains(&self, x: &T) -> bool where T: PartialEq {
        core_slice::SliceExt::contains(self, x)
    }

    /// Returns true if `needle` is a prefix of the slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.starts_with(&[10]));
    /// assert!(v.starts_with(&[10, 40]));
    /// assert!(!v.starts_with(&[50]));
    /// assert!(!v.starts_with(&[10, 50]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn starts_with(&self, needle: &[T]) -> bool where T: PartialEq {
        core_slice::SliceExt::starts_with(self, needle)
    }

    /// Returns true if `needle` is a suffix of the slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.ends_with(&[30]));
    /// assert!(v.ends_with(&[40, 30]));
    /// assert!(!v.ends_with(&[50]));
    /// assert!(!v.ends_with(&[50, 30]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn ends_with(&self, needle: &[T]) -> bool where T: PartialEq {
        core_slice::SliceExt::ends_with(self, needle)
    }

    /// Convert `self` into a vector without clones or allocation.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_vec(mut self: Box<Self>) -> Vec<T> {
        unsafe {
            let xs = Vec::from_raw_parts(self.as_mut_ptr(), self.len(), self.len());
            mem::forget(self);
            xs
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Extension traits for slices over specific kinds of data
////////////////////////////////////////////////////////////////////////////////
#[unstable(feature = "collections", reason = "U should be an associated type")]
/// An extension trait for concatenating slices
pub trait SliceConcatExt<T: ?Sized, U> {
    /// Flattens a slice of `T` into a single value `U`.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = vec!["hello", "world"];
    ///
    /// let s: String = v.concat();
    ///
    /// println!("{}", s); // prints "helloworld"
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn concat(&self) -> U;

    /// Flattens a slice of `T` into a single value `U`, placing a given separator between each.
    ///
    /// # Examples
    ///
    /// ```
    /// let v = vec!["hello", "world"];
    ///
    /// let s: String = v.connect(" ");
    ///
    /// println!("{}", s); // prints "hello world"
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn connect(&self, sep: &T) -> U;
}

impl<T: Clone, V: AsSlice<T>> SliceConcatExt<T, Vec<T>> for [V] {
    fn concat(&self) -> Vec<T> {
        let size = self.iter().fold(0, |acc, v| acc + v.as_slice().len());
        let mut result = Vec::with_capacity(size);
        for v in self {
            result.push_all(v.as_slice())
        }
        result
    }

    fn connect(&self, sep: &T) -> Vec<T> {
        let size = self.iter().fold(0, |acc, v| acc + v.as_slice().len());
        let mut result = Vec::with_capacity(size + self.len());
        let mut first = true;
        for v in self {
            if first { first = false } else { result.push(sep.clone()) }
            result.push_all(v.as_slice())
        }
        result
    }
}

/// An iterator that yields the element swaps needed to produce
/// a sequence of all possible permutations for an indexed sequence of
/// elements. Each permutation is only a single swap apart.
///
/// The Steinhaus-Johnson-Trotter algorithm is used.
///
/// Generates even and odd permutations alternately.
///
/// The last generated swap is always (0, 1), and it returns the
/// sequence to its initial order.
#[unstable(feature = "collections")]
#[derive(Clone)]
pub struct ElementSwaps {
    sdir: Vec<SizeDirection>,
    /// If `true`, emit the last swap that returns the sequence to initial
    /// state.
    emit_reset: bool,
    swaps_made : usize,
}

impl ElementSwaps {
    /// Creates an `ElementSwaps` iterator for a sequence of `length` elements.
    #[unstable(feature = "collections")]
    pub fn new(length: usize) -> ElementSwaps {
        // Initialize `sdir` with a direction that position should move in
        // (all negative at the beginning) and the `size` of the
        // element (equal to the original index).
        ElementSwaps{
            emit_reset: true,
            sdir: (0..length).map(|i| SizeDirection{ size: i, dir: Neg }).collect(),
            swaps_made: 0
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Standard trait implementations for slices
////////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Borrow<[T]> for Vec<T> {
    fn borrow(&self) -> &[T] { &self[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> BorrowMut<[T]> for Vec<T> {
    fn borrow_mut(&mut self) -> &mut [T] { &mut self[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Clone> ToOwned for [T] {
    type Owned = Vec<T>;
    fn to_owned(&self) -> Vec<T> { self.to_vec() }
}

////////////////////////////////////////////////////////////////////////////////
// Iterators
////////////////////////////////////////////////////////////////////////////////

#[derive(Copy, Clone)]
enum Direction { Pos, Neg }

/// An `Index` and `Direction` together.
#[derive(Copy, Clone)]
struct SizeDirection {
    size: usize,
    dir: Direction,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for ElementSwaps {
    type Item = (usize, usize);

    // #[inline]
    fn next(&mut self) -> Option<(usize, usize)> {
        fn new_pos_wrapping(i: usize, s: Direction) -> usize {
            i.wrapping_add(match s { Pos => 1, Neg => -1 })
        }

        fn new_pos(i: usize, s: Direction) -> usize {
            match s { Pos => i + 1, Neg => i - 1 }
        }

        // Find the index of the largest mobile element:
        // The direction should point into the vector, and the
        // swap should be with a smaller `size` element.
        let max = self.sdir.iter().cloned().enumerate()
                           .filter(|&(i, sd)|
                                new_pos_wrapping(i, sd.dir) < self.sdir.len() &&
                                self.sdir[new_pos(i, sd.dir)].size < sd.size)
                           .max_by(|&(_, sd)| sd.size);
        match max {
            Some((i, sd)) => {
                let j = new_pos(i, sd.dir);
                self.sdir.swap(i, j);

                // Swap the direction of each larger SizeDirection
                for x in &mut self.sdir {
                    if x.size > sd.size {
                        x.dir = match x.dir { Pos => Neg, Neg => Pos };
                    }
                }
                self.swaps_made += 1;
                Some((i, j))
            },
            None => if self.emit_reset {
                self.emit_reset = false;
                if self.sdir.len() > 1 {
                    // The last swap
                    self.swaps_made += 1;
                    Some((0, 1))
                } else {
                    // Vector is of the form [] or [x], and the only permutation is itself
                    self.swaps_made += 1;
                    Some((0,0))
                }
            } else { None }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // For a vector of size n, there are exactly n! permutations.
        let n = (2..self.sdir.len() + 1).product();
        (n - self.swaps_made, Some(n - self.swaps_made))
    }
}

/// An iterator that uses `ElementSwaps` to iterate through
/// all possible permutations of a vector.
///
/// The first iteration yields a clone of the vector as it is,
/// then each successive element is the vector with one
/// swap applied.
///
/// Generates even and odd permutations alternately.
#[unstable(feature = "collections")]
pub struct Permutations<T> {
    swaps: ElementSwaps,
    v: Vec<T>,
}

#[unstable(feature = "collections", reason = "trait is unstable")]
impl<T: Clone> Iterator for Permutations<T> {
    type Item = Vec<T>;

    #[inline]
    fn next(&mut self) -> Option<Vec<T>> {
        match self.swaps.next() {
            None => None,
            Some((0,0)) => Some(self.v.clone()),
            Some((a, b)) => {
                let elt = self.v.clone();
                self.v.swap(a, b);
                Some(elt)
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.swaps.size_hint()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Sorting
////////////////////////////////////////////////////////////////////////////////

fn insertion_sort<T, F>(v: &mut [T], mut compare: F) where F: FnMut(&T, &T) -> Ordering {
    let len = v.len() as isize;
    let buf_v = v.as_mut_ptr();

    // 1 <= i < len;
    for i in 1..len {
        // j satisfies: 0 <= j <= i;
        let mut j = i;
        unsafe {
            // `i` is in bounds.
            let read_ptr = buf_v.offset(i) as *const T;

            // find where to insert, we need to do strict <,
            // rather than <=, to maintain stability.

            // 0 <= j - 1 < len, so .offset(j - 1) is in bounds.
            while j > 0 &&
                    compare(&*read_ptr, &*buf_v.offset(j - 1)) == Less {
                j -= 1;
            }

            // shift everything to the right, to make space to
            // insert this value.

            // j + 1 could be `len` (for the last `i`), but in
            // that case, `i == j` so we don't copy. The
            // `.offset(j)` is always in bounds.

            if i != j {
                let tmp = ptr::read(read_ptr);
                ptr::copy(buf_v.offset(j + 1),
                          &*buf_v.offset(j),
                          (i - j) as usize);
                ptr::copy_nonoverlapping(buf_v.offset(j), &tmp, 1);
                mem::forget(tmp);
            }
        }
    }
}

fn merge_sort<T, F>(v: &mut [T], mut compare: F) where F: FnMut(&T, &T) -> Ordering {
    // warning: this wildly uses unsafe.
    const BASE_INSERTION: usize = 32;
    const LARGE_INSERTION: usize = 16;

    // FIXME #12092: smaller insertion runs seems to make sorting
    // vectors of large elements a little faster on some platforms,
    // but hasn't been tested/tuned extensively
    let insertion = if size_of::<T>() <= 16 {
        BASE_INSERTION
    } else {
        LARGE_INSERTION
    };

    let len = v.len();

    // short vectors get sorted in-place via insertion sort to avoid allocations
    if len <= insertion {
        insertion_sort(v, compare);
        return;
    }

    // allocate some memory to use as scratch memory, we keep the
    // length 0 so we can keep shallow copies of the contents of `v`
    // without risking the dtors running on an object twice if
    // `compare` panics.
    let mut working_space = Vec::with_capacity(2 * len);
    // these both are buffers of length `len`.
    let mut buf_dat = working_space.as_mut_ptr();
    let mut buf_tmp = unsafe {buf_dat.offset(len as isize)};

    // length `len`.
    let buf_v = v.as_ptr();

    // step 1. sort short runs with insertion sort. This takes the
    // values from `v` and sorts them into `buf_dat`, leaving that
    // with sorted runs of length INSERTION.

    // We could hardcode the sorting comparisons here, and we could
    // manipulate/step the pointers themselves, rather than repeatedly
    // .offset-ing.
    for start in (0.. len).step_by(insertion) {
        // start <= i < len;
        for i in start..cmp::min(start + insertion, len) {
            // j satisfies: start <= j <= i;
            let mut j = i as isize;
            unsafe {
                // `i` is in bounds.
                let read_ptr = buf_v.offset(i as isize);

                // find where to insert, we need to do strict <,
                // rather than <=, to maintain stability.

                // start <= j - 1 < len, so .offset(j - 1) is in
                // bounds.
                while j > start as isize &&
                        compare(&*read_ptr, &*buf_dat.offset(j - 1)) == Less {
                    j -= 1;
                }

                // shift everything to the right, to make space to
                // insert this value.

                // j + 1 could be `len` (for the last `i`), but in
                // that case, `i == j` so we don't copy. The
                // `.offset(j)` is always in bounds.
                ptr::copy(buf_dat.offset(j + 1),
                          &*buf_dat.offset(j),
                          i - j as usize);
                ptr::copy_nonoverlapping(buf_dat.offset(j), read_ptr, 1);
            }
        }
    }

    // step 2. merge the sorted runs.
    let mut width = insertion;
    while width < len {
        // merge the sorted runs of length `width` in `buf_dat` two at
        // a time, placing the result in `buf_tmp`.

        // 0 <= start <= len.
        for start in (0..len).step_by(2 * width) {
            // manipulate pointers directly for speed (rather than
            // using a `for` loop with `range` and `.offset` inside
            // that loop).
            unsafe {
                // the end of the first run & start of the
                // second. Offset of `len` is defined, since this is
                // precisely one byte past the end of the object.
                let right_start = buf_dat.offset(cmp::min(start + width, len) as isize);
                // end of the second. Similar reasoning to the above re safety.
                let right_end_idx = cmp::min(start + 2 * width, len);
                let right_end = buf_dat.offset(right_end_idx as isize);

                // the pointers to the elements under consideration
                // from the two runs.

                // both of these are in bounds.
                let mut left = buf_dat.offset(start as isize);
                let mut right = right_start;

                // where we're putting the results, it is a run of
                // length `2*width`, so we step it once for each step
                // of either `left` or `right`.  `buf_tmp` has length
                // `len`, so these are in bounds.
                let mut out = buf_tmp.offset(start as isize);
                let out_end = buf_tmp.offset(right_end_idx as isize);

                while out < out_end {
                    // Either the left or the right run are exhausted,
                    // so just copy the remainder from the other run
                    // and move on; this gives a huge speed-up (order
                    // of 25%) for mostly sorted vectors (the best
                    // case).
                    if left == right_start {
                        // the number remaining in this run.
                        let elems = (right_end as usize - right as usize) / mem::size_of::<T>();
                        ptr::copy_nonoverlapping(out, &*right, elems);
                        break;
                    } else if right == right_end {
                        let elems = (right_start as usize - left as usize) / mem::size_of::<T>();
                        ptr::copy_nonoverlapping(out, &*left, elems);
                        break;
                    }

                    // check which side is smaller, and that's the
                    // next element for the new run.

                    // `left < right_start` and `right < right_end`,
                    // so these are valid.
                    let to_copy = if compare(&*left, &*right) == Greater {
                        step(&mut right)
                    } else {
                        step(&mut left)
                    };
                    ptr::copy_nonoverlapping(out, &*to_copy, 1);
                    step(&mut out);
                }
            }
        }

        mem::swap(&mut buf_dat, &mut buf_tmp);

        width *= 2;
    }

    // write the result to `v` in one go, so that there are never two copies
    // of the same object in `v`.
    unsafe {
        ptr::copy_nonoverlapping(v.as_mut_ptr(), &*buf_dat, len);
    }

    // increment the pointer, returning the old pointer.
    #[inline(always)]
    unsafe fn step<T>(ptr: &mut *mut T) -> *mut T {
        let old = *ptr;
        *ptr = ptr.offset(1);
        old
    }
}
