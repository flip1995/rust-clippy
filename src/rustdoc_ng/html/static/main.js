// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*jslint browser: true, es5: true */
/*globals $: true, searchIndex: true, rootPath: true, allPaths: true */

(function () {
    "use strict";
    var resizeTimeout, interval;

    $('.js-only').removeClass('js-only');

    function resizeShortBlocks() {
        if (resizeTimeout) {
            clearTimeout(resizeTimeout);
        }
        resizeTimeout = setTimeout(function () {
            var contentWidth = $('.content').width();
            $('.docblock.short').width(function () {
                return contentWidth - 40 - $(this).prev().width();
            }).addClass('nowrap');
        }, 150);
    }
    resizeShortBlocks();
    $(window).on('resize', resizeShortBlocks);

    $(document).on('keyup', function (e) {
        if (document.activeElement.tagName === 'INPUT') {
            return;
        }

        if (e.keyCode === 188 && $('#help').hasClass('hidden')) { // question mark
            e.preventDefault();
            $('#help').removeClass('hidden');
        } else if (e.keyCode === 27 && !$('#help').hasClass('hidden')) { // esc
            e.preventDefault();
            $('#help').addClass('hidden');
        } else if (e.keyCode === 83) { // S
            e.preventDefault();
            $('.search-input').focus();
        }
    }).on('click', function (e) {
        if (!$(e.target).closest('#help').length) {
            $('#help').addClass('hidden');
        }
    });

    $('.version-selector').on('change', function () {
        var i, match,
            url = document.location.href,
            stripped = '',
            len = rootPath.match(/\.\.\//g).length + 1;

        for (i = 0; i < len; i += 1) {
            match = url.match(/\/[^\/]*$/);
            if (i < len - 1) {
                stripped = match[0] + stripped;
            }
            url = url.substring(0, url.length - match[0].length);
        }

        url += '/' + $('.version-selector').val() + stripped;

        document.location.href = url;
    });

    function initSearch(searchIndex) {
        var currentResults, index;

        // clear cached values from the search bar
        $(".search-input")[0].value = '';

        function execQuery(query, max, searchWords) {
            var valLower = query.query.toLowerCase(),
                val = valLower,
                typeFilter = query.type,
                results = [],
                aa = 0,
                bb = 0;

            // quoted values mean literal search
            bb = searchWords.length;
            if ((val.charAt(0) === "\"" || val.charAt(0) === "'") && val.charAt(val.length - 1) === val.charAt(0)) {
                val = val.substr(1, val.length - 2);
                for (aa = 0; aa < bb; aa += 1) {
                    if (searchWords[aa] === val) {
                        // filter type: ... queries
                        if (!typeFilter || typeFilter === searchIndex[aa].ty) {
                            results.push([aa, -1]);
                        }
                    }
                    if (results.length === max) {
                        break;
                    }
                }
            } else {
                // gather matching search results up to a certain maximum
                val = val.replace(/\_/g, "");
                for (aa = 0; aa < bb; aa += 1) {
                    if (searchWords[aa].indexOf(val) > -1 || searchWords[aa].replace(/_/g, "").indexOf(val) > -1) {
                        // filter type: ... queries
                        if (!typeFilter || typeFilter === searchIndex[aa].ty) {
                            results.push([aa, searchWords[aa].replace(/_/g, "").indexOf(val)]);
                        }
                    }
                    if (results.length === max) {
                        break;
                    }
                }
            }
            bb = results.length;
            for (aa = 0; aa < bb; aa += 1) {
                results[aa].push(searchIndex[results[aa][0]].ty);
            }
            for (aa = 0; aa < bb; aa += 1) {
                results[aa].push(searchIndex[results[aa][0]].path);
            }

            // if there are no results then return to default and fail
            if (results.length === 0) {
                return [];
            }

            // sort by exact match
            results.sort(function search_complete_sort0(aaa, bbb) {
                if (searchWords[aaa[0]] === valLower && searchWords[bbb[0]] !== valLower) {
                    return 1;
                }
            });
            // first sorting attempt
            // sort by item name length
            results.sort(function search_complete_sort1(aaa, bbb) {
                if (searchWords[aaa[0]].length > searchWords[bbb[0]].length) {
                    return 1;
                }
            });
            // second sorting attempt
            // sort by item name
            results.sort(function search_complete_sort1(aaa, bbb) {
                if (searchWords[aaa[0]].length === searchWords[bbb[0]].length && searchWords[aaa[0]] > searchWords[bbb[0]]) {
                    return 1;
                }
            });
            // third sorting attempt
            // sort by index of keyword in item name
            if (results[0][1] !== -1) {
                results.sort(function search_complete_sort1(aaa, bbb) {
                    if (aaa[1] > bbb[1] && bbb[1] === 0) {
                        return 1;
                    }
                });
            }
            // fourth sorting attempt
            // sort by type
            results.sort(function search_complete_sort3(aaa, bbb) {
                if (searchWords[aaa[0]] === searchWords[bbb[0]] && aaa[2] > bbb[2]) {
                    return 1;
                }
            });
            // fifth sorting attempt
            // sort by path
            results.sort(function search_complete_sort4(aaa, bbb) {
                if (searchWords[aaa[0]] === searchWords[bbb[0]] && aaa[2] === bbb[2] && aaa[3] > bbb[3]) {
                    return 1;
                }
            });
            // sixth sorting attempt
            // remove duplicates, according to the data provided
            for (aa = results.length - 1; aa > 0; aa -= 1) {
                if (searchWords[results[aa][0]] === searchWords[results[aa - 1][0]] && results[aa][2] === results[aa - 1][2] && results[aa][3] === results[aa - 1][3]) {
                    results[aa][0] = -1;
                }
            }

            return results;
        }

        function getQuery() {
            var matches, type, query = $('.search-input').val();

            matches = query.match(/^(fn|mod|str(uct)?|enum|trait|t(ype)?d(ef)?)\s*:\s*/i);
            if (matches) {
                type = matches[1].replace(/^td$/, 'typedef').replace(/^str$/, 'struct').replace(/^tdef$/, 'typedef').replace(/^typed$/, 'typedef');
                query = query.substring(matches[0].length);
            }

            return {
                query: query,
                type: type,
                id: query + type,
            };
        }

        function initSearchNav() {
            var hoverTimeout, $results = $('.search-results .result');

            $results.on('click', function () {
                document.location.href = $(this).find('a').prop('href');
            }).on('mouseover', function () {
                var $el = $(this);
                clearTimeout(hoverTimeout);
                hoverTimeout = setTimeout(function () {
                    $results.removeClass('highlighted');
                    $el.addClass('highlighted');
                }, 20);
            });

            $(document).off('keyup.searchnav');
            $(document).on('keyup.searchnav', function (e) {
                var $active = $results.filter('.highlighted');

                if (e.keyCode === 38) { // up
                    e.preventDefault();
                    if (!$active.length || !$active.prev()) {
                        return;
                    }

                    $active.prev().addClass('highlighted');
                    $active.removeClass('highlighted');
                } else if (e.keyCode === 40) { // down
                    e.preventDefault();
                    if (!$active.length) {
                        $results.first().addClass('highlighted');
                    } else if ($active.next().length) {
                        $active.next().addClass('highlighted');
                        $active.removeClass('highlighted');
                    }
                } else if (e.keyCode === 13) { // return
                    e.preventDefault();
                    if ($active.length) {
                        document.location.href = $active.find('a').prop('href');
                    }
                }
            });
        }

        function showResults(results) {
            var output, shown, query = getQuery();

            currentResults = query.id;
            output = '<h1>Results for ' + query.query + (query.type ? ' (type: ' + query.type + ')' : '') + '</h1>';
            output += '<table class="search-results">';

            if (results.length > 0) {
                shown = [];

                results.forEach(function (item) {
                    var name, type;

                    if (shown.indexOf(item) !== -1) {
                        return;
                    }

                    shown.push(item);
                    name = item.name;
                    type = item.ty;

                    output += '<tr class="' + type + ' result"><td>';

                    if (type === 'mod') {
                        output += item.path + '::<a href="' + rootPath + item.path.replace(/::/g, '/') + '/' + name + '/index.html" class="' + type + '">' + name + '</a>';
                    } else if (type === 'static' || type === 'reexport') {
                        output += item.path + '::<a href="' + rootPath + item.path.replace(/::/g, '/') + '/index.html" class="' + type + '">' + name + '</a>';
                    } else if (item.parent !== undefined) {
                        var myparent = allPaths[item.parent];
                        output += item.path + '::' + myparent.name + '::<a href="' + rootPath + item.path.replace(/::/g, '/') + '/' + myparent.type + '.' + myparent.name + '.html" class="' + type + '">' + name + '</a>';
                    } else {
                        output += item.path + '::<a href="' + rootPath + item.path.replace(/::/g, '/') + '/' + type + '.' + name + '.html" class="' + type + '">' + name + '</a>';
                    }

                    output += '</td><td><span class="desc">' + item.desc + '</span></td></tr>';
                });
            } else {
                output += 'No results :( <a href="https://duckduckgo.com/?q=' + encodeURIComponent('rust ' + query.query) + '">Try on DuckDuckGo?</a>';
            }

            output += "</p>";
            $('.content').html(output);
            $('.search-results .desc').width($('.content').width() - 40 - $('.content td:first-child').first().width());
            initSearchNav();
        }

        function search(e) {
            var query, filterdata = [], obj, i, len,
                results = [],
                maxResults = 200,
                resultIndex;

            query = getQuery();
            if (e) {
                e.preventDefault();
            }

            if (!query.query || query.id === currentResults) {
                return;
            }

            resultIndex = execQuery(query, 20000, index);
            len = resultIndex.length;
            for (i = 0; i < len; i += 1) {
                if (resultIndex[i][0] > -1) {
                    obj = searchIndex[resultIndex[i][0]];
                    filterdata.push([obj.name, obj.ty, obj.path, obj.desc]);
                    results.push(obj);
                }
                if (results.length >= maxResults) {
                    break;
                }
            }

            // TODO add sorting capability through this function?
            //
            //            // the handler for the table heading filtering
            //            filterdraw = function search_complete_filterdraw(node) {
            //                var name = "",
            //                    arrow = "",
            //                    op = 0,
            //                    tbody = node.parentNode.parentNode.nextSibling,
            //                    anchora = {},
            //                    tra = {},
            //                    tha = {},
            //                    td1a = {},
            //                    td2a = {},
            //                    td3a = {},
            //                    aaa = 0,
            //                    bbb = 0;
            //
            //                // the 4 following conditions set the rules for each
            //                // table heading
            //                if (node === ths[0]) {
            //                    op = 0;
            //                    name = "name";
            //                    ths[1].innerHTML = ths[1].innerHTML.split(" ")[0];
            //                    ths[2].innerHTML = ths[2].innerHTML.split(" ")[0];
            //                    ths[3].innerHTML = ths[3].innerHTML.split(" ")[0];
            //                }
            //                if (node === ths[1]) {
            //                    op = 1;
            //                    name = "type";
            //                    ths[0].innerHTML = ths[0].innerHTML.split(" ")[0];
            //                    ths[2].innerHTML = ths[2].innerHTML.split(" ")[0];
            //                    ths[3].innerHTML = ths[3].innerHTML.split(" ")[0];
            //                }
            //                if (node === ths[2]) {
            //                    op = 2;
            //                    name = "path";
            //                    ths[0].innerHTML = ths[0].innerHTML.split(" ")[0];
            //                    ths[1].innerHTML = ths[1].innerHTML.split(" ")[0];
            //                    ths[3].innerHTML = ths[3].innerHTML.split(" ")[0];
            //                }
            //                if (node === ths[3]) {
            //                    op = 3;
            //                    name = "description";
            //                    ths[0].innerHTML = ths[0].innerHTML.split(" ")[0];
            //                    ths[1].innerHTML = ths[1].innerHTML.split(" ")[0];
            //                    ths[2].innerHTML = ths[2].innerHTML.split(" ")[0];
            //                }
            //
            //                // ascending or descending search
            //                arrow = node.innerHTML.split(" ")[1];
            //                if (arrow === undefined || arrow === "\u25b2") {
            //                    arrow = "\u25bc";
            //                } else {
            //                    arrow = "\u25b2";
            //                }
            //
            //                // filter the data
            //                filterdata.sort(function search_complete_filterDraw_sort(xx, yy) {
            //                    if ((arrow === "\u25b2" && xx[op].toLowerCase() < yy[op].toLowerCase()) || (arrow === "\u25bc" && xx[op].toLowerCase() > yy[op].toLowerCase())) {
            //                        return 1;
            //                    }
            //                });
            //            };

            showResults(results);
        }

        function buildIndex(searchIndex) {
            var len = searchIndex.length,
                i = 0,
                searchWords = [];

            // before any analysis is performed lets gather the search terms to
            // search against apart from the rest of the data.  This is a quick
            // operation that is cached for the life of the page state so that
            // all other search operations have access to this cached data for
            // faster analysis operations
            for (i = 0; i < len; i += 1) {
                if (typeof searchIndex[i].name === "string") {
                    searchWords.push(searchIndex[i].name.toLowerCase());
                } else {
                    searchWords.push("");
                }
            }

            return searchWords;
        }

        function startSearch() {
            var keyUpTimeout;
            $('.do-search').on('click', search);
            $('.search-input').on('keyup', function () {
                clearTimeout(keyUpTimeout);
                keyUpTimeout = setTimeout(search, 100);
            });
        }

        index = buildIndex(searchIndex);
        startSearch();
    }

    initSearch(searchIndex);
}());
