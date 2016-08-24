// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hir::def_id::DefId;
use std::cell::RefCell;
use super::DepNode;
use super::thread::{DepGraphThreadData, DepMessage};

pub struct DepTask<'graph> {
    data: &'graph DepGraphThreadData,
    key: Option<DepNode<DefId>>,
}

impl<'graph> DepTask<'graph> {
    pub fn new(data: &'graph DepGraphThreadData, key: DepNode<DefId>)
               -> DepTask<'graph> {
        data.enqueue(DepMessage::PushTask(key.clone()));
        DepTask { data: data, key: Some(key) }
    }
}

impl<'graph> Drop for DepTask<'graph> {
    fn drop(&mut self) {
        self.data.enqueue(DepMessage::PopTask(self.key.take().unwrap()));
    }
}

pub struct IgnoreTask<'graph> {
    data: &'graph DepGraphThreadData
}

impl<'graph> IgnoreTask<'graph> {
    pub fn new(data: &'graph DepGraphThreadData) -> IgnoreTask<'graph> {
        data.enqueue(DepMessage::PushIgnore);
        IgnoreTask { data: data }
    }
}

impl<'graph> Drop for IgnoreTask<'graph> {
    fn drop(&mut self) {
        self.data.enqueue(DepMessage::PopIgnore);
    }
}

pub struct Forbid<'graph> {
    forbidden: &'graph RefCell<Vec<DepNode<DefId>>>
}

impl<'graph> Forbid<'graph> {
    pub fn new(forbidden: &'graph RefCell<Vec<DepNode<DefId>>>, node: DepNode<DefId>) -> Self {
        forbidden.borrow_mut().push(node);
        Forbid { forbidden: forbidden }
    }
}

impl<'graph> Drop for Forbid<'graph> {
    fn drop(&mut self) {
        self.forbidden.borrow_mut().pop();
    }
}
