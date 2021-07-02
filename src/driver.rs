#![feature(rustc_private)]
#![feature(once_cell)]
#![cfg_attr(feature = "deny-warnings", deny(warnings))]
// warn on lints, that are included in `rust-lang/rust`s bootstrap
#![warn(rust_2018_idioms, unused_lifetimes)]
// warn on rustc internal lints
#![warn(rustc::internal)]

// FIXME: switch to something more ergonomic here, once available.
// (Currently there is no way to opt into sysroot crates without `extern crate`.)
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;

use rustc_interface::interface;
use rustc_session::{config, early_error, getopts, parse::ParseSess};
use rustc_span::symbol::Symbol;
use rustc_tools_util::VersionInfo;

use std::borrow::Cow;
use std::env;
use std::lazy::SyncLazy;
use std::panic;
use std::path::PathBuf;
use std::process::{exit, Command};

fn track_clippy_args(parse_sess: &mut ParseSess, args_env_var: &Option<String>) {
    parse_sess.env_depinfo.get_mut().insert((
        Symbol::intern("CLIPPY_ARGS"),
        args_env_var.as_deref().map(Symbol::intern),
    ));
}

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

/// This is different from `DefaultCallbacks` that it will inform Cargo to track the value of
/// `CLIPPY_ARGS` environment variable.
struct RustcCallbacks {
    clippy_args_var: Option<String>,
}

impl rustc_driver::Callbacks for RustcCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let clippy_args_var = self.clippy_args_var.take();
        config.parse_sess_created = Some(Box::new(move |parse_sess| {
            track_clippy_args(parse_sess, &clippy_args_var);
        }));
    }
}

struct ClippyCallbacks {
    clippy_args_var: Option<String>,
}

impl rustc_driver::Callbacks for ClippyCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let previous = config.register_lints.take();
        let clippy_args_var = self.clippy_args_var.take();
        config.parse_sess_created = Some(Box::new(move |parse_sess| {
            track_clippy_args(parse_sess, &clippy_args_var);
        }));
        config.register_lints = Some(Box::new(move |sess, lint_store| {
            // technically we're ~guaranteed that this is none but might as well call anything that
            // is there already. Certainly it can't hurt.
            if let Some(previous) = &previous {
                (previous)(sess, lint_store);
            }

            let conf = clippy_lints::read_conf(sess);
            clippy_lints::register_plugins(lint_store, sess, &conf);
            clippy_lints::register_pre_expansion_lints(lint_store);
            clippy_lints::register_renamed(lint_store);
        }));

        // FIXME: #4825; This is required, because Clippy lints that are based on MIR have to be
        // run on the unoptimized MIR. On the other hand this results in some false negatives. If
        // MIR passes can be enabled / disabled separately, we should figure out, what passes to
        // use for Clippy.
        config.opts.debugging_opts.mir_opt_level = Some(0);
    }
}

fn display_help() {
    println!(
        "\
Checks a package to catch common mistakes and improve your Rust code.

Usage:
    clippy-driver [options] [--] [<opts>...]

Common options:
    -h, --help               Print this message
        --rustc              Pass all args to rustc
    -V, --version            Print version info and exit

Other options are the same as `cargo check`.

To allow or deny a lint from the command line you can use `cargo clippy --`
with:

    -W --warn OPT       Set lint warnings
    -A --allow OPT      Set lint allowed
    -D --deny OPT       Set lint denied
    -F --forbid OPT     Set lint forbidden

You can use tool lints to allow or deny lints from your code, eg.:

    #[allow(clippy::needless_lifetimes)]
"
    );
}

const BUG_REPORT_URL: &str = "https://github.com/rust-lang/rust-clippy/issues/new";

static ICE_HOOK: SyncLazy<Box<dyn Fn(&panic::PanicInfo<'_>) + Sync + Send + 'static>> = SyncLazy::new(|| {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|info| report_clippy_ice(info, BUG_REPORT_URL)));
    hook
});

