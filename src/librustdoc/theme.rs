// Copyright 2012-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;

macro_rules! try_something {
    ($e:expr, $out:expr) => ({
        match $e {
            Ok(c) => c,
            Err(e) => {
                eprintln!("rustdoc: got an error: {}", e);
                return $out;
            }
        }
    })
}

#[derive(Debug, Clone, Eq)]
pub struct CssPath {
    pub name: String,
    pub children: HashSet<CssPath>,
}

// This PartialEq implementation IS NOT COMMUTATIVE!!!
//
// The order is very important: the second object must have all first's rules.
// However, the first doesn't require to have all second's rules.
impl PartialEq for CssPath {
    fn eq(&self, other: &CssPath) -> bool {
        if self.name != other.name {
            false
        } else {
            for child in &self.children {
                if !other.children.iter().any(|c| child == c) {
                    return false;
                }
            }
            true
        }
    }
}

impl Hash for CssPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        for x in &self.children {
            x.hash(state);
        }
    }
}

impl CssPath {
    fn new(name: String) -> CssPath {
        CssPath {
            name,
            children: HashSet::new(),
        }
    }
}

/// All variants contain the position they occur.
#[derive(Debug, Clone, Copy)]
enum Events {
    StartLineComment(usize),
    StartComment(usize),
    EndComment(usize),
    InBlock(usize),
    OutBlock(usize),
}

impl Events {
    fn get_pos(&self) -> usize {
        match *self {
            Events::StartLineComment(p) |
            Events::StartComment(p) |
            Events::EndComment(p) |
            Events::InBlock(p) |
            Events::OutBlock(p) => p,
        }
    }

    fn is_comment(&self) -> bool {
        match *self {
            Events::StartLineComment(_) |
            Events::StartComment(_) |
            Events::EndComment(_) => true,
            _ => false,
        }
    }
}

fn previous_is_line_comment(events: &[Events]) -> bool {
    if let Some(&Events::StartLineComment(_)) = events.last() {
        true
    } else {
        false
    }
}

fn is_line_comment(pos: usize, v: &[u8], events: &[Events]) -> bool {
    if let Some(&Events::StartComment(_)) = events.last() {
        return false;
    }
    pos + 1 < v.len() && v[pos + 1] == b'/'
}

fn load_css_events(v: &[u8]) -> Vec<Events> {
    let mut pos = 0;
    let mut events = Vec::with_capacity(100);

    while pos < v.len() - 1 {
        match v[pos] {
            b'/' if pos + 1 < v.len() && v[pos + 1] == b'*' => {
                events.push(Events::StartComment(pos));
                pos += 1;
            }
            b'/' if is_line_comment(pos, v, &events) => {
                events.push(Events::StartLineComment(pos));
                pos += 1;
            }
            b'\n' if previous_is_line_comment(&events) => {
                events.push(Events::EndComment(pos));
            }
            b'*' if pos + 1 < v.len() && v[pos + 1] == b'/' => {
                events.push(Events::EndComment(pos + 2));
                pos += 1;
            }
            b'{' if !previous_is_line_comment(&events) => {
                if let Some(&Events::StartComment(_)) = events.last() {
                    pos += 1;
                    continue
                }
                events.push(Events::InBlock(pos + 1));
            }
            b'}' if !previous_is_line_comment(&events) => {
                if let Some(&Events::StartComment(_)) = events.last() {
                    pos += 1;
                    continue
                }
                events.push(Events::OutBlock(pos + 1));
            }
            _ => {}
        }
        pos += 1;
    }
    events
}

fn get_useful_next(events: &[Events], pos: &mut usize) -> Option<Events> {
    while *pos < events.len() {
        if !events[*pos].is_comment() {
            return Some(events[*pos]);
        }
        *pos += 1;
    }
    None
}

fn get_previous_positions(events: &[Events], mut pos: usize) -> Vec<usize> {
    let mut ret = Vec::with_capacity(3);

    ret.push(events[pos].get_pos());
    if pos > 0 {
        pos -= 1;
    }
    loop {
        if pos < 1 || !events[pos].is_comment() {
            let x = events[pos].get_pos();
            if *ret.last().unwrap() != x {
                ret.push(x);
            } else {
                ret.push(0);
            }
            break
        }
        ret.push(events[pos].get_pos());
        pos -= 1;
    }
    if ret.len() & 1 != 0 && events[pos].is_comment() {
        ret.push(0);
    }
    ret.iter().rev().cloned().collect()
}

