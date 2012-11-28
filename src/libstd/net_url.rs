//! Types/fns concerning URLs (see RFC 3986)
#[forbid(deprecated_mode)];

use core::cmp::Eq;
use map::HashMap;
use io::{Reader, ReaderUtil};
use dvec::DVec;
use from_str::FromStr;
use result::{Err, Ok};
use to_str::ToStr;
use to_bytes::IterBytes;

struct Url {
    scheme: ~str,
    user: Option<UserInfo>,
    host: ~str,
    port: Option<~str>,
    path: ~str,
    query: Query,
    fragment: Option<~str>
}

type UserInfo = {
    user: ~str,
    pass: Option<~str>
};

pub type Query = ~[(~str, ~str)];

pub pure fn Url(scheme: ~str, user: Option<UserInfo>, host: ~str,
       port: Option<~str>, path: ~str, query: Query,
       fragment: Option<~str>) -> Url {
    Url { scheme: move scheme, user: move user, host: move host,
         port: move port, path: move path, query: move query,
         fragment: move fragment }
}

pure fn UserInfo(user: ~str, pass: Option<~str>) -> UserInfo {
    {user: move user, pass: move pass}
}

fn encode_inner(s: &str, full_url: bool) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            let ch = rdr.read_byte() as char;
            match ch {
              // unreserved:
              'A' .. 'Z' |
              'a' .. 'z' |
              '0' .. '9' |
              '-' | '.' | '_' | '~' => {
                str::push_char(&mut out, ch);
              }
              _ => {
                  if full_url {
                    match ch {
                      // gen-delims:
                      ':' | '/' | '?' | '#' | '[' | ']' | '@' |

                      // sub-delims:
                      '!' | '$' | '&' | '"' | '(' | ')' | '*' |
                      '+' | ',' | ';' | '=' => {
                        str::push_char(&mut out, ch);
                      }

                      _ => out += fmt!("%%%X", ch as uint)
                    }
                } else {
                    out += fmt!("%%%X", ch as uint);
                }
              }
            }
        }

        out
    }
}

/**
 * Encodes a URI by replacing reserved characters with percent encoded
 * character sequences.
 *
 * This function is compliant with RFC 3986.
 */
pub pure fn encode(s: &str) -> ~str {
    // unsafe only because encode_inner does (string) IO
    unsafe {encode_inner(s, true)}
}

/**
 * Encodes a URI component by replacing reserved characters with percent
 * encoded character sequences.
 *
 * This function is compliant with RFC 3986.
 */

pub pure fn encode_component(s: &str) -> ~str {
    // unsafe only because encode_inner does (string) IO
    unsafe {encode_inner(s, false)}
}

fn decode_inner(s: &str, full_url: bool) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            match rdr.read_char() {
              '%' => {
                let bytes = rdr.read_bytes(2u);
                let ch = uint::parse_bytes(bytes, 16u).get() as char;

                if full_url {
                    // Only decode some characters:
                    match ch {
                      // gen-delims:
                      ':' | '/' | '?' | '#' | '[' | ']' | '@' |

                      // sub-delims:
                      '!' | '$' | '&' | '"' | '(' | ')' | '*' |
                      '+' | ',' | ';' | '=' => {
                        str::push_char(&mut out, '%');
                        str::push_char(&mut out, bytes[0u] as char);
                        str::push_char(&mut out, bytes[1u] as char);
                      }

                      ch => str::push_char(&mut out, ch)
                    }
                } else {
                      str::push_char(&mut out, ch);
                }
              }
              ch => str::push_char(&mut out, ch)
            }
        }

        out
    }
}

/**
 * Decode a string encoded with percent encoding.
 *
 * This will only decode escape sequences generated by encode_uri.
 */
pub pure fn decode(s: &str) -> ~str {
    // unsafe only because decode_inner does (string) IO
    unsafe {decode_inner(s, true)}
}

/**
 * Decode a string encoded with percent encoding.
 */
pub pure fn decode_component(s: &str) -> ~str {
    // unsafe only because decode_inner does (string) IO
    unsafe {decode_inner(s, false)}
}

