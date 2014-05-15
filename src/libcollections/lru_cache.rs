// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


//! A cache that holds a limited number of key-value pairs. When the
//! capacity of the cache is exceeded, the least-recently-used
//! (where "used" means a look-up or putting the pair into the cache)
//! pair is automatically removed.
//!
//! # Example
//!
//! ```rust
//! use collections::LruCache;
//!
//! let mut cache: LruCache<int, int> = LruCache::new(2);
//! cache.put(1, 10);
//! cache.put(2, 20);
//! cache.put(3, 30);
//! assert!(cache.get(&1).is_none());
//! assert_eq!(*cache.get(&2).unwrap(), 20);
//! assert_eq!(*cache.get(&3).unwrap(), 30);
//!
//! cache.put(2, 22);
//! assert_eq!(*cache.get(&2).unwrap(), 22);
//!
//! cache.put(6, 60);
//! assert!(cache.get(&3).is_none());
//!
//! cache.change_capacity(1);
//! assert!(cache.get(&2).is_none());
//! ```

use std::container::Container;
use std::hash::Hash;
use std::fmt;
use std::mem;
use std::ptr;

use HashMap;

struct KeyRef<K> { k: *K }

struct LruEntry<K, V> {
    next: *mut LruEntry<K, V>,
    prev: *mut LruEntry<K, V>,
    key: K,
    value: V,
}

/// An LRU Cache.
pub struct LruCache<K, V> {
    map: HashMap<KeyRef<K>, Box<LruEntry<K, V>>>,
    max_size: uint,
    head: *mut LruEntry<K, V>,
}

impl<S, K: Hash<S>> Hash<S> for KeyRef<K> {
    fn hash(&self, state: &mut S) {
        unsafe { (*self.k).hash(state) }
    }
}

impl<K: Eq> Eq for KeyRef<K> {
    fn eq(&self, other: &KeyRef<K>) -> bool {
        unsafe{ (*self.k).eq(&*other.k) }
    }
}

impl<K: TotalEq> TotalEq for KeyRef<K> {}

impl<K, V> LruEntry<K, V> {
    fn new(k: K, v: V) -> LruEntry<K, V> {
        LruEntry {
            key: k,
            value: v,
            next: ptr::mut_null(),
            prev: ptr::mut_null(),
        }
    }
}

impl<K: Hash + TotalEq, V> LruCache<K, V> {
    /// Create an LRU Cache that holds at most `capacity` items.
    pub fn new(capacity: uint) -> LruCache<K, V> {
        let cache = LruCache {
            map: HashMap::new(),
            max_size: capacity,
            head: unsafe{ mem::transmute(box mem::uninit::<LruEntry<K, V>>()) },
        };
        unsafe {
            (*cache.head).next = cache.head;
            (*cache.head).prev = cache.head;
        }
        return cache;
    }

    /// Put a key-value pair into cache.
    pub fn put(&mut self, k: K, v: V) {
        let (node_ptr, node_opt) = match self.map.find_mut(&KeyRef{k: &k}) {
            Some(node) => {
                node.value = v;
                let node_ptr: *mut LruEntry<K, V> = &mut **node;
                (node_ptr, None)
            }
            None => {
                let mut node = box LruEntry::new(k, v);
                let node_ptr: *mut LruEntry<K, V> = &mut *node;
                (node_ptr, Some(node))
            }
        };
        match node_opt {
            None => {
                // Existing node, just update LRU position
                self.detach(node_ptr);
                self.attach(node_ptr);
            }
            Some(node) => {
                let keyref = unsafe { &(*node_ptr).key };
                self.map.swap(KeyRef{k: keyref}, node);
                self.attach(node_ptr);
                if self.len() > self.capacity() {
                    self.remove_lru();
                }
            }
        }
    }

    /// Return a value corresponding to the key in the cache.
    pub fn get<'a>(&'a mut self, k: &K) -> Option<&'a V> {
        let (value, node_ptr_opt) = match self.map.find_mut(&KeyRef{k: k}) {
            None => (None, None),
            Some(node) => {
                let node_ptr: *mut LruEntry<K, V> = &mut **node;
                (Some(unsafe { &(*node_ptr).value }), Some(node_ptr))
            }
        };
        match node_ptr_opt {
            None => (),
            Some(node_ptr) => {
                self.detach(node_ptr);
                self.attach(node_ptr);
            }
        }
        return value;
    }

    /// Remove and return a value corresponding to the key from the cache.
    pub fn pop(&mut self, k: &K) -> Option<V> {
        match self.map.pop(&KeyRef{k: k}) {
            None => None,
            Some(lru_entry) => Some(lru_entry.value)
        }
    }

    /// Return the maximum number of key-value pairs the cache can hold.
    pub fn capacity(&self) -> uint {
        self.max_size
    }

    /// Change the number of key-value pairs the cache can hold. Remove
    /// least-recently-used key-value pairs if necessary.
    pub fn change_capacity(&mut self, capacity: uint) {
        for _ in range(capacity, self.len()) {
            self.remove_lru();
        }
        self.max_size = capacity;
    }

    #[inline]
    fn remove_lru(&mut self) {
        if self.len() > 0 {
            let lru = unsafe { (*self.head).prev };
            self.detach(lru);
            self.map.pop(&KeyRef{k: unsafe { &(*lru).key }});
        }
    }

    #[inline]
    fn detach(&mut self, node: *mut LruEntry<K, V>) {
        unsafe {
            (*(*node).prev).next = (*node).next;
            (*(*node).next).prev = (*node).prev;
        }
    }

    #[inline]
    fn attach(&mut self, node: *mut LruEntry<K, V>) {
        unsafe {
            (*node).next = (*self.head).next;
            (*node).prev = self.head;
            (*self.head).next = node;
            (*(*node).next).prev = node;
        }
    }
}