fn build_rule(v: &[u8], positions: &[usize]) -> String {
    positions.chunks(2)
             .map(|x| ::std::str::from_utf8(&v[x[0]..x[1]]).unwrap_or(""))
             .collect::<String>()
             .trim()
             .replace("\n", " ")
             .replace("/", "")
             .replace("\t", " ")
             .replace("{", "")
             .replace("}", "")
             .split(" ")
             .filter(|s| s.len() > 0)
             .collect::<Vec<&str>>()
             .join(" ")
}

fn inner(v: &[u8], events: &[Events], pos: &mut usize) -> HashSet<CssPath> {
    let mut paths = Vec::with_capacity(50);

    while *pos < events.len() {
        if let Some(Events::OutBlock(_)) = get_useful_next(events, pos) {
            *pos += 1;
            break
        }
        if let Some(Events::InBlock(_)) = get_useful_next(events, pos) {
            paths.push(CssPath::new(build_rule(v, &get_previous_positions(events, *pos))));
            *pos += 1;
        }
        while let Some(Events::InBlock(_)) = get_useful_next(events, pos) {
            if let Some(ref mut path) = paths.last_mut() {
                for entry in inner(v, events, pos).iter() {
                    path.children.insert(entry.clone());
                }
            }
        }
        if let Some(Events::OutBlock(_)) = get_useful_next(events, pos) {
            *pos += 1;
        }
    }
    paths.iter().cloned().collect()
}

pub fn load_css_paths(v: &[u8]) -> CssPath {
    let events = load_css_events(v);
    let mut pos = 0;

    let mut parent = CssPath::new("parent".to_owned());
    parent.children = inner(v, &events, &mut pos);
    parent
}

pub fn get_differences(against: &CssPath, other: &CssPath, v: &mut Vec<String>) {
    if against.name != other.name {
        return
    } else {
        for child in &against.children {
            let mut found = false;
            let mut found_working = false;
            let mut tmp = Vec::new();

            for other_child in &other.children {
                if child.name == other_child.name {
                    if child != other_child {
                        get_differences(child, other_child, &mut tmp);
                    } else {
                        found_working = true;
                    }
                    found = true;
                    break
                }
            }
            if found == false {
                v.push(format!("  Missing \"{}\" rule", child.name));
            } else if found_working == false {
                v.extend(tmp.iter().cloned());
            }
        }
    }
}

pub fn test_theme_against<P: AsRef<Path>>(f: &P, against: &CssPath) -> (bool, Vec<String>) {
    let mut file = try_something!(File::open(f), (false, Vec::new()));
    let mut data = Vec::with_capacity(1000);

    try_something!(file.read_to_end(&mut data), (false, Vec::new()));
    let paths = load_css_paths(&data);
    let mut ret = Vec::new();
    get_differences(against, &paths, &mut ret);
    (true, ret)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_comments_in_rules() {
        let text = r#"
rule a {}

rule b, c
// a line comment
{}

rule d
// another line comment
e {}

rule f/* a multine

comment*/{}

rule g/* another multine

comment*/h

i {}

rule j/*commeeeeent

you like things like "{}" in there? :)
*/
end {}"#;

        let against = r#"
rule a {}

rule b, c {}

rule d e {}

rule f {}

rule gh i {}

rule j end {}
"#;

        let mut ret = Vec::new();
        get_differences(&load_css_paths(against.as_bytes()),
                        &load_css_paths(text.as_bytes()),
                        &mut ret);
        assert!(ret.is_empty());
    }

    #[test]
    fn test_text() {
        let text = r#"
a
/* sdfs
*/ b
c // sdf
d {}
"#;
        let paths = load_css_paths(text.as_bytes());
        assert!(paths.children.contains(&CssPath::new("a b c d".to_owned())));
    }

    #[test]
    fn test_comparison() {
        let x = r#"
a {
    b {
        c {}
    }
}
"#;

        let y = r#"
a {
    b {}
}
"#;

        let against = load_css_paths(y.as_bytes());
        let other = load_css_paths(x.as_bytes());

        let mut ret = Vec::new();
        get_differences(&against, &other, &mut ret);
        assert!(ret.is_empty());
        get_differences(&other, &against, &mut ret);
        assert_eq!(ret, vec!["  Missing \"c\" rule".to_owned()]);
    }
}
