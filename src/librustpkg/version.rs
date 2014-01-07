// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A version is either an exact revision,
/// or a semantic version

extern mod std;

use extra::semver;
use std::{char, result};

#[deriving(Clone)]
pub enum Version {
    ExactRevision(~str), // Should look like a m.n.(...).x
    SemanticVersion(semver::Version),
    Tagged(~str), // String that can't be parsed as a version.
                  // Requirements get interpreted exactly
    NoVersion // user didn't specify a version -- prints as 0.0
}

// Equality on versions is non-symmetric: if self is NoVersion, it's equal to
// anything; but if self is a precise version, it's not equal to NoVersion.
// We should probably make equality symmetric, and use less-than and greater-than
// where we currently use eq
impl Eq for Version {
    fn eq(&self, other: &Version) -> bool {
        match (self, other) {
            (&ExactRevision(ref s1), &ExactRevision(ref s2)) => *s1 == *s2,
            (&SemanticVersion(ref v1), &SemanticVersion(ref v2)) => *v1 == *v2,
            (&NoVersion, _) => true,
            _ => false
        }
    }
}

impl Ord for Version {
    fn lt(&self, other: &Version) -> bool {
        match (self, other) {
            (&NoVersion, _) => true,
            (&ExactRevision(ref f1), &ExactRevision(ref f2)) => f1 < f2,
            (&SemanticVersion(ref v1), &SemanticVersion(ref v2)) => v1 < v2,
            _ => false // incomparable, really
        }
    }
    fn le(&self, other: &Version) -> bool {
        match (self, other) {
            (&NoVersion, _) => true,
            (&ExactRevision(ref f1), &ExactRevision(ref f2)) => f1 <= f2,
            (&SemanticVersion(ref v1), &SemanticVersion(ref v2)) => v1 <= v2,
            _ => false // incomparable, really
        }
    }
    fn ge(&self, other: &Version) -> bool {
        match (self, other) {
            (&ExactRevision(ref f1), &ExactRevision(ref f2)) => f1 > f2,
            (&SemanticVersion(ref v1), &SemanticVersion(ref v2)) => v1 > v2,
            _ => false // incomparable, really
        }
    }
    fn gt(&self, other: &Version) -> bool {
        match (self, other) {
            (&ExactRevision(ref f1), &ExactRevision(ref f2)) => f1 >= f2,
            (&SemanticVersion(ref v1), &SemanticVersion(ref v2)) => v1 >= v2,
            _ => false // incomparable, really
        }
    }

}

impl ToStr for Version {
    fn to_str(&self) -> ~str {
        match *self {
            ExactRevision(ref n) | Tagged(ref n) => format!("{}", n.to_str()),
            SemanticVersion(ref v) => format!("{}", v.to_str()),
            NoVersion => ~"0.0"
        }
    }
}

pub fn parse_vers(vers: ~str) -> result::Result<semver::Version, ~str> {
    match semver::parse(vers) {
        Some(vers) => result::Ok(vers),
        None => result::Err(~"could not parse version: invalid")
    }
}

// Being lazy since we don't have a regexp library now
#[deriving(Eq)]
enum ParseState {
    Start,
    SawDigit,
    SawDot
}

pub fn try_parsing_version(s: &str) -> Option<Version> {
    let s = s.trim();
    debug!("Attempting to parse: {}", s);
    let mut parse_state = Start;
    for c in s.chars() {
        if char::is_digit(c) {
            parse_state = SawDigit;
        }
        else if c == '.' && parse_state == SawDigit {
            parse_state = SawDot;
        }
        else {
            return None;
        }
    }
    match parse_state {
        SawDigit => Some(ExactRevision(s.to_owned())),
        _        => None
    }
}

/// If s is of the form foo#bar, where bar is a valid version
/// number, return the prefix before the # and the version.
/// Otherwise, return None.
pub fn split_version<'a>(s: &'a str) -> Option<(&'a str, Version)> {
    // Check for extra '#' characters separately
    if s.split('#').len() > 2 {
        return None;
    }
    split_version_general(s, '#')
}

pub fn split_version_general<'a>(s: &'a str, sep: char) -> Option<(&'a str, Version)> {
    match s.rfind(sep) {
        Some(i) => {
            let path = s.slice(0, i);
            // n.b. for now, assuming an exact revision is intended, not a SemVer
            Some((path, ExactRevision(s.slice(i + 1, s.len()).to_owned())))
        }
        None => {
            None
        }
    }
}

#[test]
fn test_parse_version() {
    assert!(try_parsing_version("1.2") == Some(ExactRevision(~"1.2")));
    assert!(try_parsing_version("1.0.17") == Some(ExactRevision(~"1.0.17")));
    assert!(try_parsing_version("you're_a_kitty") == None);
    assert!(try_parsing_version("42..1") == None);
    assert!(try_parsing_version("17") == Some(ExactRevision(~"17")));
    assert!(try_parsing_version(".1.2.3") == None);
    assert!(try_parsing_version("2.3.") == None);
}

#[test]
fn test_split_version() {
    let s = "a/b/c#0.1";
    debug!("== {:?} ==", split_version(s));
    assert!(split_version(s) == Some((s.slice(0, 5), ExactRevision(~"0.1"))));
    assert!(split_version("a/b/c") == None);
    let s = "a#1.2";
    assert!(split_version(s) == Some((s.slice(0, 1), ExactRevision(~"1.2"))));
    assert!(split_version("a#a#3.4") == None);
}