fn report_clippy_ice(info: &panic::PanicInfo<'_>, bug_report_url: &str) {
    // Invoke our ICE handler, which prints the actual panic message and optionally a backtrace
    (*ICE_HOOK)(info);

    // Separate the output with an empty line
    eprintln!();

    let emitter = Box::new(rustc_errors::emitter::EmitterWriter::stderr(
        rustc_errors::ColorConfig::Auto,
        None,
        false,
        false,
        None,
        false,
    ));
    let handler = rustc_errors::Handler::with_emitter(true, None, emitter);

    // a .span_bug or .bug call has already printed what
    // it wants to print.
    if !info.payload().is::<rustc_errors::ExplicitBug>() {
        let d = rustc_errors::Diagnostic::new(rustc_errors::Level::Bug, "unexpected panic");
        handler.emit_diagnostic(&d);
    }

    let version_info = rustc_tools_util::get_version_info!();

    let xs: Vec<Cow<'static, str>> = vec![
        "the compiler unexpectedly panicked. this is a bug.".into(),
        format!("we would appreciate a bug report: {}", bug_report_url).into(),
        format!("Clippy version: {}", version_info).into(),
    ];

    for note in &xs {
        handler.note_without_error(note);
    }

    // If backtraces are enabled, also print the query stack
    let backtrace = env::var_os("RUST_BACKTRACE").map_or(false, |x| &x != "0");

    let num_frames = if backtrace { None } else { Some(2) };

    interface::try_print_query_stack(&handler, num_frames);
}

fn toolchain_path(home: Option<String>, toolchain: Option<String>) -> Option<PathBuf> {
    home.and_then(|home| {
        toolchain.map(|toolchain| {
            let mut path = PathBuf::from(home);
            path.push("toolchains");
            path.push(toolchain);
            path
        })
    })
}