fn encode_plus(s: &str) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            let ch = rdr.read_byte() as char;
            match ch {
              'A' .. 'Z' | 'a' .. 'z' | '0' .. '9' | '_' | '.' | '-' => {
                str::push_char(&mut out, ch);
              }
              ' ' => str::push_char(&mut out, '+'),
              _ => out += fmt!("%%%X", ch as uint)
            }
        }

        out
    }
}

/**
 * Encode a hashmap to the 'application/x-www-form-urlencoded' media type.
 */
pub fn encode_form_urlencoded(m: HashMap<~str, @DVec<@~str>>) -> ~str {
    let mut out = ~"";
    let mut first = true;

    for m.each |key, values| {
        let key = encode_plus(key);

        for (*values).each |value| {
            if first {
                first = false;
            } else {
                str::push_char(&mut out, '&');
                first = false;
            }

            out += fmt!("%s=%s", key, encode_plus(**value));
        }
    }

    out
}

/**
 * Decode a string encoded with the 'application/x-www-form-urlencoded' media
 * type into a hashmap.
 */
pub fn decode_form_urlencoded(s: ~[u8]) ->
    map::HashMap<~str, @dvec::DVec<@~str>> {
    do io::with_bytes_reader(s) |rdr| {
        let m = HashMap();
        let mut key = ~"";
        let mut value = ~"";
        let mut parsing_key = true;

        while !rdr.eof() {
            match rdr.read_char() {
              '&' | ';' => {
                if key != ~"" && value != ~"" {
                    let values = match m.find(key) {
                      Some(values) => values,
                      None => {
                        let values = @DVec();
                        m.insert(key, values);
                        values
                      }
                    };
                    (*values).push(@value)
                }

                parsing_key = true;
                key = ~"";
                value = ~"";
              }
              '=' => parsing_key = false,
              ch => {
                let ch = match ch {
                  '%' => {
                    uint::parse_bytes(rdr.read_bytes(2u), 16u).get() as char
                  }
                  '+' => ' ',
                  ch => ch
                };

                if parsing_key {
                    str::push_char(&mut key, ch)
                } else {
                    str::push_char(&mut value, ch)
                }
              }
            }
        }

        if key != ~"" && value != ~"" {
            let values = match m.find(key) {
              Some(values) => values,
              None => {
                let values = @DVec();
                m.insert(key, values);
                values
              }
            };
            (*values).push(@value)
        }

        m
    }
}


pure fn split_char_first(s: &str, c: char) -> (~str, ~str) {
    let len = str::len(s);
    let mut index = len;
    let mut mat = 0;
    unsafe {
        do io::with_str_reader(s) |rdr| {
            let mut ch : char;
            while !rdr.eof() {
                ch = rdr.read_byte() as char;
                if ch == c {
                    // found a match, adjust markers
                    index = rdr.tell()-1;
                    mat = 1;
                    break;
                }
            }
        }
    }
    if index+mat == len {
        return (str::slice(s, 0, index), ~"");
    } else {
        return (str::slice(s, 0, index),
             str::slice(s, index + mat, str::len(s)));
    }
}

pure fn userinfo_from_str(uinfo: &str) -> UserInfo {
    let (user, p) = split_char_first(uinfo, ':');
    let pass = if str::len(p) == 0 {
        option::None
    } else {
        option::Some(p)
    };
    return UserInfo(user, pass);
}

pure fn userinfo_to_str(userinfo: UserInfo) -> ~str {
    if option::is_some(&userinfo.pass) {
        return str::concat(~[copy userinfo.user, ~":",
                          option::unwrap(copy userinfo.pass),
                          ~"@"]);
    } else {
        return str::concat(~[copy userinfo.user, ~"@"]);
    }
}

impl UserInfo : Eq {
    pure fn eq(&self, other: &UserInfo) -> bool {
        (*self).user == (*other).user && (*self).pass == (*other).pass
    }
    pure fn ne(&self, other: &UserInfo) -> bool { !(*self).eq(other) }
}

