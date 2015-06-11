// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// The rust-book CSS in string form.

pub static STYLE: &'static str = r#"
@import url("../rust.css");

body {
    max-width:none;
    font: 16px/1.4 'Open Sans', 'Source Serif Pro', Georgia, Times, 'Times New Roman', serif;
    line-height: 1.6;
    color: #333;
}

h1, h2, h3, h4, h5, h6 {
    font-family: 'Open Sans', 'Fira Sans', 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-weight: bold;
    color: #333;
}

@media only screen {
    #toc {
        position: absolute;
        left: 0px;
        top: 0px;
        bottom: 0px;
        width: 300px;
        overflow-y: auto;
        border-right: 1px solid rgba(0, 0, 0, 0.07);
        padding: 10px 10px;
        font-size: 14px;
        box-sizing: border-box;
        -webkit-overflow-scrolling: touch;
        background-color: #fafafa;
        color: #364149;
    }

    #page-wrapper {
        position: absolute;
        overflow-y: auto;
        left: 310px;
        right: 0px;
        top: 0px;
        bottom: 0px;
        box-sizing: border-box;
        background: none repeat scroll 0% 0% #FFF;
        -webkit-overflow-scrolling: touch;
    }
}

@media only print {
    #toc, #nav {
        display: none;
    }
}

@media only screen and (max-width: 1060px) {
    #toc {
        width: 100%;
        margin-right: 0;
        top: 40px;
    }
    #page-wrapper {
        top: 40px;
        left: 15px;
        padding-right: 15px;
    }
    .mobile-hidden {
        display: none;
    }
}

#page {
    margin-left: auto;
    margin-right:auto;
    max-width: 750px;
    padding-bottom: 50px;
}

.chapter {
    list-style: none outside none;
    padding-left: 0px;
    line-height: 30px;
}

.section {
    list-style: none outside none;
    padding-left: 20px;
    line-height: 40px;
}

.section li {
    text-overflow: ellipsis;
    overflow: hidden;
    white-space: nowrap;
}

.chapter li a {
    color: #333;
    padding: 5px 0;
}

.chapter li a.active {
    color: #008cff;
}

.chapter li a:hover {
    color: #008cff;
    text-decoration: none;
}

#toggle-nav {
    height: 20px;
    width:  30px;
    padding: 3px 3px 0 3px;
}

#toggle-nav {
    margin-top: 5px;
    width: 30px;
    height: 30px;
    background-color: #FFF;
    border: 1px solid #666;
    border-radius: 3px 3px 3px 3px;
}

.sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    border: 0;
}

.bar {
    display: block;
    background-color: #000;
    border-radius: 2px;
    width: 100%;
    height: 2px;
    margin: 2px 0 3px;
    padding: 0;
}

pre {
    padding: 16px;
    overflow: auto;
    font-size: 85%;
    line-height: 1.45;
    background-color: #f7f7f7;
    border: 0;
    border-radius: 3px;
}

.nav-previous-next {
    margin-top: 60px;
}

.left {
    float: left;
}

.right {
    float: right;
}
"#;