#[allow(clippy::too_many_lines)]
pub fn main() {
    let mut options = getopts::Options::new();
    for option in config::rustc_optgroups() {
        (option.apply)(&mut options);
    }
    // Clippy specific options. Make sure to remove them from `args` before passing it to
    // `RunCompiler`.
    options.optflag("", "no-deps", "Only run Clippy on the primary package");
    options.optflag("", "rustc", "Make clippy-driver behave like calling rustc directly");
    let clippy_args_var = env::var("CLIPPY_ARGS").ok();
    let mut args: Vec<_> = env::args()
        .chain(
            clippy_args_var
                .as_deref()
                .unwrap_or_default()
                .split("__CLIPPY_HACKERY__")
                .filter_map(|s| match s {
                    "" => None,
                    _ => Some(s.to_string()),
                }),
        )
        .chain(vec!["--cfg".into(), r#"feature="cargo-clippy""#.into()])
        .collect();
    let matches = options
        .parse(&args[1..])
        .unwrap_or_else(|err| early_error(config::ErrorOutputType::default(), &err.to_string()));

    rustc_driver::init_rustc_env_logger();
    SyncLazy::force(&ICE_HOOK);

    exit(rustc_driver::catch_with_exit_code(move || {
        // Get the sysroot, looking from most specific to this invocation to the least:
        // - command line
        // - runtime environment
        //    - SYSROOT
        //    - RUSTUP_HOME, MULTIRUST_HOME, RUSTUP_TOOLCHAIN, MULTIRUST_TOOLCHAIN
        // - sysroot from rustc in the path
        // - compile-time environment
        //    - SYSROOT
        //    - RUSTUP_HOME, MULTIRUST_HOME, RUSTUP_TOOLCHAIN, MULTIRUST_TOOLCHAIN
        let sys_root_arg = matches.opt_str("sysroot");
        let has_sys_root_arg = sys_root_arg.is_some();
        let sys_root = sys_root_arg
            .map(PathBuf::from)
            .or_else(|| std::env::var("SYSROOT").ok().map(PathBuf::from))
            .or_else(|| {
                let home = std::env::var("RUSTUP_HOME")
                    .or_else(|_| std::env::var("MULTIRUST_HOME"))
                    .ok();
                let toolchain = std::env::var("RUSTUP_TOOLCHAIN")
                    .or_else(|_| std::env::var("MULTIRUST_TOOLCHAIN"))
                    .ok();
                toolchain_path(home, toolchain)
            })
            .or_else(|| {
                Command::new("rustc")
                    .arg("--print")
                    .arg("sysroot")
                    .output()
                    .ok()
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .map(|s| PathBuf::from(s.trim()))
            })
            .or_else(|| option_env!("SYSROOT").map(PathBuf::from))
            .or_else(|| {
                let home = option_env!("RUSTUP_HOME")
                    .or(option_env!("MULTIRUST_HOME"))
                    .map(ToString::to_string);
                let toolchain = option_env!("RUSTUP_TOOLCHAIN")
                    .or(option_env!("MULTIRUST_TOOLCHAIN"))
                    .map(ToString::to_string);
                toolchain_path(home, toolchain)
            })
            .map(|pb| pb.to_string_lossy().to_string())
            .expect("need to specify SYSROOT env var during clippy compilation, or use rustup or multirust");

        // this conditional check for the --sysroot flag is there so users can call
        // `clippy_driver` directly without having to pass --sysroot or anything
        if !has_sys_root_arg {
            args.extend(vec!["--sysroot".into(), sys_root]);
        };

        let remove_arg = |args: &mut Vec<_>, name| {
            args.remove(
                args.iter()
                    .position(|arg| arg == name)
                    .expect("option must exist because it was parsed"),
            )
        };

        // Check for the --no-deps option. If present remove it from the args list.
        let no_deps = matches.opt_present("no-deps");
        if no_deps {
            remove_arg(&mut args, "--no-deps");
        }

        // make "clippy-driver --rustc" work like a subcommand that passes further args to "rustc"
        // for example `clippy-driver --rustc --version` will print the rustc version that clippy-driver
        // uses
        if matches.opt_present("rustc") {
            args[0] = "rustc".to_string();
            remove_arg(&mut args, "--rustc");

            return rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run();
        }

        if matches.opt_present("version") {
            let version_info = rustc_tools_util::get_version_info!();
            println!("{}", version_info);
            exit(0);
        }

        // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
        // We're invoking the compiler programmatically, so we ignore this
        let wrapper_mode = matches.free.contains(&String::from("rustc"));
        if wrapper_mode {
            // we still want to be able to invoke it normally though
            remove_arg(&mut args, "rustc");
        }

        if !wrapper_mode && (matches.opt_present("help") || env::args().len() == 1) {
            display_help();
            exit(0);
        }

        // We enable Clippy if one of the following conditions is met
        // - IF Clippy is run on its test suite OR
        // - IF Clippy is run on the main crate, not on deps (`!cap_lints_allow`) THEN
        //    - IF `--no-deps` is not set (`!no_deps`) OR
        //    - IF `--no-deps` is set and Clippy is run on the specified primary package
        let clippy_tests_set = env::var("__CLIPPY_INTERNAL_TESTS").map_or(false, |val| val == "true");
        let cap_lints_allow = matches.opt_str("cap-lints").map_or(false, |val| val == "allow");
        let in_primary_package = env::var("CARGO_PRIMARY_PACKAGE").is_ok();

        let clippy_enabled = clippy_tests_set || (!cap_lints_allow && (!no_deps || in_primary_package));

        if clippy_enabled {
            rustc_driver::RunCompiler::new(&args, &mut ClippyCallbacks { clippy_args_var }).run()
        } else {
            rustc_driver::RunCompiler::new(&args, &mut RustcCallbacks { clippy_args_var }).run()
        }
    }))
}