pure fn query_from_str(rawquery: &str) -> Query {
    let mut query: Query = ~[];
    if str::len(rawquery) != 0 {
        for str::split_char(rawquery, '&').each |p| {
            let (k, v) = split_char_first(*p, '=');
            unsafe {query.push((decode_component(k), decode_component(v)));}
        };
    }
    return query;
}

pub pure fn query_to_str(query: Query) -> ~str {
    let mut strvec = ~[];
    for query.each |kv| {
        let (k, v) = copy *kv;
        // This is really safe...
        unsafe {
          strvec += ~[fmt!("%s=%s",
                           encode_component(k), encode_component(v))];
        }
    };
    return str::connect(strvec, ~"&");
}

// returns the scheme and the rest of the url, or a parsing error
pub pure fn get_scheme(rawurl: &str) -> result::Result<(~str, ~str), @~str> {
    for str::each_chari(rawurl) |i,c| {
        match c {
          'A' .. 'Z' | 'a' .. 'z' => loop,
          '0' .. '9' | '+' | '-' | '.' => {
            if i == 0 {
                return result::Err(@~"url: Scheme must begin with a letter.");
            }
            loop;
          }
          ':' => {
            if i == 0 {
                return result::Err(@~"url: Scheme cannot be empty.");
            } else {
                return result::Ok((rawurl.slice(0,i),
                                rawurl.slice(i+1,str::len(rawurl))));
            }
          }
          _ => {
            return result::Err(@~"url: Invalid character in scheme.");
          }
        }
    };
    return result::Err(@~"url: Scheme must be terminated with a colon.");
}

enum Input {
    Digit, // all digits
    Hex, // digits and letters a-f
    Unreserved // all other legal characters
}

impl Input : Eq {
    pure fn eq(&self, other: &Input) -> bool {
        match ((*self), (*other)) {
            (Digit, Digit) => true,
            (Hex, Hex) => true,
            (Unreserved, Unreserved) => true,
            (Digit, _) => false,
            (Hex, _) => false,
            (Unreserved, _) => false
        }
    }
    pure fn ne(&self, other: &Input) -> bool { !(*self).eq(other) }
}

