#!/usr/bin/env python2

# Copyright 2014 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

from __future__ import absolute_import, division, print_function
import argparse
import datetime
import os.path as path


def print_tests(tests):
    print('\n'.join([test_tostr(t) for t in tests]))


def read_tests(f):
    basename, _ = path.splitext(path.basename(f))
    tests = []
    for lineno, line in enumerate(open(f), 1):
        fields = filter(None, map(str.strip, line.split('\t')))
        if not (4 <= len(fields) <= 5) \
           or 'E' not in fields[0] or fields[0][0] == '#':
            continue

        opts, pat, text, sgroups = fields[0:4]
        groups = []  # groups as integer ranges
        if sgroups == 'NOMATCH':
            groups = [None]
        elif ',' in sgroups:
            noparen = map(lambda s: s.strip('()'), sgroups.split(')('))
            for g in noparen:
                s, e = map(str.strip, g.split(','))
                if s == '?' and e == '?':
                    groups.append(None)
                else:
                    groups.append((int(s), int(e)))
        else:
            # This skips tests that should result in an error.
            # There aren't many, so I think we can just capture those
            # manually. Possibly fix this in future.
            continue

        if pat == 'SAME':
            pat = tests[-1][1]
        if '$' in opts:
            pat = pat.decode('string_escape')
            text = text.decode('string_escape')
        if 'i' in opts:
            pat = '(?i)%s' % pat

        name = '%s_%d' % (basename, lineno)
        tests.append((name, pat, text, groups))
    return tests


def test_tostr(t):
    lineno, pat, text, groups = t
    options = map(group_tostr, groups)
    return 'mat!{match_%s, r"%s", r"%s", %s}' \
           % (lineno, pat, '' if text == "NULL" else text, ', '.join(options))


def group_tostr(g):
    if g is None:
        return 'None'
    else:
        return 'Some((%d, %d))' % (g[0], g[1])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Generate match tests from an AT&T POSIX test file.')
    aa = parser.add_argument
    aa('files', nargs='+',
       help='A list of dat AT&T POSIX test files. See src/libregexp/testdata')
    args = parser.parse_args()

    tests = []
    for f in args.files:
        tests += read_tests(f)

    tpl = '''// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-tidy-linelength

// DO NOT EDIT. Automatically generated by 'src/etc/regexp-match-tests'
// on {date}.
'''
    print(tpl.format(date=str(datetime.datetime.now())))

    for f in args.files:
        print('// Tests from %s' % path.basename(f))
        print_tests(read_tests(f))
        print('')