impl<A: fmt::Show + Hash + TotalEq, B: fmt::Show> fmt::Show for LruCache<A, B> {
    /// Return a string that lists the key-value pairs from most-recently
    /// used to least-recently used.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f.buf, r"\{"));
        let mut cur = self.head;
        for i in range(0, self.len()) {
            if i > 0 { try!(write!(f.buf, ", ")) }
            unsafe {
                cur = (*cur).next;
                try!(write!(f.buf, "{}", (*cur).key));
            }
            try!(write!(f.buf, ": "));
            unsafe {
                try!(write!(f.buf, "{}", (*cur).value));
            }
        }
        write!(f.buf, r"\}")
    }
}

impl<K: Hash + TotalEq, V> Container for LruCache<K, V> {
    /// Return the number of key-value pairs in the cache.
    fn len(&self) -> uint {
        self.map.len()
    }
}

impl<K: Hash + TotalEq, V> Mutable for LruCache<K, V> {
    /// Clear the cache of all key-value pairs.
    fn clear(&mut self) {
        self.map.clear();
    }
}

#[unsafe_destructor]
impl<K, V> Drop for LruCache<K, V> {
    fn drop(&mut self) {
        unsafe {
            let node: Box<LruEntry<K, V>> = mem::transmute(self.head);
            // Prevent compiler from trying to drop the un-initialized field in the sigil node.
            let box LruEntry { key: k, value: v, .. } = node;
            mem::forget(k);
            mem::forget(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;

    fn assert_opt_eq<V: Eq>(opt: Option<&V>, v: V) {
        assert!(opt.is_some());
        assert!(opt.unwrap() == &v);
    }

    #[test]
    fn test_put_and_get() {
        let mut cache: LruCache<int, int> = LruCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        assert_opt_eq(cache.get(&1), 10);
        assert_opt_eq(cache.get(&2), 20);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_put_update() {
        let mut cache: LruCache<StrBuf, Vec<u8>> = LruCache::new(1);
        cache.put("1".to_strbuf(), vec![10, 10]);
        cache.put("1".to_strbuf(), vec![10, 19]);
        assert_opt_eq(cache.get(&"1".to_strbuf()), vec![10, 19]);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_expire_lru() {
        let mut cache: LruCache<StrBuf, StrBuf> = LruCache::new(2);
        cache.put("foo1".to_strbuf(), "bar1".to_strbuf());
        cache.put("foo2".to_strbuf(), "bar2".to_strbuf());
        cache.put("foo3".to_strbuf(), "bar3".to_strbuf());
        assert!(cache.get(&"foo1".to_strbuf()).is_none());
        cache.put("foo2".to_strbuf(), "bar2update".to_strbuf());
        cache.put("foo4".to_strbuf(), "bar4".to_strbuf());
        assert!(cache.get(&"foo3".to_strbuf()).is_none());
    }

    #[test]
    fn test_pop() {
        let mut cache: LruCache<int, int> = LruCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        assert_eq!(cache.len(), 2);
        let opt1 = cache.pop(&1);
        assert!(opt1.is_some());
        assert_eq!(opt1.unwrap(), 10);
        assert!(cache.get(&1).is_none());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_change_capacity() {
        let mut cache: LruCache<int, int> = LruCache::new(2);
        assert_eq!(cache.capacity(), 2);
        cache.put(1, 10);
        cache.put(2, 20);
        cache.change_capacity(1);
        assert!(cache.get(&1).is_none());
        assert_eq!(cache.capacity(), 1);
    }

    #[test]
    fn test_to_str() {
        let mut cache: LruCache<int, int> = LruCache::new(3);
        cache.put(1, 10);
        cache.put(2, 20);
        cache.put(3, 30);
        assert_eq!(cache.to_str(), "{3: 30, 2: 20, 1: 10}".to_owned());
        cache.put(2, 22);
        assert_eq!(cache.to_str(), "{2: 22, 3: 30, 1: 10}".to_owned());
        cache.put(6, 60);
        assert_eq!(cache.to_str(), "{6: 60, 2: 22, 3: 30}".to_owned());
        cache.get(&3);
        assert_eq!(cache.to_str(), "{3: 30, 6: 60, 2: 22}".to_owned());
        cache.change_capacity(2);
        assert_eq!(cache.to_str(), "{3: 30, 6: 60}".to_owned());
    }

    #[test]
    fn test_clear() {
        let mut cache: LruCache<int, int> = LruCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        cache.clear();
        assert!(cache.get(&1).is_none());
        assert!(cache.get(&2).is_none());
        assert_eq!(cache.to_str(), "{}".to_owned());
    }
}