// returns userinfo, host, port, and unparsed part, or an error
pure fn get_authority(rawurl: &str) ->
    result::Result<(Option<UserInfo>, ~str, Option<~str>, ~str), @~str> {
    if !str::starts_with(rawurl, ~"//") {
        // there is no authority.
        return result::Ok((option::None, ~"", option::None, rawurl.to_str()));
    }

    enum State {
        Start, // starting state
        PassHostPort, // could be in user or port
        Ip6Port, // either in ipv6 host or port
        Ip6Host, // are in an ipv6 host
        InHost, // are in a host - may be ipv6, but don't know yet
        InPort // are in port
    }

    let len = str::len(rawurl);
    let mut st : State = Start;
    let mut in : Input = Digit; // most restricted, start here.

    let mut userinfo : Option<UserInfo> = option::None;
    let mut host : ~str = ~"";
    let mut port : option::Option<~str> = option::None;

    let mut colon_count = 0;
    let mut pos : uint = 0, begin : uint = 2, end : uint = len;

    for str::each_chari(rawurl) |i,c| {
        if i < 2 { loop; } // ignore the leading //

        // deal with input class first
        match c {
          '0' .. '9' => (),
          'A' .. 'F' | 'a' .. 'f' => {
            if in == Digit {
                in = Hex;
            }
          }
          'G' .. 'Z' | 'g' .. 'z' | '-' | '.' | '_' | '~' | '%' |
          '&' |'\'' | '(' | ')' | '+' | '!' | '*' | ',' | ';' | '=' => {
            in = Unreserved;
          }
          ':' | '@' | '?' | '#' | '/' => {
            // separators, don't change anything
          }
          _ => {
            return result::Err(@~"Illegal character in authority");
          }
        }

        // now process states
        match c {
          ':' => {
            colon_count += 1;
            match st {
              Start => {
                pos = i;
                st = PassHostPort;
              }
              PassHostPort => {
                // multiple colons means ipv6 address.
                if in == Unreserved {
                    return result::Err(
                        @~"Illegal characters in IPv6 address.");
                }
                st = Ip6Host;
              }
              InHost => {
                pos = i;
                // can't be sure whether this is an ipv6 address or a port
                if in == Unreserved {
                    return result::Err(@~"Illegal characters in authority.");
                }
                st = Ip6Port;
              }
              Ip6Port => {
                if in == Unreserved {
                    return result::Err(@~"Illegal characters in authority.");
                }
                st = Ip6Host;
              }
              Ip6Host => {
                if colon_count > 7 {
                    host = str::slice(rawurl, begin, i);
                    pos = i;
                    st = InPort;
                }
              }
              _ => {
                return result::Err(@~"Invalid ':' in authority.");
              }
            }
            in = Digit; // reset input class
          }

          '@' => {
            in = Digit; // reset input class
            colon_count = 0; // reset count
            match st {
              Start => {
                let user = str::slice(rawurl, begin, i);
                userinfo = option::Some({user : user,
                                         pass: option::None});
                st = InHost;
              }
              PassHostPort => {
                let user = str::slice(rawurl, begin, pos);
                let pass = str::slice(rawurl, pos+1, i);
                userinfo = option::Some({user: user,
                                         pass: option::Some(pass)});
                st = InHost;
              }
              _ => {
                return result::Err(@~"Invalid '@' in authority.");
              }
            }
            begin = i+1;
          }

          '?' | '#' | '/' => {
            end = i;
            break;
          }
          _ => ()
        }
        end = i;
    }

    let end = end; // make end immutable so it can be captured

    let host_is_end_plus_one: &pure fn() -> bool = || {
        end+1 == len
            && !['?', '#', '/'].contains(&(rawurl[end] as char))
    };

    // finish up
    match st {
      Start => {
        if host_is_end_plus_one() {
            host = str::slice(rawurl, begin, end+1);
        } else {
            host = str::slice(rawurl, begin, end);
        }
      }
      PassHostPort | Ip6Port => {
        if in != Digit {
            return result::Err(@~"Non-digit characters in port.");
        }
        host = str::slice(rawurl, begin, pos);
        port = option::Some(str::slice(rawurl, pos+1, end));
      }
      Ip6Host | InHost => {
        host = str::slice(rawurl, begin, end);
      }
      InPort => {
        if in != Digit {
            return result::Err(@~"Non-digit characters in port.");
        }
        port = option::Some(str::slice(rawurl, pos+1, end));
      }
    }

    let rest = if host_is_end_plus_one() { ~"" }
    else { str::slice(rawurl, end, len) };
    return result::Ok((userinfo, host, port, rest));
}


// returns the path and unparsed part of url, or an error
pure fn get_path(rawurl: &str, authority : bool) ->
    result::Result<(~str, ~str), @~str> {
    let len = str::len(rawurl);
    let mut end = len;
    for str::each_chari(rawurl) |i,c| {
        match c {
          'A' .. 'Z' | 'a' .. 'z' | '0' .. '9' | '&' |'\'' | '(' | ')' | '.'
          | '@' | ':' | '%' | '/' | '+' | '!' | '*' | ',' | ';' | '='
          | '_' | '-' => {
            loop;
          }
          '?' | '#' => {
            end = i;
            break;
          }
          _ => return result::Err(@~"Invalid character in path.")
        }
    }

    if authority {
        if end != 0 && !str::starts_with(rawurl, ~"/") {
            return result::Err(@~"Non-empty path must begin with\
                               '/' in presence of authority.");
        }
    }

    return result::Ok((decode_component(str::slice(rawurl, 0, end)),
                    str::slice(rawurl, end, len)));
}

