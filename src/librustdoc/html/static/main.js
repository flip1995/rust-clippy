// From rust:
/* global ALIASES, currentCrate, rootPath */

// Local js definitions:
/* global addClass, getCurrentValue, hasClass */
/* global isHidden onEach, removeClass, updateLocalStorage */

if (!String.prototype.startsWith) {
    String.prototype.startsWith = function(searchString, position) {
        position = position || 0;
        return this.indexOf(searchString, position) === position;
    };
}
if (!String.prototype.endsWith) {
    String.prototype.endsWith = function(suffix, length) {
        var l = length || this.length;
        return this.indexOf(suffix, l - suffix.length) !== -1;
    };
}

if (!DOMTokenList.prototype.add) {
    DOMTokenList.prototype.add = function(className) {
        if (className && !hasClass(this, className)) {
            if (this.className && this.className.length > 0) {
                this.className += " " + className;
            } else {
                this.className = className;
            }
        }
    };
}

if (!DOMTokenList.prototype.remove) {
    DOMTokenList.prototype.remove = function(className) {
        if (className && this.className) {
            this.className = (" " + this.className + " ").replace(" " + className + " ", " ")
                                                         .trim();
        }
    };
}

(function() {
    "use strict";

    // This mapping table should match the discriminants of
    // `rustdoc::html::item_type::ItemType` type in Rust.
    var itemTypes = ["mod",
                     "externcrate",
                     "import",
                     "struct",
                     "enum",
                     "fn",
                     "type",
                     "static",
                     "trait",
                     "impl",
                     "tymethod",
                     "method",
                     "structfield",
                     "variant",
                     "macro",
                     "primitive",
                     "associatedtype",
                     "constant",
                     "associatedconstant",
                     "union",
                     "foreigntype",
                     "keyword",
                     "existential",
                     "attr",
                     "derive"];

    var search_input = document.getElementsByClassName("search-input")[0];

    // On the search screen, so you remain on the last tab you opened.
    //
    // 0 for "In Names"
    // 1 for "In Parameters"
    // 2 for "In Return Types"
    var currentTab = 0;

    var themesWidth = null;

    var titleBeforeSearch = document.title;

    function getPageId() {
        var id = document.location.href.split("#")[1];
        if (id) {
            return id.split("?")[0].split("&")[0];
        }
        return null;
    }

    function showSidebar() {
        var elems = document.getElementsByClassName("sidebar-elems")[0];
        if (elems) {
            addClass(elems, "show-it");
        }
        var sidebar = document.getElementsByClassName("sidebar")[0];
        if (sidebar) {
            addClass(sidebar, "mobile");
            var filler = document.getElementById("sidebar-filler");
            if (!filler) {
                var div = document.createElement("div");
                div.id = "sidebar-filler";
                sidebar.appendChild(div);
            }
        }
        var themePicker = document.getElementsByClassName("theme-picker");
        if (themePicker && themePicker.length > 0) {
            themePicker[0].style.display = "none";
        }
    }

    function hideSidebar() {
        var elems = document.getElementsByClassName("sidebar-elems")[0];
        if (elems) {
            removeClass(elems, "show-it");
        }
        var sidebar = document.getElementsByClassName("sidebar")[0];
        removeClass(sidebar, "mobile");
        var filler = document.getElementById("sidebar-filler");
        if (filler) {
            filler.remove();
        }
        document.getElementsByTagName("body")[0].style.marginTop = "";
        var themePicker = document.getElementsByClassName("theme-picker");
        if (themePicker && themePicker.length > 0) {
            themePicker[0].style.display = null;
        }
    }

    // used for special search precedence
    var TY_PRIMITIVE = itemTypes.indexOf("primitive");
    var TY_KEYWORD = itemTypes.indexOf("keyword");

    onEachLazy(document.getElementsByClassName("js-only"), function(e) {
        removeClass(e, "js-only");
    });

    function getQueryStringParams() {
        var params = {};
        window.location.search.substring(1).split("&").
            map(function(s) {
                var pair = s.split("=");
                params[decodeURIComponent(pair[0])] =
                    typeof pair[1] === "undefined" ? null : decodeURIComponent(pair[1]);
            });
        return params;
    }

    function browserSupportsHistoryApi() {
        return document.location.protocol != "file:" &&
          window.history && typeof window.history.pushState === "function";
    }

    var main = document.getElementById("main");

    function highlightSourceLines(ev) {
        // If we're in mobile mode, we should add the sidebar in any case.
        hideSidebar();
        var elem;
        var search = document.getElementById("search");
        var i, from, to, match = window.location.hash.match(/^#?(\d+)(?:-(\d+))?$/);
        if (match) {
            from = parseInt(match[1], 10);
            to = Math.min(50000, parseInt(match[2] || match[1], 10));
            from = Math.min(from, to);
            elem = document.getElementById(from);
            if (!elem) {
                return;
            }
            if (ev === null) {
                var x = document.getElementById(from);
                if (x) {
                    x.scrollIntoView();
                }
            }
            onEachLazy(document.getElementsByClassName("line-numbers"), function(e) {
                onEachLazy(e.getElementsByTagName("span"), function(i_e) {
                    removeClass(i_e, "line-highlighted");
                });
            });
            for (i = from; i <= to; ++i) {
                addClass(document.getElementById(i), "line-highlighted");
            }
        } else if (ev !== null && search && !hasClass(search, "hidden") && ev.newURL) {
            addClass(search, "hidden");
            removeClass(main, "hidden");
            var hash = ev.newURL.slice(ev.newURL.indexOf("#") + 1);
            if (browserSupportsHistoryApi()) {
                history.replaceState(hash, "", "?search=#" + hash);
            }
            elem = document.getElementById(hash);
            if (elem) {
                elem.scrollIntoView();
            }
        }
    }

    function expandSection(id) {
        var elem = document.getElementById(id);
        if (elem && isHidden(elem)) {
            var h3 = elem.parentNode.previousElementSibling;
            if (h3 && h3.tagName !== "H3") {
                h3 = h3.previousElementSibling; // skip div.docblock
            }

            if (h3) {
                var collapses = h3.getElementsByClassName("collapse-toggle");
                if (collapses.length > 0) {
                    // The element is not visible, we need to make it appear!
                    collapseDocs(collapses[0], "show");
                }
            }
        }
    }

    highlightSourceLines(null);
    window.onhashchange = highlightSourceLines;

    // Gets the human-readable string for the virtual-key code of the
    // given KeyboardEvent, ev.
    //
    // This function is meant as a polyfill for KeyboardEvent#key,
    // since it is not supported in Trident.  We also test for
    // KeyboardEvent#keyCode because the handleShortcut handler is
    // also registered for the keydown event, because Blink doesn't fire
    // keypress on hitting the Escape key.
    //
    // So I guess you could say things are getting pretty interoperable.
    function getVirtualKey(ev) {
        if ("key" in ev && typeof ev.key != "undefined") {
            return ev.key;
        }

        var c = ev.charCode || ev.keyCode;
        if (c == 27) {
            return "Escape";
        }
        return String.fromCharCode(c);
    }

    function displayHelp(display, ev) {
        if (display === true) {
            if (hasClass(help, "hidden")) {
                ev.preventDefault();
                removeClass(help, "hidden");
                addClass(document.body, "blur");
            }
        } else if (hasClass(help, "hidden") === false) {
            ev.preventDefault();
            addClass(help, "hidden");
            removeClass(document.body, "blur");
        }
    }

    function handleEscape(ev, help) {
        hideModal();
        var search = document.getElementById("search");
        if (hasClass(help, "hidden") === false) {
            displayHelp(false, ev);
        } else if (hasClass(search, "hidden") === false) {
            ev.preventDefault();
            addClass(search, "hidden");
            removeClass(main, "hidden");
            document.title = titleBeforeSearch;
        }
        defocusSearchBar();
    }

    function handleShortcut(ev) {
        // Don't interfere with browser shortcuts
        if (ev.ctrlKey || ev.altKey || ev.metaKey) {
            return;
        }

        var help = document.getElementById("help");
        if (document.activeElement.tagName === "INPUT") {
            switch (getVirtualKey(ev)) {
            case "Escape":
                handleEscape(ev, help);
                break;
            }
        } else {
            switch (getVirtualKey(ev)) {
            case "Escape":
                handleEscape(ev, help);
                break;

            case "s":
            case "S":
                displayHelp(false, ev);
                hideModal();
                ev.preventDefault();
                focusSearchBar();
                break;

            case "+":
            case "-":
                ev.preventDefault();
                toggleAllDocs();
                break;

            case "?":
                if (ev.shiftKey) {
                    hideModal();
                    displayHelp(true, ev);
                }
                break;
            }
        }
    }

    function findParentElement(elem, tagName) {
        do {
            if (elem && elem.tagName === tagName) {
                return elem;
            }
            elem = elem.parentNode;
        } while (elem);
        return null;
    }

    document.onkeypress = handleShortcut;
    document.onkeydown = handleShortcut;
    document.onclick = function(ev) {
        if (hasClass(ev.target, "collapse-toggle")) {
            collapseDocs(ev.target, "toggle");
        } else if (hasClass(ev.target.parentNode, "collapse-toggle")) {
            collapseDocs(ev.target.parentNode, "toggle");
        } else if (ev.target.tagName === "SPAN" && hasClass(ev.target.parentNode, "line-numbers")) {
            var prev_id = 0;

            var set_fragment = function(name) {
                if (browserSupportsHistoryApi()) {
                    history.replaceState(null, null, "#" + name);
                    window.hashchange();
                } else {
                    location.replace("#" + name);
                }
            };

            var cur_id = parseInt(ev.target.id, 10);

            if (ev.shiftKey && prev_id) {
                if (prev_id > cur_id) {
                    var tmp = prev_id;
                    prev_id = cur_id;
                    cur_id = tmp;
                }

                set_fragment(prev_id + "-" + cur_id);
            } else {
                prev_id = cur_id;

                set_fragment(cur_id);
            }
        } else if (hasClass(document.getElementById("help"), "hidden") === false) {
            addClass(document.getElementById("help"), "hidden");
            removeClass(document.body, "blur");
        } else {
            // Making a collapsed element visible on onhashchange seems
            // too late
            var a = findParentElement(ev.target, "A");
            if (a && a.hash) {
                expandSection(a.hash.replace(/^#/, ""));
            }
        }
    };

    var x = document.getElementsByClassName("version-selector");
    if (x.length > 0) {
        x[0].onchange = function() {
            var i, match,
                url = document.location.href,
                stripped = "",
                len = rootPath.match(/\.\.\//g).length + 1;

            for (i = 0; i < len; ++i) {
                match = url.match(/\/[^\/]*$/);
                if (i < len - 1) {
                    stripped = match[0] + stripped;
                }
                url = url.substring(0, url.length - match[0].length);
            }

            url += "/" + document.getElementsByClassName("version-selector")[0].value + stripped;

            document.location.href = url;
        };
    }

    /**
     * A function to compute the Levenshtein distance between two strings
     * Licensed under the Creative Commons Attribution-ShareAlike 3.0 Unported
     * Full License can be found at http://creativecommons.org/licenses/by-sa/3.0/legalcode
     * This code is an unmodified version of the code written by Marco de Wit
     * and was found at http://stackoverflow.com/a/18514751/745719
     */
    var levenshtein_row2 = [];
    function levenshtein(s1, s2) {
        if (s1 === s2) {
            return 0;
        }
        var s1_len = s1.length, s2_len = s2.length;
        if (s1_len && s2_len) {
            var i1 = 0, i2 = 0, a, b, c, c2, row = levenshtein_row2;
            while (i1 < s1_len) {
                row[i1] = ++i1;
            }
            while (i2 < s2_len) {
                c2 = s2.charCodeAt(i2);
                a = i2;
                ++i2;
                b = i2;
                for (i1 = 0; i1 < s1_len; ++i1) {
                    c = a + (s1.charCodeAt(i1) !== c2 ? 1 : 0);
                    a = row[i1];
                    b = b < a ? (b < c ? b + 1 : c) : (a < c ? a + 1 : c);
                    row[i1] = b;
                }
            }
            return b;
        }
        return s1_len + s2_len;
    }

    function initSearch(rawSearchIndex) {
        var currentResults, index, searchIndex;
        var MAX_LEV_DISTANCE = 3;
        var MAX_RESULTS = 200;
        var GENERICS_DATA = 1;
        var NAME = 0;
        var INPUTS_DATA = 0;
        var OUTPUT_DATA = 1;
        var params = getQueryStringParams();

        // Populate search bar with query string search term when provided,
        // but only if the input bar is empty. This avoid the obnoxious issue
        // where you start trying to do a search, and the index loads, and
        // suddenly your search is gone!
        if (search_input.value === "") {
            search_input.value = params.search || "";
        }

        /**
         * Executes the query and builds an index of results
         * @param  {[Object]} query      [The user query]
         * @param  {[type]} searchWords  [The list of search words to query
         *                                against]
         * @param  {[type]} filterCrates [Crate to search in if defined]
         * @return {[type]}              [A search index of results]
         */
        function execQuery(query, searchWords, filterCrates) {
            function itemTypeFromName(typename) {
                var length = itemTypes.length;
                for (var i = 0; i < length; ++i) {
                    if (itemTypes[i] === typename) {
                        return i;
                    }
                }
                return -1;
            }

            var valLower = query.query.toLowerCase(),
                val = valLower,
                typeFilter = itemTypeFromName(query.type),
                results = {}, results_in_args = {}, results_returned = {},
                split = valLower.split("::");

            var length = split.length;
            for (var z = 0; z < length; ++z) {
                if (split[z] === "") {
                    split.splice(z, 1);
                    z -= 1;
                }
            }

            function transformResults(results, isType) {
                var out = [];
                var length = results.length;
                for (var i = 0; i < length; ++i) {
                    if (results[i].id > -1) {
                        var obj = searchIndex[results[i].id];
                        obj.lev = results[i].lev;
                        if (isType !== true || obj.type) {
                            var res = buildHrefAndPath(obj);
                            obj.displayPath = pathSplitter(res[0]);
                            obj.fullPath = obj.displayPath + obj.name;
                            // To be sure than it some items aren't considered as duplicate.
                            obj.fullPath += "|" + obj.ty;
                            obj.href = res[1];
                            out.push(obj);
                            if (out.length >= MAX_RESULTS) {
                                break;
                            }
                        }
                    }
                }
                return out;
            }

            function sortResults(results, isType) {
                var ar = [];
                for (var entry in results) {
                    if (results.hasOwnProperty(entry)) {
                        ar.push(results[entry]);
                    }
                }
                results = ar;
                var i;
                var nresults = results.length;
                for (i = 0; i < nresults; ++i) {
                    results[i].word = searchWords[results[i].id];
                    results[i].item = searchIndex[results[i].id] || {};
                }
                // if there are no results then return to default and fail
                if (results.length === 0) {
                    return [];
                }

                results.sort(function(aaa, bbb) {
                    var a, b;

                    // Sort by non levenshtein results and then levenshtein results by the distance
                    // (less changes required to match means higher rankings)
                    a = (aaa.lev);
                    b = (bbb.lev);
                    if (a !== b) { return a - b; }

                    // sort by crate (non-current crate goes later)
                    a = (aaa.item.crate !== window.currentCrate);
                    b = (bbb.item.crate !== window.currentCrate);
                    if (a !== b) { return a - b; }

                    // sort by exact match (mismatch goes later)
                    a = (aaa.word !== valLower);
                    b = (bbb.word !== valLower);
                    if (a !== b) { return a - b; }

                    // sort by item name length (longer goes later)
                    a = aaa.word.length;
                    b = bbb.word.length;
                    if (a !== b) { return a - b; }

                    // sort by item name (lexicographically larger goes later)
                    a = aaa.word;
                    b = bbb.word;
                    if (a !== b) { return (a > b ? +1 : -1); }

                    // sort by index of keyword in item name (no literal occurrence goes later)
                    a = (aaa.index < 0);
                    b = (bbb.index < 0);
                    if (a !== b) { return a - b; }
                    // (later literal occurrence, if any, goes later)
                    a = aaa.index;
                    b = bbb.index;
                    if (a !== b) { return a - b; }

                    // special precedence for primitive and keyword pages
                    if ((aaa.item.ty === TY_PRIMITIVE && bbb.item.ty !== TY_KEYWORD) ||
                        (aaa.item.ty === TY_KEYWORD && bbb.item.ty !== TY_PRIMITIVE)) {
                        return -1;
                    }
                    if ((bbb.item.ty === TY_PRIMITIVE && aaa.item.ty !== TY_PRIMITIVE) ||
                        (bbb.item.ty === TY_KEYWORD && aaa.item.ty !== TY_KEYWORD)) {
                        return 1;
                    }

                    // sort by description (no description goes later)
                    a = (aaa.item.desc === "");
                    b = (bbb.item.desc === "");
                    if (a !== b) { return a - b; }

                    // sort by type (later occurrence in `itemTypes` goes later)
                    a = aaa.item.ty;
                    b = bbb.item.ty;
                    if (a !== b) { return a - b; }

                    // sort by path (lexicographically larger goes later)
                    a = aaa.item.path;
                    b = bbb.item.path;
                    if (a !== b) { return (a > b ? +1 : -1); }

                    // que sera, sera
                    return 0;
                });

                var length = results.length;
                for (i = 0; i < length; ++i) {
                    var result = results[i];

                    // this validation does not make sense when searching by types
                    if (result.dontValidate) {
                        continue;
                    }
                    var name = result.item.name.toLowerCase(),
                        path = result.item.path.toLowerCase(),
                        parent = result.item.parent;

                    if (isType !== true &&
                        validateResult(name, path, split, parent) === false)
                    {
                        result.id = -1;
                    }
                }
                return transformResults(results);
            }

            function extractGenerics(val) {
                val = val.toLowerCase();
                if (val.indexOf("<") !== -1) {
                    var values = val.substring(val.indexOf("<") + 1, val.lastIndexOf(">"));
                    return {
                        name: val.substring(0, val.indexOf("<")),
                        generics: values.split(/\s*,\s*/),
                    };
                }
                return {
                    name: val,
                    generics: [],
                };
            }

            function checkGenerics(obj, val) {
                // The names match, but we need to be sure that all generics kinda
                // match as well.
                var lev_distance = MAX_LEV_DISTANCE + 1;
                if (val.generics.length > 0) {
                    if (obj.length > GENERICS_DATA &&
                          obj[GENERICS_DATA].length >= val.generics.length) {
                        var elems = obj[GENERICS_DATA].slice(0);
                        var total = 0;
                        var done = 0;
                        // We need to find the type that matches the most to remove it in order
                        // to move forward.
                        var vlength = val.generics.length;
                        for (var y = 0; y < vlength; ++y) {
                            var lev = { pos: -1, lev: MAX_LEV_DISTANCE + 1};
                            var elength = elems.length;
                            for (var x = 0; x < elength; ++x) {
                                var tmp_lev = levenshtein(elems[x], val.generics[y]);
                                if (tmp_lev < lev.lev) {
                                    lev.lev = tmp_lev;
                                    lev.pos = x;
                                }
                            }
                            if (lev.pos !== -1) {
                                elems.splice(lev.pos, 1);
                                lev_distance = Math.min(lev.lev, lev_distance);
                                total += lev.lev;
                                done += 1;
                            } else {
                                return MAX_LEV_DISTANCE + 1;
                            }
                        }
                        return lev_distance;//Math.ceil(total / done);
                    }
                }
                return MAX_LEV_DISTANCE + 1;
            }

            // Check for type name and type generics (if any).
            function checkType(obj, val, literalSearch) {
                var lev_distance = MAX_LEV_DISTANCE + 1;
                var x;
                if (obj[NAME] === val.name) {
                    if (literalSearch === true) {
                        if (val.generics && val.generics.length !== 0) {
                            if (obj.length > GENERICS_DATA &&
                                  obj[GENERICS_DATA].length >= val.generics.length) {
                                var elems = obj[GENERICS_DATA].slice(0);
                                var allFound = true;

                                for (var y = 0; allFound === true && y < val.generics.length; ++y) {
                                    allFound = false;
                                    for (x = 0; allFound === false && x < elems.length; ++x) {
                                        allFound = elems[x] === val.generics[y];
                                    }
                                    if (allFound === true) {
                                        elems.splice(x - 1, 1);
                                    }
                                }
                                if (allFound === true) {
                                    return true;
                                }
                            } else {
                                return false;
                            }
                        }
                        return true;
                    }
                    // If the type has generics but don't match, then it won't return at this point.
                    // Otherwise, `checkGenerics` will return 0 and it'll return.
                    if (obj.length > GENERICS_DATA && obj[GENERICS_DATA].length !== 0) {
                        var tmp_lev = checkGenerics(obj, val);
                        if (tmp_lev <= MAX_LEV_DISTANCE) {
                            return tmp_lev;
                        }
                    } else {
                        return 0;
                    }
                }
                // Names didn't match so let's check if one of the generic types could.
                if (literalSearch === true) {
                     if (obj.length > GENERICS_DATA && obj[GENERICS_DATA].length > 0) {
                        var length = obj[GENERICS_DATA].length;
                        for (x = 0; x < length; ++x) {
                            if (obj[GENERICS_DATA][x] === val.name) {
                                return true;
                            }
                        }
                    }
                    return false;
                }
                lev_distance = Math.min(levenshtein(obj[NAME], val.name), lev_distance);
                if (lev_distance <= MAX_LEV_DISTANCE) {
                    lev_distance = Math.min(checkGenerics(obj, val), lev_distance);
                } else if (obj.length > GENERICS_DATA && obj[GENERICS_DATA].length > 0) {
                    // We can check if the type we're looking for is inside the generics!
                    var olength = obj[GENERICS_DATA].length;
                    for (x = 0; x < olength; ++x) {
                        lev_distance = Math.min(levenshtein(obj[GENERICS_DATA][x], val.name),
                                                lev_distance);
                    }
                }
                // Now whatever happens, the returned distance is "less good" so we should mark it
                // as such, and so we add 1 to the distance to make it "less good".
                return lev_distance + 1;
            }

            function findArg(obj, val, literalSearch) {
                var lev_distance = MAX_LEV_DISTANCE + 1;

                if (obj && obj.type && obj.type[INPUTS_DATA] &&
                      obj.type[INPUTS_DATA].length > 0) {
                    var length = obj.type[INPUTS_DATA].length;
                    for (var i = 0; i < length; i++) {
                        var tmp = checkType(obj.type[INPUTS_DATA][i], val, literalSearch);
                        if (literalSearch === true && tmp === true) {
                            return true;
                        }
                        lev_distance = Math.min(tmp, lev_distance);
                        if (lev_distance === 0) {
                            return 0;
                        }
                    }
                }
                return literalSearch === true ? false : lev_distance;
            }

            function checkReturned(obj, val, literalSearch) {
                var lev_distance = MAX_LEV_DISTANCE + 1;

                if (obj && obj.type && obj.type.length > OUTPUT_DATA) {
                    var tmp = checkType(obj.type[OUTPUT_DATA], val, literalSearch);
                    if (literalSearch === true && tmp === true) {
                        return true;
                    }
                    lev_distance = Math.min(tmp, lev_distance);
                    if (lev_distance === 0) {
                        return 0;
                    }
                }
                return literalSearch === true ? false : lev_distance;
            }

            function checkPath(contains, lastElem, ty) {
                if (contains.length === 0) {
                    return 0;
                }
                var ret_lev = MAX_LEV_DISTANCE + 1;
                var path = ty.path.split("::");

                if (ty.parent && ty.parent.name) {
                    path.push(ty.parent.name.toLowerCase());
                }

                var length = path.length;
                var clength = contains.length;
                if (clength > length) {
                    return MAX_LEV_DISTANCE + 1;
                }
                for (var i = 0; i < length; ++i) {
                    if (i + clength > length) {
                        break;
                    }
                    var lev_total = 0;
                    var aborted = false;
                    for (var x = 0; x < clength; ++x) {
                        var lev = levenshtein(path[i + x], contains[x]);
                        if (lev > MAX_LEV_DISTANCE) {
                            aborted = true;
                            break;
                        }
                        lev_total += lev;
                    }
                    if (aborted === false) {
                        ret_lev = Math.min(ret_lev, Math.round(lev_total / clength));
                    }
                }
                return ret_lev;
            }

            function typePassesFilter(filter, type) {
                // No filter
                if (filter < 0) return true;

                // Exact match
                if (filter === type) return true;

                // Match related items
                var name = itemTypes[type];
                switch (itemTypes[filter]) {
                    case "constant":
                        return (name == "associatedconstant");
                    case "fn":
                        return (name == "method" || name == "tymethod");
                    case "type":
                        return (name == "primitive" || name == "keyword");
                }

                // No match
                return false;
            }

            function generateId(ty) {
                if (ty.parent && ty.parent.name) {
                    return itemTypes[ty.ty] + ty.path + ty.parent.name + ty.name;
                }
                return itemTypes[ty.ty] + ty.path + ty.name;
            }

            // quoted values mean literal search
            var nSearchWords = searchWords.length;
            var i;
            var ty;
            var fullId;
            var returned;
            var in_args;
            if ((val.charAt(0) === "\"" || val.charAt(0) === "'") &&
                val.charAt(val.length - 1) === val.charAt(0))
            {
                val = extractGenerics(val.substr(1, val.length - 2));
                for (i = 0; i < nSearchWords; ++i) {
                    if (filterCrates !== undefined && searchIndex[i].crate !== filterCrates) {
                        continue;
                    }
                    in_args = findArg(searchIndex[i], val, true);
                    returned = checkReturned(searchIndex[i], val, true);
                    ty = searchIndex[i];
                    fullId = generateId(ty);

                    if (searchWords[i] === val.name) {
                        // filter type: ... queries
                        if (typePassesFilter(typeFilter, searchIndex[i].ty) &&
                            results[fullId] === undefined)
                        {
                            results[fullId] = {id: i, index: -1};
                        }
                    } else if ((in_args === true || returned === true) &&
                               typePassesFilter(typeFilter, searchIndex[i].ty)) {
                        if (in_args === true || returned === true) {
                            if (in_args === true) {
                                results_in_args[fullId] = {
                                    id: i,
                                    index: -1,
                                    dontValidate: true,
                                };
                            }
                            if (returned === true) {
                                results_returned[fullId] = {
                                    id: i,
                                    index: -1,
                                    dontValidate: true,
                                };
                            }
                        } else {
                            results[fullId] = {
                                id: i,
                                index: -1,
                                dontValidate: true,
                            };
                        }
                    }
                }
                query.inputs = [val];
                query.output = val;
                query.search = val;
            // searching by type
            } else if (val.search("->") > -1) {
                var trimmer = function(s) { return s.trim(); };
                var parts = val.split("->").map(trimmer);
                var input = parts[0];
                // sort inputs so that order does not matter
                var inputs = input.split(",").map(trimmer).sort();
                for (i = 0; i < inputs.length; ++i) {
                    inputs[i] = extractGenerics(inputs[i]);
                }
                var output = extractGenerics(parts[1]);

                for (i = 0; i < nSearchWords; ++i) {
                    if (filterCrates !== undefined && searchIndex[i].crate !== filterCrates) {
                        continue;
                    }
                    var type = searchIndex[i].type;
                    ty = searchIndex[i];
                    if (!type) {
                        continue;
                    }
                    fullId = generateId(ty);

                    // allow searching for void (no output) functions as well
                    var typeOutput = type.length > OUTPUT_DATA ? type[OUTPUT_DATA].name : "";
                    returned = checkReturned(ty, output, true);
                    if (output.name === "*" || returned === true) {
                        in_args = false;
                        var module = false;

                        if (input === "*") {
                            module = true;
                        } else {
                            var allFound = true;
                            for (var it = 0; allFound === true && it < inputs.length; it++) {
                                allFound = checkType(type, inputs[it], true);
                            }
                            in_args = allFound;
                        }
                        if (in_args === true) {
                            results_in_args[fullId] = {
                                id: i,
                                index: -1,
                                dontValidate: true,
                            };
                        }
                        if (returned === true) {
                            results_returned[fullId] = {
                                id: i,
                                index: -1,
                                dontValidate: true,
                            };
                        }
                        if (module === true) {
                            results[fullId] = {
                                id: i,
                                index: -1,
                                dontValidate: true,
                            };
                        }
                    }
                }
                query.inputs = inputs.map(function(input) {
                    return input.name;
                });
                query.output = output.name;
            } else {
                query.inputs = [val];
                query.output = val;
                query.search = val;
                // gather matching search results up to a certain maximum
                val = val.replace(/\_/g, "");

                var valGenerics = extractGenerics(val);

                var paths = valLower.split("::");
                var j;
                for (j = 0; j < paths.length; ++j) {
                    if (paths[j] === "") {
                        paths.splice(j, 1);
                        j -= 1;
                    }
                }
                val = paths[paths.length - 1];
                var contains = paths.slice(0, paths.length > 1 ? paths.length - 1 : 1);

                for (j = 0; j < nSearchWords; ++j) {
                    var lev;
                    var lev_distance;
                    ty = searchIndex[j];
                    if (!ty || (filterCrates !== undefined && ty.crate !== filterCrates)) {
                        continue;
                    }
                    var lev_distance;
                    var lev_add = 0;
                    if (paths.length > 1) {
                        lev = checkPath(contains, paths[paths.length - 1], ty);
                        if (lev > MAX_LEV_DISTANCE) {
                            continue;
                        } else if (lev > 0) {
                            lev_add = 1;
                        }
                    }

                    returned = MAX_LEV_DISTANCE + 1;
                    in_args = MAX_LEV_DISTANCE + 1;
                    var index = -1;
                    // we want lev results to go lower than others
                    lev = MAX_LEV_DISTANCE + 1;
                    fullId = generateId(ty);

                    if (searchWords[j].indexOf(split[i]) > -1 ||
                        searchWords[j].indexOf(val) > -1 ||
                        searchWords[j].replace(/_/g, "").indexOf(val) > -1)
                    {
                        // filter type: ... queries
                        if (typePassesFilter(typeFilter, ty.ty) && results[fullId] === undefined) {
                            index = searchWords[j].replace(/_/g, "").indexOf(val);
                        }
                    }
                    if ((lev = levenshtein(searchWords[j], val)) <= MAX_LEV_DISTANCE) {
                        if (typePassesFilter(typeFilter, ty.ty) === false) {
                            lev = MAX_LEV_DISTANCE + 1;
                        } else {
                            lev += 1;
                        }
                    }
                    if ((in_args = findArg(ty, valGenerics)) <= MAX_LEV_DISTANCE) {
                        if (typePassesFilter(typeFilter, ty.ty) === false) {
                            in_args = MAX_LEV_DISTANCE + 1;
                        }
                    }
                    if ((returned = checkReturned(ty, valGenerics)) <= MAX_LEV_DISTANCE) {
                        if (typePassesFilter(typeFilter, ty.ty) === false) {
                            returned = MAX_LEV_DISTANCE + 1;
                        }
                    }

                    lev += lev_add;
                    if (lev > 0 && val.length > 3 && searchWords[j].indexOf(val) > -1) {
                        if (val.length < 6) {
                            lev -= 1;
                        } else {
                            lev = 0;
                        }
                    }
                    if (in_args <= MAX_LEV_DISTANCE) {
                        if (results_in_args[fullId] === undefined) {
                            results_in_args[fullId] = {
                                id: j,
                                index: index,
                                lev: in_args,
                            };
                        }
                        results_in_args[fullId].lev =
                            Math.min(results_in_args[fullId].lev, in_args);
                    }
                    if (returned <= MAX_LEV_DISTANCE) {
                        if (results_returned[fullId] === undefined) {
                            results_returned[fullId] = {
                                id: j,
                                index: index,
                                lev: returned,
                            };
                        }
                        results_returned[fullId].lev =
                            Math.min(results_returned[fullId].lev, returned);
                    }
                    if (index !== -1 || lev <= MAX_LEV_DISTANCE) {
                        if (index !== -1 && paths.length < 2) {
                            lev = 0;
                        }
                        if (results[fullId] === undefined) {
                            results[fullId] = {
                                id: j,
                                index: index,
                                lev: lev,
                            };
                        }
                        results[fullId].lev = Math.min(results[fullId].lev, lev);
                    }
                }
            }

            var ret = {
                "in_args": sortResults(results_in_args, true),
                "returned": sortResults(results_returned, true),
                "others": sortResults(results),
            };
            if (ALIASES && ALIASES[window.currentCrate] &&
                    ALIASES[window.currentCrate][query.raw]) {
                var aliases = ALIASES[window.currentCrate][query.raw];
                for (i = 0; i < aliases.length; ++i) {
                    aliases[i].is_alias = true;
                    aliases[i].alias = query.raw;
                    aliases[i].path = aliases[i].p;
                    var res = buildHrefAndPath(aliases[i]);
                    aliases[i].displayPath = pathSplitter(res[0]);
                    aliases[i].fullPath = aliases[i].displayPath + aliases[i].name;
                    aliases[i].href = res[1];
                    ret.others.unshift(aliases[i]);
                    if (ret.others.length > MAX_RESULTS) {
                        ret.others.pop();
                    }
                }
            }
            return ret;
        }

        /**
         * Validate performs the following boolean logic. For example:
         * "File::open" will give IF A PARENT EXISTS => ("file" && "open")
         * exists in (name || path || parent) OR => ("file" && "open") exists in
         * (name || path )
         *
         * This could be written functionally, but I wanted to minimise
         * functions on stack.
         *
         * @param  {[string]} name   [The name of the result]
         * @param  {[string]} path   [The path of the result]
         * @param  {[string]} keys   [The keys to be used (["file", "open"])]
         * @param  {[object]} parent [The parent of the result]
         * @return {[boolean]}       [Whether the result is valid or not]
         */
        function validateResult(name, path, keys, parent) {
            for (var i = 0; i < keys.length; ++i) {
                // each check is for validation so we negate the conditions and invalidate
                if (!(
                    // check for an exact name match
                    name.indexOf(keys[i]) > -1 ||
                    // then an exact path match
                    path.indexOf(keys[i]) > -1 ||
                    // next if there is a parent, check for exact parent match
                    (parent !== undefined &&
                        parent.name.toLowerCase().indexOf(keys[i]) > -1) ||
                    // lastly check to see if the name was a levenshtein match
                    levenshtein(name, keys[i]) <= MAX_LEV_DISTANCE)) {
                    return false;
                }
            }
            return true;
        }

        function getQuery(raw) {
            var matches, type, query;
            query = raw;

            matches = query.match(/^(fn|mod|struct|enum|trait|type|const|macro)\s*:\s*/i);
            if (matches) {
                type = matches[1].replace(/^const$/, "constant");
                query = query.substring(matches[0].length);
            }

            return {
                raw: raw,
                query: query,
                type: type,
                id: query + type
            };
        }

        function initSearchNav() {
            var hoverTimeout;

            var click_func = function(e) {
                var el = e.target;
                // to retrieve the real "owner" of the event.
                while (el.tagName !== "TR") {
                    el = el.parentNode;
                }
                var dst = e.target.getElementsByTagName("a");
                if (dst.length < 1) {
                    return;
                }
                dst = dst[0];
                if (window.location.pathname === dst.pathname) {
                    addClass(document.getElementById("search"), "hidden");
                    removeClass(main, "hidden");
                    document.location.href = dst.href;
                }
            };
            var mouseover_func = function(e) {
                var el = e.target;
                // to retrieve the real "owner" of the event.
                while (el.tagName !== "TR") {
                    el = el.parentNode;
                }
                clearTimeout(hoverTimeout);
                hoverTimeout = setTimeout(function() {
                    onEachLazy(document.getElementsByClassName("search-results"), function(e) {
                        onEachLazy(e.getElementsByClassName("result"), function(i_e) {
                            removeClass(i_e, "highlighted");
                        });
                    });
                    addClass(el, "highlighted");
                }, 20);
            };
            onEachLazy(document.getElementsByClassName("search-results"), function(e) {
                onEachLazy(e.getElementsByClassName("result"), function(i_e) {
                    i_e.onclick = click_func;
                    i_e.onmouseover = mouseover_func;
                });
            });

            search_input.onkeydown = function(e) {
                // "actives" references the currently highlighted item in each search tab.
                // Each array in "actives" represents a tab.
                var actives = [[], [], []];
                // "current" is used to know which tab we're looking into.
                var current = 0;
                onEachLazy(document.getElementsByClassName("search-results"), function(e) {
                    onEachLazy(e.getElementsByClassName("highlighted"), function(e) {
                        actives[current].push(e);
                    });
                    current += 1;
                });

                if (e.which === 38) { // up
                    if (!actives[currentTab].length ||
                        !actives[currentTab][0].previousElementSibling) {
                        return;
                    }

                    addClass(actives[currentTab][0].previousElementSibling, "highlighted");
                    removeClass(actives[currentTab][0], "highlighted");
                } else if (e.which === 40) { // down
                    if (!actives[currentTab].length) {
                        var results = document.getElementsByClassName("search-results");
                        if (results.length > 0) {
                            var res = results[currentTab].getElementsByClassName("result");
                            if (res.length > 0) {
                                addClass(res[0], "highlighted");
                            }
                        }
                    } else if (actives[currentTab][0].nextElementSibling) {
                        addClass(actives[currentTab][0].nextElementSibling, "highlighted");
                        removeClass(actives[currentTab][0], "highlighted");
                    }
                } else if (e.which === 13) { // return
                    if (actives[currentTab].length) {
                        document.location.href =
                            actives[currentTab][0].getElementsByTagName("a")[0].href;
                    }
                } else if (e.which === 9) { // tab
                    if (e.shiftKey) {
                        printTab(currentTab > 0 ? currentTab - 1 : 2);
                    } else {
                        printTab(currentTab > 1 ? 0 : currentTab + 1);
                    }
                    e.preventDefault();
                } else if (e.which === 16) { // shift
                    // Does nothing, it's just to avoid losing "focus" on the highlighted element.
                } else if (e.which === 27) { // escape
                    removeClass(actives[currentTab][0], "highlighted");
                    search_input.value = "";
                    defocusSearchBar();
                } else if (actives[currentTab].length > 0) {
                    removeClass(actives[currentTab][0], "highlighted");
                }
            };
        }

        function buildHrefAndPath(item) {
            var displayPath;
            var href;
            var type = itemTypes[item.ty];
            var name = item.name;

            if (type === "mod") {
                displayPath = item.path + "::";
                href = rootPath + item.path.replace(/::/g, "/") + "/" +
                       name + "/index.html";
            } else if (type === "primitive" || type === "keyword") {
                displayPath = "";
                href = rootPath + item.path.replace(/::/g, "/") +
                       "/" + type + "." + name + ".html";
            } else if (type === "externcrate") {
                displayPath = "";
                href = rootPath + name + "/index.html";
            } else if (item.parent !== undefined) {
                var myparent = item.parent;
                var anchor = "#" + type + "." + name;
                var parentType = itemTypes[myparent.ty];
                if (parentType === "primitive") {
                    displayPath = myparent.name + "::";
                } else {
                    displayPath = item.path + "::" + myparent.name + "::";
                }
                href = rootPath + item.path.replace(/::/g, "/") +
                       "/" + parentType +
                       "." + myparent.name +
                       ".html" + anchor;
            } else {
                displayPath = item.path + "::";
                href = rootPath + item.path.replace(/::/g, "/") +
                       "/" + type + "." + name + ".html";
            }
            return [displayPath, href];
        }

        function escape(content) {
            var h1 = document.createElement("h1");
            h1.textContent = content;
            return h1.innerHTML;
        }

        function pathSplitter(path) {
            var tmp = "<span>" + path.replace(/::/g, "::</span><span>");
            if (tmp.endsWith("<span>")) {
                return tmp.slice(0, tmp.length - 6);
            }
            return tmp;
        }

        function addTab(array, query, display) {
            var extraStyle = "";
            if (display === false) {
                extraStyle = " style=\"display: none;\"";
            }

            var output = "";
            var duplicates = {};
            var length = 0;
            if (array.length > 0) {
                output = "<table class=\"search-results\"" + extraStyle + ">";

                array.forEach(function(item) {
                    var name, type;

                    name = item.name;
                    type = itemTypes[item.ty];

                    if (item.is_alias !== true) {
                        if (duplicates[item.fullPath]) {
                            return;
                        }
                        duplicates[item.fullPath] = true;
                    }
                    length += 1;

                    output += "<tr class=\"" + type + " result\"><td>" +
                              "<a href=\"" + item.href + "\">" +
                              (item.is_alias === true ?
                               ("<span class=\"alias\"><b>" + item.alias + " </b></span><span " +
                                  "class=\"grey\"><i>&nbsp;- see&nbsp;</i></span>") : "") +
                              item.displayPath + "<span class=\"" + type + "\">" +
                              name + "</span></a></td><td>" +
                              "<a href=\"" + item.href + "\">" +
                              "<span class=\"desc\">" + escape(item.desc) +
                              "&nbsp;</span></a></td></tr>";
                });
                output += "</table>";
            } else {
                output = "<div class=\"search-failed\"" + extraStyle + ">No results :(<br/>" +
                    "Try on <a href=\"https://duckduckgo.com/?q=" +
                    encodeURIComponent("rust " + query.query) +
                    "\">DuckDuckGo</a>?<br/><br/>" +
                    "Or try looking in one of these:<ul><li>The <a " +
                    "href=\"https://doc.rust-lang.org/reference/index.html\">Rust Reference</a> " +
                    " for technical details about the language.</li><li><a " +
                    "href=\"https://doc.rust-lang.org/rust-by-example/index.html\">Rust By " +
                    "Example</a> for expository code examples.</a></li><li>The <a " +
                    "href=\"https://doc.rust-lang.org/book/index.html\">Rust Book</a> for " +
                    "introductions to language features and the language itself.</li><li><a " +
                    "href=\"https://docs.rs\">Docs.rs</a> for documentation of crates released on" +
                    " <a href=\"https://crates.io/\">crates.io</a>.</li></ul></div>";
            }
            return [output, length];
        }

        function makeTabHeader(tabNb, text, nbElems) {
            if (currentTab === tabNb) {
                return "<div class=\"selected\">" + text +
                       " <div class=\"count\">(" + nbElems + ")</div></div>";
            }
            return "<div>" + text + " <div class=\"count\">(" + nbElems + ")</div></div>";
        }

        function showResults(results) {
            if (results.others.length === 1 &&
                getCurrentValue("rustdoc-go-to-only-result") === "true") {
                var elem = document.createElement("a");
                elem.href = results.others[0].href;
                elem.style.display = "none";
                // For firefox, we need the element to be in the DOM so it can be clicked.
                document.body.appendChild(elem);
                elem.click();
            }
            var query = getQuery(search_input.value);

            currentResults = query.id;

            var ret_others = addTab(results.others, query);
            var ret_in_args = addTab(results.in_args, query, false);
            var ret_returned = addTab(results.returned, query, false);

            var output = "<h1>Results for " + escape(query.query) +
                (query.type ? " (type: " + escape(query.type) + ")" : "") + "</h1>" +
                "<div id=\"titles\">" +
                makeTabHeader(0, "In Names", ret_others[1]) +
                makeTabHeader(1, "In Parameters", ret_in_args[1]) +
                makeTabHeader(2, "In Return Types", ret_returned[1]) +
                "</div><div id=\"results\">" +
                ret_others[0] + ret_in_args[0] + ret_returned[0] + "</div>";

            addClass(main, "hidden");
            var search = document.getElementById("search");
            removeClass(search, "hidden");
            search.innerHTML = output;
            var tds = search.getElementsByTagName("td");
            var td_width = 0;
            if (tds.length > 0) {
                td_width = tds[0].offsetWidth;
            }
            var width = search.offsetWidth - 40 - td_width;
            onEachLazy(search.getElementsByClassName("desc"), function(e) {
                e.style.width = width + "px";
            });
            initSearchNav();
            var elems = document.getElementById("titles").childNodes;
            elems[0].onclick = function() { printTab(0); };
            elems[1].onclick = function() { printTab(1); };
            elems[2].onclick = function() { printTab(2); };
            printTab(currentTab);
        }

        function execSearch(query, searchWords, filterCrates) {
            function getSmallest(arrays, positions, notDuplicates) {
                var start = null;

                for (var it = 0; it < positions.length; ++it) {
                    if (arrays[it].length > positions[it] &&
                        (start === null || start > arrays[it][positions[it]].lev) &&
                        !notDuplicates[arrays[it][positions[it]].fullPath]) {
                        start = arrays[it][positions[it]].lev;
                    }
                }
                return start;
            }

            function mergeArrays(arrays) {
                var ret = [];
                var positions = [];
                var notDuplicates = {};

                for (var x = 0; x < arrays.length; ++x) {
                    positions.push(0);
                }
                while (ret.length < MAX_RESULTS) {
                    var smallest = getSmallest(arrays, positions, notDuplicates);

                    if (smallest === null) {
                        break;
                    }
                    for (x = 0; x < arrays.length && ret.length < MAX_RESULTS; ++x) {
                        if (arrays[x].length > positions[x] &&
                                arrays[x][positions[x]].lev === smallest &&
                                !notDuplicates[arrays[x][positions[x]].fullPath]) {
                            ret.push(arrays[x][positions[x]]);
                            notDuplicates[arrays[x][positions[x]].fullPath] = true;
                            positions[x] += 1;
                        }
                    }
                }
                return ret;
            }

            var queries = query.raw.split(",");
            var results = {
                "in_args": [],
                "returned": [],
                "others": [],
            };

            for (var i = 0; i < queries.length; ++i) {
                query = queries[i].trim();
                if (query.length !== 0) {
                    var tmp = execQuery(getQuery(query), searchWords, filterCrates);

                    results.in_args.push(tmp.in_args);
                    results.returned.push(tmp.returned);
                    results.others.push(tmp.others);
                }
            }
            if (queries.length > 1) {
                return {
                    "in_args": mergeArrays(results.in_args),
                    "returned": mergeArrays(results.returned),
                    "others": mergeArrays(results.others),
                };
            } else {
                return {
                    "in_args": results.in_args[0],
                    "returned": results.returned[0],
                    "others": results.others[0],
                };
            }
        }

        function getFilterCrates() {
            var elem = document.getElementById("crate-search");

            if (elem && elem.value !== "All crates" && rawSearchIndex.hasOwnProperty(elem.value)) {
                return elem.value;
            }
            return undefined;
        }

        function search(e, forced) {
            var params = getQueryStringParams();
            var query = getQuery(search_input.value.trim());

            if (e) {
                e.preventDefault();
            }

            if (query.query.length === 0) {
                return;
            }
            if (forced !== true && query.id === currentResults) {
                if (query.query.length > 0) {
                    putBackSearch(search_input);
                }
                return;
            }

            // Update document title to maintain a meaningful browser history
            document.title = "Results for " + query.query + " - Rust";

            // Because searching is incremental by character, only the most
            // recent search query is added to the browser history.
            if (browserSupportsHistoryApi()) {
                if (!history.state && !params.search) {
                    history.pushState(query, "", "?search=" + encodeURIComponent(query.raw));
                } else {
                    history.replaceState(query, "", "?search=" + encodeURIComponent(query.raw));
                }
            }

            var filterCrates = getFilterCrates();
            showResults(execSearch(query, index, filterCrates), filterCrates);
        }

        function buildIndex(rawSearchIndex) {
            searchIndex = [];
            var searchWords = [];
            var i;

            for (var crate in rawSearchIndex) {
                if (!rawSearchIndex.hasOwnProperty(crate)) { continue; }

                searchWords.push(crate);
                searchIndex.push({
                    crate: crate,
                    ty: 1, // == ExternCrate
                    name: crate,
                    path: "",
                    desc: rawSearchIndex[crate].doc,
                    type: null,
                });

                // an array of [(Number) item type,
                //              (String) name,
                //              (String) full path or empty string for previous path,
                //              (String) description,
                //              (Number | null) the parent path index to `paths`]
                //              (Object | null) the type of the function (if any)
                var items = rawSearchIndex[crate].items;
                // an array of [(Number) item type,
                //              (String) name]
                var paths = rawSearchIndex[crate].paths;

                // convert `paths` into an object form
                var len = paths.length;
                for (i = 0; i < len; ++i) {
                    paths[i] = {ty: paths[i][0], name: paths[i][1]};
                }

                // convert `items` into an object form, and construct word indices.
                //
                // before any analysis is performed lets gather the search terms to
                // search against apart from the rest of the data.  This is a quick
                // operation that is cached for the life of the page state so that
                // all other search operations have access to this cached data for
                // faster analysis operations
                len = items.length;
                var lastPath = "";
                for (i = 0; i < len; ++i) {
                    var rawRow = items[i];
                    var row = {crate: crate, ty: rawRow[0], name: rawRow[1],
                               path: rawRow[2] || lastPath, desc: rawRow[3],
                               parent: paths[rawRow[4]], type: rawRow[5]};
                    searchIndex.push(row);
                    if (typeof row.name === "string") {
                        var word = row.name.toLowerCase();
                        searchWords.push(word);
                    } else {
                        searchWords.push("");
                    }
                    lastPath = row.path;
                }
            }
            return searchWords;
        }

        function startSearch() {
            var searchTimeout;
            var callback = function() {
                clearTimeout(searchTimeout);
                if (search_input.value.length === 0) {
                    if (browserSupportsHistoryApi()) {
                        history.replaceState("", "std - Rust", "?search=");
                    }
                    if (hasClass(main, "content")) {
                        removeClass(main, "hidden");
                    }
                    var search_c = document.getElementById("search");
                    if (hasClass(search_c, "content")) {
                        addClass(search_c, "hidden");
                    }
                } else {
                    searchTimeout = setTimeout(search, 500);
                }
            };
            search_input.onkeyup = callback;
            search_input.oninput = callback;
            document.getElementsByClassName("search-form")[0].onsubmit = function(e) {
                e.preventDefault();
                clearTimeout(searchTimeout);
                search();
            };
            search_input.onchange = function(e) {
                // Do NOT e.preventDefault() here. It will prevent pasting.
                clearTimeout(searchTimeout);
                // zero-timeout necessary here because at the time of event handler execution the
                // pasted content is not in the input field yet. Shouldn’t make any difference for
                // change, though.
                setTimeout(search, 0);
            };
            search_input.onpaste = search_input.onchange;

            var selectCrate = document.getElementById('crate-search');
            if (selectCrate) {
                selectCrate.onchange = function() {
                    search(undefined, true);
                };
            }

            // Push and pop states are used to add search results to the browser
            // history.
            if (browserSupportsHistoryApi()) {
                // Store the previous <title> so we can revert back to it later.
                var previousTitle = document.title;

                window.onpopstate = function(e) {
                    var params = getQueryStringParams();
                    // When browsing back from search results the main page
                    // visibility must be reset.
                    if (!params.search) {
                        if (hasClass(main, "content")) {
                            removeClass(main, "hidden");
                        }
                        var search_c = document.getElementById("search");
                        if (hasClass(search_c, "content")) {
                            addClass(search_c, "hidden");
                        }
                    }
                    // Revert to the previous title manually since the History
                    // API ignores the title parameter.
                    document.title = previousTitle;
                    // When browsing forward to search results the previous
                    // search will be repeated, so the currentResults are
                    // cleared to ensure the search is successful.
                    currentResults = null;
                    // Synchronize search bar with query string state and
                    // perform the search. This will empty the bar if there's
                    // nothing there, which lets you really go back to a
                    // previous state with nothing in the bar.
                    if (params.search) {
                        search_input.value = params.search;
                    } else {
                        search_input.value = "";
                    }
                    // Some browsers fire "onpopstate" for every page load
                    // (Chrome), while others fire the event only when actually
                    // popping a state (Firefox), which is why search() is
                    // called both here and at the end of the startSearch()
                    // function.
                    search();
                };
            }
            search();
        }

        index = buildIndex(rawSearchIndex);
        startSearch();

        // Draw a convenient sidebar of known crates if we have a listing
        if (rootPath === "../" || rootPath === "./") {
            var sidebar = document.getElementsByClassName("sidebar-elems")[0];
            if (sidebar) {
                var div = document.createElement("div");
                div.className = "block crate";
                div.innerHTML = "<h3>Crates</h3>";
                var ul = document.createElement("ul");
                div.appendChild(ul);

                var crates = [];
                for (var crate in rawSearchIndex) {
                    if (!rawSearchIndex.hasOwnProperty(crate)) {
                        continue;
                    }
                    crates.push(crate);
                }
                crates.sort();
                for (var i = 0; i < crates.length; ++i) {
                    var klass = "crate";
                    if (rootPath !== "./" && crates[i] === window.currentCrate) {
                        klass += " current";
                    }
                    var link = document.createElement("a");
                    link.href = rootPath + crates[i] + "/index.html";
                    link.title = rawSearchIndex[crates[i]].doc;
                    link.className = klass;
                    link.textContent = crates[i];

                    var li = document.createElement("li");
                    li.appendChild(link);
                    ul.appendChild(li);
                }
                sidebar.appendChild(div);
            }
        }
    }

    window.initSearch = initSearch;

    // delayed sidebar rendering.
    function initSidebarItems(items) {
        var sidebar = document.getElementsByClassName("sidebar-elems")[0];
        var current = window.sidebarCurrent;

        function block(shortty, longty) {
            var filtered = items[shortty];
            if (!filtered) {
                return;
            }

            var div = document.createElement("div");
            div.className = "block " + shortty;
            var h3 = document.createElement("h3");
            h3.textContent = longty;
            div.appendChild(h3);
            var ul = document.createElement("ul");

            var length = filtered.length;
            for (var i = 0; i < length; ++i) {
                var item = filtered[i];
                var name = item[0];
                var desc = item[1]; // can be null

                var klass = shortty;
                if (name === current.name && shortty === current.ty) {
                    klass += " current";
                }
                var path;
                if (shortty === "mod") {
                    path = name + "/index.html";
                } else {
                    path = shortty + "." + name + ".html";
                }
                var link = document.createElement("a");
                link.href = current.relpath + path;
                link.title = desc;
                link.className = klass;
                link.textContent = name;
                var li = document.createElement("li");
                li.appendChild(link);
                ul.appendChild(li);
            }
            div.appendChild(ul);
            if (sidebar) {
                sidebar.appendChild(div);
            }
        }

        block("primitive", "Primitive Types");
        block("mod", "Modules");
        block("macro", "Macros");
        block("struct", "Structs");
        block("enum", "Enums");
        block("union", "Unions");
        block("constant", "Constants");
        block("static", "Statics");
        block("trait", "Traits");
        block("fn", "Functions");
        block("type", "Type Definitions");
        block("foreigntype", "Foreign Types");
        block("keyword", "Keywords");
    }

    window.initSidebarItems = initSidebarItems;

    window.register_implementors = function(imp) {
        var implementors = document.getElementById("implementors-list");
        var synthetic_implementors = document.getElementById("synthetic-implementors-list");

        var libs = Object.getOwnPropertyNames(imp);
        var llength = libs.length;
        for (var i = 0; i < llength; ++i) {
            if (libs[i] === currentCrate) { continue; }
            var structs = imp[libs[i]];

            var slength = structs.length;
            struct_loop:
            for (var j = 0; j < slength; ++j) {
                var struct = structs[j];

                var list = struct.synthetic ? synthetic_implementors : implementors;

                if (struct.synthetic) {
                    var stlength = struct.types.length;
                    for (var k = 0; k < stlength; k++) {
                        if (window.inlined_types.has(struct.types[k])) {
                            continue struct_loop;
                        }
                        window.inlined_types.add(struct.types[k]);
                    }
                }

                var code = document.createElement("code");
                code.innerHTML = struct.text;

                var x = code.getElementsByTagName("a");
                var xlength = x.length;
                for (var it = 0; it < xlength; it++) {
                    var href = x[it].getAttribute("href");
                    if (href && href.indexOf("http") !== 0) {
                        x[it].setAttribute("href", rootPath + href);
                    }
                }
                var display = document.createElement("h3");
                addClass(display, "impl");
                display.innerHTML = "<span class=\"in-band\"><table class=\"table-display\">" +
                    "<tbody><tr><td><code>" + code.outerHTML + "</code></td><td></td></tr>" +
                    "</tbody></table></span>";
                list.appendChild(display);
            }
        }
    };
    if (window.pending_implementors) {
        window.register_implementors(window.pending_implementors);
    }

    function labelForToggleButton(sectionIsCollapsed) {
        if (sectionIsCollapsed) {
            // button will expand the section
            return "+";
        }
        // button will collapse the section
        // note that this text is also set in the HTML template in render.rs
        return "\u2212"; // "\u2212" is "−" minus sign
    }

    function onEveryMatchingChild(elem, className, func) {
        if (elem && className && func) {
            var length = elem.childNodes.length;
            var nodes = elem.childNodes;
            for (var i = 0; i < length; ++i) {
                if (hasClass(nodes[i], className)) {
                    func(nodes[i]);
                } else {
                    onEveryMatchingChild(nodes[i], className, func);
                }
            }
        }
    }

    function toggleAllDocs(pageId, fromAutoCollapse) {
        var innerToggle = document.getElementById("toggle-all-docs");
        if (!innerToggle) {
            return;
        }
        if (hasClass(innerToggle, "will-expand")) {
            updateLocalStorage("rustdoc-collapse", "false");
            removeClass(innerToggle, "will-expand");
            onEveryMatchingChild(innerToggle, "inner", function(e) {
                e.innerHTML = labelForToggleButton(false);
            });
            innerToggle.title = "collapse all docs";
            if (fromAutoCollapse !== true) {
                onEachLazy(document.getElementsByClassName("collapse-toggle"), function(e) {
                    collapseDocs(e, "show");
                });
            }
        } else {
            updateLocalStorage("rustdoc-collapse", "true");
            addClass(innerToggle, "will-expand");
            onEveryMatchingChild(innerToggle, "inner", function(e) {
                e.innerHTML = labelForToggleButton(true);
            });
            innerToggle.title = "expand all docs";
            if (fromAutoCollapse !== true) {
                onEachLazy(document.getElementsByClassName("collapse-toggle"), function(e) {
                    collapseDocs(e, "hide", pageId);
                });
            }
        }
    }

    function collapseDocs(toggle, mode, pageId) {
        if (!toggle || !toggle.parentNode) {
            return;
        }

        function adjustToggle(arg) {
            return function(e) {
                if (hasClass(e, "toggle-label")) {
                    if (arg) {
                        e.style.display = "inline-block";
                    } else {
                        e.style.display = "none";
                    }
                }
                if (hasClass(e, "inner")) {
                    e.innerHTML = labelForToggleButton(arg);
                }
            };
        }

        function implHider(addOrRemove) {
            return function(n) {
                var is_method = hasClass(n, "method");
                if (is_method || hasClass(n, "type")) {
                    if (is_method === true) {
                        if (addOrRemove) {
                            addClass(n, "hidden-by-impl-hider");
                        } else {
                            removeClass(n, "hidden-by-impl-hider");
                        }
                    }
                    var ns = n.nextElementSibling;
                    while (true) {
                        if (ns && (
                                hasClass(ns, "docblock") ||
                                hasClass(ns, "stability"))) {
                            if (addOrRemove) {
                                addClass(ns, "hidden-by-impl-hider");
                            } else {
                                removeClass(ns, "hidden-by-impl-hider");
                            }
                            ns = ns.nextElementSibling;
                            continue;
                        }
                        break;
                    }
                }
            };
        }

        var relatedDoc;
        var action = mode;
        if (hasClass(toggle.parentNode, "impl") === false) {
            relatedDoc = toggle.parentNode.nextElementSibling;
            if (hasClass(relatedDoc, "stability")) {
                relatedDoc = relatedDoc.nextElementSibling;
            }
            if (hasClass(relatedDoc, "docblock") || hasClass(relatedDoc, "sub-variant")) {
                if (mode === "toggle") {
                    if (hasClass(relatedDoc, "hidden-by-usual-hider")) {
                        action = "show";
                    } else {
                        action = "hide";
                    }
                }
                if (action === "hide") {
                    addClass(relatedDoc, "hidden-by-usual-hider");
                    onEachLazy(toggle.childNodes, adjustToggle(true));
                    addClass(toggle.parentNode, "collapsed");
                } else if (action === "show") {
                    removeClass(relatedDoc, "hidden-by-usual-hider");
                    removeClass(toggle.parentNode, "collapsed");
                    onEachLazy(toggle.childNodes, adjustToggle(false));
                }
            }
        } else {
            // we are collapsing the impl block

            var parentElem = toggle.parentNode;
            relatedDoc = parentElem;
            var docblock = relatedDoc.nextElementSibling;

            while (hasClass(relatedDoc, "impl-items") === false) {
                relatedDoc = relatedDoc.nextElementSibling;
            }

            if ((!relatedDoc && hasClass(docblock, "docblock") === false) ||
                (pageId && document.getElementById(pageId))) {
                return;
            }

            // Hide all functions, but not associated types/consts

            if (mode === "toggle") {
                if (hasClass(relatedDoc, "fns-now-collapsed") ||
                    hasClass(docblock, "hidden-by-impl-hider")) {
                    action = "show";
                } else {
                    action = "hide";
                }
            }

            if (action === "show") {
                removeClass(relatedDoc, "fns-now-collapsed");
                removeClass(docblock, "hidden-by-usual-hider");
                onEachLazy(toggle.childNodes, adjustToggle(false));
                onEachLazy(relatedDoc.childNodes, implHider(false));
            } else if (action === "hide") {
                addClass(relatedDoc, "fns-now-collapsed");
                addClass(docblock, "hidden-by-usual-hider");
                onEachLazy(toggle.childNodes, adjustToggle(true));
                onEachLazy(relatedDoc.childNodes, implHider(true));
            }
        }
    }

    function collapser(e, collapse) {
        // inherent impl ids are like "impl" or impl-<number>'.
        // they will never be hidden by default.
        var n = e.parentElement;
        if (n.id.match(/^impl(?:-\d+)?$/) === null) {
            // Automatically minimize all non-inherent impls
            if (collapse || hasClass(n, "impl")) {
                collapseDocs(e, "hide", pageId);
            }
        }
    }

    function autoCollapse(pageId, collapse) {
        if (collapse) {
            toggleAllDocs(pageId, true);
        } else if (getCurrentValue("rustdoc-trait-implementations") !== "false") {
            var impl_list = document.getElementById("implementations-list");

            if (impl_list !== null) {
                onEachLazy(impl_list.getElementsByClassName("collapse-toggle"), function(e) {
                    collapser(e, collapse);
                });
            }
        }
    }

    var toggles = document.getElementById("toggle-all-docs");
    if (toggles) {
        toggles.onclick = toggleAllDocs;
    }

    function insertAfter(newNode, referenceNode) {
        referenceNode.parentNode.insertBefore(newNode, referenceNode.nextSibling);
    }

    function createSimpleToggle(sectionIsCollapsed) {
        var toggle = document.createElement("a");
        toggle.href = "javascript:void(0)";
        toggle.className = "collapse-toggle";
        toggle.innerHTML = "[<span class=\"inner\">" + labelForToggleButton(sectionIsCollapsed) +
                           "</span>]";
        return toggle;
    }

    var toggle = createSimpleToggle(false);

    var func = function(e) {
        var next = e.nextElementSibling;
        if (!next) {
            return;
        }
        if (hasClass(next, "docblock") ||
            (hasClass(next, "stability") &&
             hasClass(next.nextElementSibling, "docblock"))) {
            insertAfter(toggle.cloneNode(true), e.childNodes[e.childNodes.length - 1]);
        }
    };

    var funcImpl = function(e) {
        var next = e.nextElementSibling;
        if (next && hasClass(next, "docblock")) {
            next = next.nextElementSibling;
        }
        if (!next) {
            return;
        }
        if (next.getElementsByClassName("method").length > 0 && hasClass(e, "impl")) {
            insertAfter(toggle.cloneNode(true), e.childNodes[e.childNodes.length - 1]);
        }
    };

    onEachLazy(document.getElementsByClassName("method"), func);
    onEachLazy(document.getElementsByClassName("associatedconstant"), func);
    onEachLazy(document.getElementsByClassName("impl"), funcImpl);
    var impl_call = function() {};
    if (getCurrentValue("rustdoc-method-docs") !== "false") {
        impl_call = function(e, newToggle, pageId) {
            if (e.id.match(/^impl(?:-\d+)?$/) === null) {
                // Automatically minimize all non-inherent impls
                if (hasClass(e, "impl")) {
                    collapseDocs(newToggle, "hide", pageId);
                }
            }
        };
    }
    var pageId = getPageId();
    var newToggle = document.createElement("a");
    newToggle.href = "javascript:void(0)";
    newToggle.className = "collapse-toggle hidden-default collapsed";
    newToggle.innerHTML = "[<span class=\"inner\">" + labelForToggleButton(true) +
                          "</span>] Show hidden undocumented items";
    function toggleClicked() {
        if (hasClass(this, "collapsed")) {
            removeClass(this, "collapsed");
            onEachLazy(this.parentNode.getElementsByClassName("hidden"), function(x) {
                if (hasClass(x, "content") === false) {
                    removeClass(x, "hidden");
                    addClass(x, "x");
                }
            }, true);
            this.innerHTML = "[<span class=\"inner\">" + labelForToggleButton(false) +
                             "</span>] Hide undocumented items";
        } else {
            addClass(this, "collapsed");
            onEachLazy(this.parentNode.getElementsByClassName("x"), function(x) {
                if (hasClass(x, "content") === false) {
                    addClass(x, "hidden");
                    removeClass(x, "x");
                }
            }, true);
            this.innerHTML = "[<span class=\"inner\">" + labelForToggleButton(true) +
                             "</span>] Show hidden undocumented items";
        }
    }
    onEachLazy(document.getElementsByClassName("impl-items"), function(e) {
        onEachLazy(e.getElementsByClassName("associatedconstant"), func);
        var hiddenElems = e.getElementsByClassName("hidden");
        var needToggle = false;

        var hlength = hiddenElems.length;
        for (var i = 0; i < hlength; ++i) {
            if (hasClass(hiddenElems[i], "content") === false &&
                hasClass(hiddenElems[i], "docblock") === false) {
                needToggle = true;
                break;
            }
        }
        if (needToggle === true) {
            var inner_toggle = newToggle.cloneNode(true);
            inner_toggle.onclick = toggleClicked;
            e.insertBefore(inner_toggle, e.firstChild);
            impl_call(e, inner_toggle, pageId);
        }
    });

    function createToggle(otherMessage, fontSize, extraClass, show) {
        var span = document.createElement("span");
        span.className = "toggle-label";
        if (show) {
            span.style.display = "none";
        }
        if (!otherMessage) {
            span.innerHTML = "&nbsp;Expand&nbsp;description";
        } else {
            span.innerHTML = otherMessage;
        }

        if (fontSize) {
            span.style.fontSize = fontSize;
        }

        var mainToggle = toggle.cloneNode(true);
        mainToggle.appendChild(span);

        var wrapper = document.createElement("div");
        wrapper.className = "toggle-wrapper";
        if (!show) {
            addClass(wrapper, "collapsed");
            var inner = mainToggle.getElementsByClassName("inner");
            if (inner && inner.length > 0) {
                inner[0].innerHTML = "+";
            }
        }
        if (extraClass) {
            addClass(wrapper, extraClass);
        }
        wrapper.appendChild(mainToggle);
        return wrapper;
    }

    var showItemDeclarations = getCurrentValue("rustdoc-item-declarations") === "false";
    function buildToggleWrapper(e) {
        if (hasClass(e, "autohide")) {
            var wrap = e.previousElementSibling;
            if (wrap && hasClass(wrap, "toggle-wrapper")) {
                var inner_toggle = wrap.childNodes[0];
                var extra = e.childNodes[0].tagName === "H3";

                e.style.display = "none";
                addClass(wrap, "collapsed");
                onEachLazy(inner_toggle.getElementsByClassName("inner"), function(e) {
                    e.innerHTML = labelForToggleButton(true);
                });
                onEachLazy(inner_toggle.getElementsByClassName("toggle-label"), function(e) {
                    e.style.display = "inline-block";
                    if (extra === true) {
                        i_e.innerHTML = " Show " + e.childNodes[0].innerHTML;
                    }
                });
            }
        }
        if (e.parentNode.id === "main") {
            var otherMessage = "";
            var fontSize;
            var extraClass;

            if (hasClass(e, "type-decl")) {
                fontSize = "20px";
                otherMessage = "&nbsp;Show&nbsp;declaration";
                if (showItemDeclarations === false) {
                    extraClass = "collapsed";
                }
            } else if (hasClass(e, "sub-variant")) {
                otherMessage = "&nbsp;Show&nbsp;fields";
            } else if (hasClass(e, "non-exhaustive")) {
                otherMessage = "&nbsp;This&nbsp;";
                if (hasClass(e, "non-exhaustive-struct")) {
                    otherMessage += "struct";
                } else if (hasClass(e, "non-exhaustive-enum")) {
                    otherMessage += "enum";
                } else if (hasClass(e, "non-exhaustive-type")) {
                    otherMessage += "type";
                }
                otherMessage += "&nbsp;is&nbsp;marked&nbsp;as&nbsp;non-exhaustive";
            } else if (hasClass(e.childNodes[0], "impl-items")) {
                extraClass = "marg-left";
            }

            e.parentNode.insertBefore(
                createToggle(otherMessage,
                             fontSize,
                             extraClass,
                             hasClass(e, "type-decl") === false || showItemDeclarations === true),
                e);
            if (hasClass(e, "type-decl") === true && showItemDeclarations === true) {
                collapseDocs(e.previousSibling.childNodes[0], "toggle");
            }
        }
    }

    onEachLazy(document.getElementsByClassName("docblock"), buildToggleWrapper);
    onEachLazy(document.getElementsByClassName("sub-variant"), buildToggleWrapper);

    // In the search display, allows to switch between tabs.
    function printTab(nb) {
        if (nb === 0 || nb === 1 || nb === 2) {
            currentTab = nb;
        }
        var nb_copy = nb;
        onEachLazy(document.getElementById("titles").childNodes, function(elem) {
            if (nb_copy === 0) {
                addClass(elem, "selected");
            } else {
                removeClass(elem, "selected");
            }
            nb_copy -= 1;
        });
        onEachLazy(document.getElementById("results").childNodes, function(elem) {
            if (nb === 0) {
                elem.style.display = "";
            } else {
                elem.style.display = "none";
            }
            nb -= 1;
        });
    }

    function createToggleWrapper(tog) {
        var span = document.createElement("span");
        span.className = "toggle-label";
        span.style.display = "none";
        span.innerHTML = "&nbsp;Expand&nbsp;attributes";
        tog.appendChild(span);

        var wrapper = document.createElement("div");
        wrapper.className = "toggle-wrapper toggle-attributes";
        wrapper.appendChild(tog);
        return wrapper;
    }

    // To avoid checking on "rustdoc-item-attributes" value on every loop...
    var itemAttributesFunc = function() {};
    if (getCurrentValue("rustdoc-item-attributes") !== "false") {
        itemAttributesFunc = function(x) {
            collapseDocs(x.previousSibling.childNodes[0], "toggle");
        };
    }
    var attributesToggle = createToggleWrapper(createSimpleToggle(false));
    onEachLazy(main.getElementsByClassName("attributes"), function(i_e) {
        i_e.parentNode.insertBefore(attributesToggle.cloneNode(true), i_e);
        itemAttributesFunc(i_e);
    });

    // To avoid checking on "rustdoc-line-numbers" value on every loop...
    var lineNumbersFunc = function() {};
    if (getCurrentValue("rustdoc-line-numbers") === "true") {
        lineNumbersFunc = function(x) {
            var count = x.textContent.split("\n").length;
            var elems = [];
            for (var i = 0; i < count; ++i) {
                elems.push(i + 1);
            }
            var node = document.createElement("pre");
            addClass(node, "line-number");
            node.innerHTML = elems.join("\n");
            x.parentNode.insertBefore(node, x);
        };
    }
    onEachLazy(document.getElementsByClassName("rust-example-rendered"), function(e) {
        if (hasClass(e, "compile_fail")) {
            e.addEventListener("mouseover", function(event) {
                this.parentElement.previousElementSibling.childNodes[0].style.color = "#f00";
            });
            e.addEventListener("mouseout", function(event) {
                this.parentElement.previousElementSibling.childNodes[0].style.color = "";
            });
        } else if (hasClass(e, "ignore")) {
            e.addEventListener("mouseover", function(event) {
                this.parentElement.previousElementSibling.childNodes[0].style.color = "#ff9200";
            });
            e.addEventListener("mouseout", function(event) {
                this.parentElement.previousElementSibling.childNodes[0].style.color = "";
            });
        }
        lineNumbersFunc(e);
    });

    function showModal(content) {
        var modal = document.createElement("div");
        modal.id = "important";
        addClass(modal, "modal");
        modal.innerHTML = "<div class=\"modal-content\"><div class=\"close\" id=\"modal-close\">✕" +
                          "</div><div class=\"whiter\"></div><span class=\"docblock\">" + content +
                          "</span></div>";
        document.getElementsByTagName("body")[0].appendChild(modal);
        document.getElementById("modal-close").onclick = hideModal;
        modal.onclick = hideModal;
    }

    function hideModal() {
        var modal = document.getElementById("important");
        if (modal) {
            modal.parentNode.removeChild(modal);
        }
    }

    onEachLazy(document.getElementsByClassName("important-traits"), function(e) {
        e.onclick = function() {
            showModal(e.lastElementChild.innerHTML);
        };
    });

    function putBackSearch(search_input) {
        if (search_input.value !== "") {
            addClass(main, "hidden");
            removeClass(document.getElementById("search"), "hidden");
            if (browserSupportsHistoryApi()) {
                history.replaceState(search_input.value,
                                     "",
                                     "?search=" + encodeURIComponent(search_input.value));
            }
        }
    }

    if (search_input) {
        search_input.onfocus = function() {
            putBackSearch(this);
        };
    }

    var params = getQueryStringParams();
    if (params && params.search) {
        addClass(main, "hidden");
        var search = document.getElementById("search");
        removeClass(search, "hidden");
        search.innerHTML = "<h3 style=\"text-align: center;\">Loading search results...</h3>";
    }

    var sidebar_menu = document.getElementsByClassName("sidebar-menu")[0];
    if (sidebar_menu) {
        sidebar_menu.onclick = function() {
            var sidebar = document.getElementsByClassName("sidebar")[0];
            if (hasClass(sidebar, "mobile") === true) {
                hideSidebar();
            } else {
                showSidebar();
            }
        };
    }

    window.onresize = function() {
        hideSidebar();
    };

    autoCollapse(getPageId(), getCurrentValue("rustdoc-collapse") === "true");

    if (window.location.hash && window.location.hash.length > 0) {
        expandSection(window.location.hash.replace(/^#/, ""));
    }

    if (main) {
        onEachLazy(main.getElementsByClassName("loading-content"), function(e) {
            e.remove();
        });
        onEachLazy(main.childNodes, function(e) {
            // Unhide the actual content once loading is complete. Headers get
            // flex treatment for their horizontal layout, divs get block treatment
            // for vertical layout (column-oriented flex layout for divs caused
            // errors in mobile browsers).
            if (e.tagName === "H2" || e.tagName === "H3") {
                let nextTagName = e.nextElementSibling.tagName;
                if (nextTagName == "H2" || nextTagName == "H3") {
                    e.nextElementSibling.style.display = "flex";
                } else {
                    e.nextElementSibling.style.display = "block";
                }
            }
        });
    }

    function addSearchOptions(crates) {
        var elem = document.getElementById('crate-search');

        if (!elem) {
            return;
        }
        var crates_text = [];
        if (Object.keys(crates).length > 1) {
            for (var crate in crates) {
                if (crates.hasOwnProperty(crate)) {
                    crates_text.push(crate);
                }
            }
        }
        crates_text.sort(function(a, b) {
            var lower_a = a.toLowerCase();
            var lower_b = b.toLowerCase();

            if (lower_a < lower_b) {
                return -1;
            } else if (lower_a > lower_b) {
                return 1;
            }
            return 0;
        });
        for (var i = 0; i < crates_text.length; ++i) {
            var option = document.createElement("option");
            option.value = crates_text[i];
            option.innerText = crates_text[i];
            elem.appendChild(option);
        }
    }

    window.addSearchOptions = addSearchOptions;
}());

// Sets the focus on the search bar at the top of the page
function focusSearchBar() {
    document.getElementsByClassName("search-input")[0].focus();
}

// Removes the focus from the search bar
function defocusSearchBar() {
    document.getElementsByClassName("search-input")[0].blur();
}
