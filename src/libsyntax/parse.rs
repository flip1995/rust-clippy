//! The main parser interface

#[legacy_exports];

export parser;
export common;
export lexer;
export token;
export comments;
export prec;
export classify;
export attr;

export parse_sess;
export new_parse_sess, new_parse_sess_special_handler;
export next_node_id;
export new_parser_from_file, new_parser_etc_from_file;
export new_parser_from_source_str;
export new_parser_from_tts;
export new_sub_parser_from_file;
export parse_crate_from_file, parse_crate_from_crate_file;
export parse_crate_from_source_str;
export parse_expr_from_source_str, parse_item_from_source_str;
export parse_stmt_from_source_str;
export parse_tts_from_source_str;
export parse_from_source_str;

use parser::Parser;
use attr::parser_attr;
use common::parser_common;
use ast::node_id;
use util::interner;
use diagnostic::{span_handler, mk_span_handler, mk_handler, emitter};
use lexer::{reader, string_reader};
use parse::token::{ident_interner, mk_ident_interner};
use codemap::{span, CodeMap, FileMap, CharPos, BytePos};

type parse_sess = @{
    cm: @codemap::CodeMap,
    mut next_id: node_id,
    span_diagnostic: span_handler,
    interner: @ident_interner,
};

fn new_parse_sess(demitter: Option<emitter>) -> parse_sess {
    let cm = @CodeMap::new();
    return @{cm: cm,
             mut next_id: 1,
             span_diagnostic: mk_span_handler(mk_handler(demitter), cm),
             interner: mk_ident_interner(),
            };
}

fn new_parse_sess_special_handler(sh: span_handler, cm: @codemap::CodeMap)
    -> parse_sess {
    return @{cm: cm,
             mut next_id: 1,
             span_diagnostic: sh,
             interner: mk_ident_interner(),
             };
}

fn parse_crate_from_file(input: &Path, cfg: ast::crate_cfg,
                         sess: parse_sess) -> @ast::crate {
    let p = new_crate_parser_from_file(sess, cfg, input);
    let r = p.parse_crate_mod(cfg);
    return r;
}

fn parse_crate_from_source_str(name: ~str, source: @~str, cfg: ast::crate_cfg,
                               sess: parse_sess) -> @ast::crate {
    let p = new_parser_from_source_str(sess, cfg, name,
                                       codemap::FssNone, source);
    let r = p.parse_crate_mod(cfg);
    p.abort_if_errors();
    return r;
}

fn parse_expr_from_source_str(name: ~str, source: @~str, cfg: ast::crate_cfg,
                              sess: parse_sess) -> @ast::expr {
    let p = new_parser_from_source_str(sess, cfg, name,
                                       codemap::FssNone, source);
    let r = p.parse_expr();
    p.abort_if_errors();
    return r;
}

fn parse_item_from_source_str(name: ~str, source: @~str, cfg: ast::crate_cfg,
                              +attrs: ~[ast::attribute],
                              sess: parse_sess) -> Option<@ast::item> {
    let p = new_parser_from_source_str(sess, cfg, name,
                                       codemap::FssNone, source);
    let r = p.parse_item(attrs);
    p.abort_if_errors();
    return r;
}

fn parse_stmt_from_source_str(name: ~str, source: @~str, cfg: ast::crate_cfg,
                              +attrs: ~[ast::attribute],
                              sess: parse_sess) -> @ast::stmt {
    let p = new_parser_from_source_str(sess, cfg, name,
                                       codemap::FssNone, source);
    let r = p.parse_stmt(attrs);
    p.abort_if_errors();
    return r;
}

fn parse_tts_from_source_str(name: ~str, source: @~str, cfg: ast::crate_cfg,
                             sess: parse_sess) -> ~[ast::token_tree] {
    let p = new_parser_from_source_str(sess, cfg, name,
                                       codemap::FssNone, source);
    p.quote_depth += 1u;
    let r = p.parse_all_token_trees();
    p.abort_if_errors();
    return r;
}

fn parse_from_source_str<T>(f: fn (p: Parser) -> T,
                            name: ~str, ss: codemap::FileSubstr,
                            source: @~str, cfg: ast::crate_cfg,
                            sess: parse_sess)
    -> T
{
    let p = new_parser_from_source_str(sess, cfg, name, ss,
                                       source);
    let r = f(p);
    if !p.reader.is_eof() {
        p.reader.fatal(~"expected end-of-string");
    }
    p.abort_if_errors();
    move r
}

fn next_node_id(sess: parse_sess) -> node_id {
    let rv = sess.next_id;
    sess.next_id += 1;
    // ID 0 is reserved for the crate and doesn't actually exist in the AST
    assert rv != 0;
    return rv;
}

fn new_parser_from_source_str(sess: parse_sess, cfg: ast::crate_cfg,
                              +name: ~str, +ss: codemap::FileSubstr,
                              source: @~str) -> Parser {
    let filemap = sess.cm.new_filemap_w_substr(name, ss, source);
    let srdr = lexer::new_string_reader(sess.span_diagnostic, filemap,
                                        sess.interner);
    return Parser(sess, cfg, srdr as reader);
}

fn new_parser_from_file(sess: parse_sess, cfg: ast::crate_cfg,
                        path: &Path) -> Result<Parser, ~str> {
    match io::read_whole_file_str(path) {
      result::Ok(move src) => {

          // HACK: If the file contains a special token use a different
          // source file. Used to send the stage1+ parser (the stage0 parser
          // doesn't have this hack) to a different crate file.
          // Transitional. Remove me.
          let src = if src.starts_with("// DIVERT") {
              let actual_path = &path.with_filestem("alternate_crate");
              result::unwrap(io::read_whole_file_str(actual_path))
          } else {
              move src
          };

          let filemap = sess.cm.new_filemap(path.to_str(), @move src);
          let srdr = lexer::new_string_reader(sess.span_diagnostic, filemap,
                                              sess.interner);

          Ok(Parser(sess, cfg, srdr as reader))

      }
      result::Err(move e) => Err(move e)
    }
}

/// Create a new parser for an entire crate, handling errors as appropriate
/// if the file doesn't exist
fn new_crate_parser_from_file(sess: parse_sess, cfg: ast::crate_cfg,
                              path: &Path) -> Parser {
    match new_parser_from_file(sess, cfg, path) {
        Ok(move parser) => move parser,
        Err(move e) => {
            sess.span_diagnostic.handler().fatal(e)
        }
    }
}

/// Create a new parser based on a span from an existing parser. Handles
/// error messages correctly when the file does not exist.
fn new_sub_parser_from_file(sess: parse_sess, cfg: ast::crate_cfg,
                            path: &Path, sp: span) -> Parser {
    match new_parser_from_file(sess, cfg, path) {
        Ok(move parser) => move parser,
        Err(move e) => {
            sess.span_diagnostic.span_fatal(sp, e)
        }
    }
}

fn new_parser_from_tts(sess: parse_sess, cfg: ast::crate_cfg,
                       tts: ~[ast::token_tree]) -> Parser {
    let trdr = lexer::new_tt_reader(sess.span_diagnostic, sess.interner,
                                    None, tts);
    return Parser(sess, cfg, trdr as reader)
}