// returns the parsed query and the fragment, if present
pure fn get_query_fragment(rawurl: &str) ->
    result::Result<(Query, Option<~str>), @~str> {
    if !str::starts_with(rawurl, ~"?") {
        if str::starts_with(rawurl, ~"#") {
            let f = decode_component(str::slice(rawurl,
                                                1,
                                                str::len(rawurl)));
            return result::Ok((~[], option::Some(f)));
        } else {
            return result::Ok((~[], option::None));
        }
    }
    let (q, r) = split_char_first(str::slice(rawurl, 1,
                                             str::len(rawurl)), '#');
    let f = if str::len(r) != 0 {
        option::Some(decode_component(r)) } else { option::None };
    return result::Ok((query_from_str(q), f));
}

/**
 * Parse a `str` to a `url`
 *
 * # Arguments
 *
 * `rawurl` - a string representing a full url, including scheme.
 *
 * # Returns
 *
 * a `url` that contains the parsed representation of the url.
 *
 */

pub pure fn from_str(rawurl: &str) -> result::Result<Url, ~str> {
    // scheme
    let mut schm = get_scheme(rawurl);
    if result::is_err(&schm) {
        return result::Err(copy *result::get_err(&schm));
    }
    let (scheme, rest) = schm.get();

    // authority
    let mut auth = get_authority(rest);
    if result::is_err(&auth) {
        return result::Err(copy *result::get_err(&auth));
    }
    let (userinfo, host, port, rest) = auth.get();

    // path
    let has_authority = if host == ~"" { false } else { true };
    let mut pth = get_path(rest, has_authority);
    if result::is_err(&pth) {
        return result::Err(copy *result::get_err(&pth));
    }
    let (path, rest) = pth.get();

    // query and fragment
    let mut qry = get_query_fragment(rest);
    if result::is_err(&qry) {
        return result::Err(copy *result::get_err(&qry));
    }
    let (query, fragment) = qry.get();

    return result::Ok(Url(scheme, userinfo, host,
                       port, path, query, fragment));
}

impl Url : FromStr {
    static pure fn from_str(s: &str) -> Option<Url> {
        match from_str(s) {
            Ok(move url) => Some(url),
            Err(_) => None
        }
    }
}

/**
 * Format a `url` as a string
 *
 * # Arguments
 *
 * `url` - a url.
 *
 * # Returns
 *
 * a `str` that contains the formatted url. Note that this will usually
 * be an inverse of `from_str` but might strip out unneeded separators.
 * for example, "http://somehost.com?", when parsed and formatted, will
 * result in just "http://somehost.com".
 *
 */
pub pure fn to_str(url: Url) -> ~str {
    let user = if url.user.is_some() {
      userinfo_to_str(option::unwrap(copy url.user))
    } else {
       ~""
    };
    let authority = if str::len(url.host) != 0 {
        str::concat(~[~"//", user, copy url.host])
    } else {
        ~""
    };
    let query = if url.query.len() == 0 {
        ~""
    } else {
        str::concat(~[~"?", query_to_str(url.query)])
    };
    // ugh, this really is safe
    let fragment = if url.fragment.is_some() unsafe {
        str::concat(~[~"#", encode_component(
            option::unwrap(copy url.fragment))])
    } else {
        ~""
    };

    return str::concat(~[copy url.scheme,
                      ~":",
                      authority,
                      copy url.path,
                      query,
                      fragment]);
}

impl Url: to_str::ToStr {
    pub pure fn to_str() -> ~str {
        to_str(self)
    }
}

impl Url : Eq {
    pure fn eq(&self, other: &Url) -> bool {
        (*self).scheme == (*other).scheme
            && (*self).user == (*other).user
            && (*self).host == (*other).host
            && (*self).port == (*other).port
            && (*self).path == (*other).path
            && (*self).query == (*other).query
            && (*self).fragment == (*other).fragment
    }

    pure fn ne(&self, other: &Url) -> bool {
        !(*self).eq(other)
    }
}

#[cfg(stage0)]
impl Url: IterBytes {
    pure fn iter_bytes(lsb0: bool, f: to_bytes::Cb) {
        unsafe { self.to_str() }.iter_bytes(lsb0, f)
    }
}

#[cfg(stage1)]
#[cfg(stage2)]
impl Url: IterBytes {
    pure fn iter_bytes(&self, lsb0: bool, f: to_bytes::Cb) {
        unsafe { self.to_str() }.iter_bytes(lsb0, f)
    }
}

