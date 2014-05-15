// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



trait vec_monad<A> {
    fn bind<B>(&self, f: |&A| -> Vec<B> ) -> Vec<B> ;
}

impl<A> vec_monad<A> for Vec<A> {
    fn bind<B>(&self, f: |&A| -> Vec<B> ) -> Vec<B> {
        let mut r = Vec::new();
        for elt in self.iter() {
            r.push_all_move(f(elt));
        }
        r
    }
}

trait option_monad<A> {
    fn bind<B>(&self, f: |&A| -> Option<B>) -> Option<B>;
}

impl<A> option_monad<A> for Option<A> {
    fn bind<B>(&self, f: |&A| -> Option<B>) -> Option<B> {
        match *self {
            Some(ref a) => { f(a) }
            None => { None }
        }
    }
}

fn transform(x: Option<int>) -> Option<StrBuf> {
    x.bind(|n| Some(*n + 1) ).bind(|n| Some(n.to_str().to_strbuf()) )
}

pub fn main() {
    assert_eq!(transform(Some(10)), Some("11".to_strbuf()));
    assert_eq!(transform(None), None);
    assert!((vec!("hi".to_strbuf()))
        .bind(|x| vec!(x.clone(), format_strbuf!("{}!", x)) )
        .bind(|x| vec!(x.clone(), format_strbuf!("{}?", x)) ) ==
        vec!("hi".to_strbuf(),
             "hi?".to_strbuf(),
             "hi!".to_strbuf(),
             "hi!?".to_strbuf()));
}