#[cfg(test)]
mod tests {
    #[legacy_exports];
    #[test]
    fn test_split_char_first() {
        let (u,v) = split_char_first(~"hello, sweet world", ',');
        assert u == ~"hello";
        assert v == ~" sweet world";

        let (u,v) = split_char_first(~"hello sweet world", ',');
        assert u == ~"hello sweet world";
        assert v == ~"";
    }

    #[test]
    fn test_get_authority() {
        let (u, h, p, r) = result::unwrap(get_authority(
            ~"//user:pass@rust-lang.org/something"));
        assert u == option::Some({user: ~"user",
                                  pass: option::Some(~"pass")});
        assert h == ~"rust-lang.org";
        assert p.is_none();
        assert r == ~"/something";

        let (u, h, p, r) = result::unwrap(get_authority(
            ~"//rust-lang.org:8000?something"));
        assert u.is_none();
        assert h == ~"rust-lang.org";
        assert p == option::Some(~"8000");
        assert r == ~"?something";

        let (u, h, p, r) = result::unwrap(get_authority(
            ~"//rust-lang.org#blah"));
        assert u.is_none();
        assert h == ~"rust-lang.org";
        assert p.is_none();
        assert r == ~"#blah";

        // ipv6 tests
        let (_, h, _, _) = result::unwrap(get_authority(
            ~"//2001:0db8:85a3:0042:0000:8a2e:0370:7334#blah"));
        assert h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334";

        let (_, h, p, _) = result::unwrap(get_authority(
            ~"//2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000#blah"));
        assert h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334";
        assert p == option::Some(~"8000");

        let (u, h, p, _) = result::unwrap(get_authority(
            ~"//us:p@2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000#blah"));
        assert u == option::Some({user: ~"us", pass : option::Some(~"p")});
        assert h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334";
        assert p == option::Some(~"8000");

        // invalid authorities;
        assert result::is_err(&get_authority(
            ~"//user:pass@rust-lang:something"));
        assert result::is_err(&get_authority(
            ~"//user@rust-lang:something:/path"));
        assert result::is_err(&get_authority(
            ~"//2001:0db8:85a3:0042:0000:8a2e:0370:7334:800a"));
        assert result::is_err(&get_authority(
            ~"//2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000:00"));

        // these parse as empty, because they don't start with '//'
        let (_, h, _, _) = result::unwrap(
            get_authority(~"user:pass@rust-lang"));
        assert h == ~"";
        let (_, h, _, _) = result::unwrap(
            get_authority(~"rust-lang.org"));
        assert h == ~"";

    }

    #[test]
    fn test_get_path() {
        let (p, r) = result::unwrap(get_path(
            ~"/something+%20orother", true));
        assert p == ~"/something+ orother";
        assert r == ~"";
        let (p, r) = result::unwrap(get_path(
            ~"test@email.com#fragment", false));
        assert p == ~"test@email.com";
        assert r == ~"#fragment";
        let (p, r) = result::unwrap(get_path(~"/gen/:addr=?q=v", false));
        assert p == ~"/gen/:addr=";
        assert r == ~"?q=v";

        //failure cases
        assert result::is_err(&get_path(~"something?q", true));

    }

    #[test]
    fn test_url_parse() {
        let url = ~"http://user:pass@rust-lang.org/doc?s=v#something";

        let up = from_str(url);
        let u = result::unwrap(up);
        assert u.scheme == ~"http";
        assert option::unwrap(copy u.user).user == ~"user";
        assert option::unwrap(copy option::unwrap(copy u.user).pass)
            == ~"pass";
        assert u.host == ~"rust-lang.org";
        assert u.path == ~"/doc";
        assert u.query.find(|kv| kv.first() == ~"s").get().second() == ~"v";
        assert option::unwrap(copy u.fragment) == ~"something";
    }

    #[test]
    fn test_url_parse_host_slash() {
        let urlstr = ~"http://0.42.42.42/";
        let url = from_str(urlstr).get();
        debug!("url: %?", url);
        assert url.host == ~"0.42.42.42";
        assert url.path == ~"/";
    }

    #[test]
    fn test_url_with_underscores() {
        let urlstr = ~"http://dotcom.com/file_name.html";
        let url = from_str(urlstr).get();
        debug!("url: %?", url);
        assert url.path == ~"/file_name.html";
    }

    #[test]
    fn test_url_with_dashes() {
        let urlstr = ~"http://dotcom.com/file-name.html";
        let url = from_str(urlstr).get();
        debug!("url: %?", url);
        assert url.path == ~"/file-name.html";
    }

    #[test]
    fn test_no_scheme() {
        assert result::is_err(&get_scheme(~"noschemehere.html"));
    }

    #[test]
    fn test_invalid_scheme_errors() {
        assert result::is_err(&from_str(~"99://something"));
        assert result::is_err(&from_str(~"://something"));
    }

    #[test]
    fn test_full_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?s=v#something";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_userless_url_parse_and_format() {
        let url = ~"http://rust-lang.org/doc?s=v#something";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_queryless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc#something";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_empty_query_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?#something";
        let should_be = ~"http://user:pass@rust-lang.org/doc#something";
        assert to_str(result::unwrap(from_str(url))) == should_be;
    }

    #[test]
    fn test_fragmentless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?q=v";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_minimal_url_parse_and_format() {
        let url = ~"http://rust-lang.org/doc";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_scheme_host_only_url_parse_and_format() {
        let url = ~"http://rust-lang.org";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_pathless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org?q=v#something";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_scheme_host_fragment_only_url_parse_and_format() {
        let url = ~"http://rust-lang.org#something";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_url_component_encoding() {
        let url = ~"http://rust-lang.org/doc%20uments?ba%25d%20=%23%26%2B";
        let u = result::unwrap(from_str(url));
        assert u.path == ~"/doc uments";
        assert u.query.find(|kv| kv.first() == ~"ba%d ")
            .get().second() == ~"#&+";
    }

    #[test]
    fn test_url_without_authority() {
        let url = ~"mailto:test@email.com";
        assert to_str(result::unwrap(from_str(url))) == url;
    }

    #[test]
    fn test_encode() {
        assert encode("") == ~"";
        assert encode("http://example.com") == ~"http://example.com";
        assert encode("foo bar% baz") == ~"foo%20bar%25%20baz";
        assert encode(" ") == ~"%20";
        assert encode("!") == ~"!";
        assert encode("\"") == ~"\"";
        assert encode("#") == ~"#";
        assert encode("$") == ~"$";
        assert encode("%") == ~"%25";
        assert encode("&") == ~"&";
        assert encode("'") == ~"%27";
        assert encode("(") == ~"(";
        assert encode(")") == ~")";
        assert encode("*") == ~"*";
        assert encode("+") == ~"+";
        assert encode(",") == ~",";
        assert encode("/") == ~"/";
        assert encode(":") == ~":";
        assert encode(";") == ~";";
        assert encode("=") == ~"=";
        assert encode("?") == ~"?";
        assert encode("@") == ~"@";
        assert encode("[") == ~"[";
        assert encode("]") == ~"]";
    }

    #[test]
    fn test_encode_component() {
        assert encode_component(~"") == ~"";
        assert encode_component(~"http://example.com") ==
            ~"http%3A%2F%2Fexample.com";
        assert encode_component(~"foo bar% baz") == ~"foo%20bar%25%20baz";
        assert encode_component(~" ") == ~"%20";
        assert encode_component(~"!") == ~"%21";
        assert encode_component(~"#") == ~"%23";
        assert encode_component(~"$") == ~"%24";
        assert encode_component(~"%") == ~"%25";
        assert encode_component(~"&") == ~"%26";
        assert encode_component(~"'") == ~"%27";
        assert encode_component(~"(") == ~"%28";
        assert encode_component(~")") == ~"%29";
        assert encode_component(~"*") == ~"%2A";
        assert encode_component(~"+") == ~"%2B";
        assert encode_component(~",") == ~"%2C";
        assert encode_component(~"/") == ~"%2F";
        assert encode_component(~":") == ~"%3A";
        assert encode_component(~";") == ~"%3B";
        assert encode_component(~"=") == ~"%3D";
        assert encode_component(~"?") == ~"%3F";
        assert encode_component(~"@") == ~"%40";
        assert encode_component(~"[") == ~"%5B";
        assert encode_component(~"]") == ~"%5D";
    }

    #[test]
    fn test_decode() {
        assert decode(~"") == ~"";
        assert decode(~"abc/def 123") == ~"abc/def 123";
        assert decode(~"abc%2Fdef%20123") == ~"abc%2Fdef 123";
        assert decode(~"%20") == ~" ";
        assert decode(~"%21") == ~"%21";
        assert decode(~"%22") == ~"%22";
        assert decode(~"%23") == ~"%23";
        assert decode(~"%24") == ~"%24";
        assert decode(~"%25") == ~"%";
        assert decode(~"%26") == ~"%26";
        assert decode(~"%27") == ~"'";
        assert decode(~"%28") == ~"%28";
        assert decode(~"%29") == ~"%29";
        assert decode(~"%2A") == ~"%2A";
        assert decode(~"%2B") == ~"%2B";
        assert decode(~"%2C") == ~"%2C";
        assert decode(~"%2F") == ~"%2F";
        assert decode(~"%3A") == ~"%3A";
        assert decode(~"%3B") == ~"%3B";
        assert decode(~"%3D") == ~"%3D";
        assert decode(~"%3F") == ~"%3F";
        assert decode(~"%40") == ~"%40";
        assert decode(~"%5B") == ~"%5B";
        assert decode(~"%5D") == ~"%5D";
    }

    #[test]
    fn test_decode_component() {
        assert decode_component(~"") == ~"";
        assert decode_component(~"abc/def 123") == ~"abc/def 123";
        assert decode_component(~"abc%2Fdef%20123") == ~"abc/def 123";
        assert decode_component(~"%20") == ~" ";
        assert decode_component(~"%21") == ~"!";
        assert decode_component(~"%22") == ~"\"";
        assert decode_component(~"%23") == ~"#";
        assert decode_component(~"%24") == ~"$";
        assert decode_component(~"%25") == ~"%";
        assert decode_component(~"%26") == ~"&";
        assert decode_component(~"%27") == ~"'";
        assert decode_component(~"%28") == ~"(";
        assert decode_component(~"%29") == ~")";
        assert decode_component(~"%2A") == ~"*";
        assert decode_component(~"%2B") == ~"+";
        assert decode_component(~"%2C") == ~",";
        assert decode_component(~"%2F") == ~"/";
        assert decode_component(~"%3A") == ~":";
        assert decode_component(~"%3B") == ~";";
        assert decode_component(~"%3D") == ~"=";
        assert decode_component(~"%3F") == ~"?";
        assert decode_component(~"%40") == ~"@";
        assert decode_component(~"%5B") == ~"[";
        assert decode_component(~"%5D") == ~"]";
    }

    #[test]
    fn test_encode_form_urlencoded() {
        let m = HashMap();
        assert encode_form_urlencoded(m) == ~"";

        m.insert(~"", @DVec());
        m.insert(~"foo", @DVec());
        assert encode_form_urlencoded(m) == ~"";

        let m = HashMap();
        m.insert(~"foo", @dvec::from_vec(~[@~"bar", @~"123"]));
        assert encode_form_urlencoded(m) == ~"foo=bar&foo=123";

        let m = HashMap();
        m.insert(~"foo bar", @dvec::from_vec(~[@~"abc", @~"12 = 34"]));
        assert encode_form_urlencoded(m) == ~"foo+bar=abc&foo+bar=12+%3D+34";
    }

    #[test]
    fn test_decode_form_urlencoded() {
        assert decode_form_urlencoded(~[]).size() == 0;

        let s = str::to_bytes(~"a=1&foo+bar=abc&foo+bar=12+%3D+34");
        assert decode_form_urlencoded(s).size() == 2;
    }

}

