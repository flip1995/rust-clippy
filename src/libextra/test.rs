// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[doc(hidden)];

// Support code for rustc's built in test runner generator. Currently,
// none of this is meant for users. It is intended to support the
// simplest interface possible for representing and running tests
// while providing a base that other test frameworks may build off of.


use getopts;
use sort;
use stats;
use stats::Stats;
use term;
use time::precise_time_ns;

use std::comm::{stream, SharedChan};
use std::either;
use std::io;
use std::option;
use std::result;
use std::task;
use std::to_str::ToStr;
use std::u64;
use std::uint;


// The name of a test. By convention this follows the rules for rust
// paths; i.e. it should be a series of identifiers separated by double
// colons. This way if some test runner wants to arrange the tests
// hierarchically it may.

pub enum TestName {
    StaticTestName(&'static str),
    DynTestName(~str)
}
impl ToStr for TestName {
    fn to_str(&self) -> ~str {
        match copy *self {
            StaticTestName(s) => s.to_str(),
            DynTestName(s) => s.to_str()
        }
    }
}

// A function that runs a test. If the function returns successfully,
// the test succeeds; if the function fails then the test fails. We
// may need to come up with a more clever definition of test in order
// to support isolation of tests into tasks.
pub enum TestFn {
    StaticTestFn(extern fn()),
    StaticBenchFn(extern fn(&mut BenchHarness)),
    DynTestFn(~fn()),
    DynBenchFn(~fn(&mut BenchHarness))
}

// Structure passed to BenchFns
pub struct BenchHarness {
    iterations: u64,
    ns_start: u64,
    ns_end: u64,
    bytes: u64
}

// The definition of a single test. A test runner will run a list of
// these.
pub struct TestDesc {
    name: TestName,
    ignore: bool,
    should_fail: bool
}

pub struct TestDescAndFn {
    desc: TestDesc,
    testfn: TestFn,
}

// The default console test runner. It accepts the command line
// arguments and a vector of test_descs.
pub fn test_main(args: &[~str], tests: ~[TestDescAndFn]) {
    let opts =
        match parse_opts(args) {
          either::Left(o) => o,
          either::Right(m) => fail!(m)
        };
    if !run_tests_console(&opts, tests) { fail!("Some tests failed"); }
}

// A variant optimized for invocation with a static test vector.
// This will fail (intentionally) when fed any dynamic tests, because
// it is copying the static values out into a dynamic vector and cannot
// copy dynamic values. It is doing this because from this point on
// a ~[TestDescAndFn] is used in order to effect ownership-transfer
// semantics into parallel test runners, which in turn requires a ~[]
// rather than a &[].
pub fn test_main_static(args: &[~str], tests: &[TestDescAndFn]) {
    let owned_tests = do tests.map |t| {
        match t.testfn {
            StaticTestFn(f) =>
            TestDescAndFn { testfn: StaticTestFn(f), desc: copy t.desc },

            StaticBenchFn(f) =>
            TestDescAndFn { testfn: StaticBenchFn(f), desc: copy t.desc },

            _ => {
                fail!("non-static tests passed to test::test_main_static");
            }
        }
    };
    test_main(args, owned_tests)
}

pub struct TestOpts {
    filter: Option<~str>,
    run_ignored: bool,
    run_tests: bool,
    run_benchmarks: bool,
    save_results: Option<Path>,
    compare_results: Option<Path>,
    logfile: Option<Path>
}

type OptRes = Either<TestOpts, ~str>;

// Parses command line arguments into test options
pub fn parse_opts(args: &[~str]) -> OptRes {
    let args_ = args.tail();
    let opts = ~[getopts::optflag("ignored"),
                 getopts::optflag("test"),
                 getopts::optflag("bench"),
                 getopts::optopt("save"),
                 getopts::optopt("diff"),
                 getopts::optopt("logfile")];
    let matches =
        match getopts::getopts(args_, opts) {
          Ok(m) => m,
          Err(f) => return either::Right(getopts::fail_str(f))
        };

    let filter =
        if matches.free.len() > 0 {
            option::Some(copy (matches).free[0])
        } else { option::None };

    let run_ignored = getopts::opt_present(&matches, "ignored");

    let logfile = getopts::opt_maybe_str(&matches, "logfile");
    let logfile = logfile.map(|s| Path(*s));

    let run_benchmarks = getopts::opt_present(&matches, "bench");
    let run_tests = ! run_benchmarks ||
        getopts::opt_present(&matches, "test");

    let save_results = getopts::opt_maybe_str(&matches, "save");
    let save_results = save_results.map(|s| Path(*s));

    let compare_results = getopts::opt_maybe_str(&matches, "diff");
    let compare_results = compare_results.map(|s| Path(*s));

    let test_opts = TestOpts {
        filter: filter,
        run_ignored: run_ignored,
        run_tests: run_tests,
        run_benchmarks: run_benchmarks,
        save_results: save_results,
        compare_results: compare_results,
        logfile: logfile
    };

    either::Left(test_opts)
}

#[deriving(Eq)]
pub struct BenchSamples {
    ns_iter_summ: stats::Summary,
    mb_s: uint
}

#[deriving(Eq)]
pub enum TestResult { TrOk, TrFailed, TrIgnored, TrBench(BenchSamples) }

struct ConsoleTestState {
    out: @io::Writer,
    log_out: Option<@io::Writer>,
    use_color: bool,
    total: uint,
    passed: uint,
    failed: uint,
    ignored: uint,
    benchmarked: uint,
    failures: ~[TestDesc]
}

// A simple console test runner
pub fn run_tests_console(opts: &TestOpts,
                         tests: ~[TestDescAndFn]) -> bool {
    fn callback(event: &TestEvent, st: &mut ConsoleTestState) {
        debug!("callback(event=%?)", event);
        match copy *event {
          TeFiltered(ref filtered_tests) => {
            st.total = filtered_tests.len();
            let noun = if st.total != 1 { ~"tests" } else { ~"test" };
            st.out.write_line(fmt!("\nrunning %u %s", st.total, noun));
          }
          TeWait(ref test) => st.out.write_str(
              fmt!("test %s ... ", test.name.to_str())),
          TeResult(test, result) => {
            match st.log_out {
                Some(f) => write_log(f, copy result, &test),
                None => ()
            }
            match result {
              TrOk => {
                st.passed += 1;
                write_ok(st.out, st.use_color);
                st.out.write_line("");
              }
              TrFailed => {
                st.failed += 1;
                write_failed(st.out, st.use_color);
                st.out.write_line("");
                st.failures.push(test);
              }
              TrIgnored => {
                st.ignored += 1;
                write_ignored(st.out, st.use_color);
                st.out.write_line("");
              }
              TrBench(bs) => {
                st.benchmarked += 1u;
                write_bench(st.out, st.use_color);
                st.out.write_line(fmt!(": %s",
                                       fmt_bench_samples(&bs)));
              }
            }
          }
        }
    }

    let log_out = match opts.logfile {
        Some(ref path) => match io::file_writer(path,
                                                [io::Create,
                                                 io::Truncate]) {
          result::Ok(w) => Some(w),
          result::Err(ref s) => {
              fail!("can't open output file: %s", *s)
          }
        },
        None => None
    };

    let st = @mut ConsoleTestState {
        out: io::stdout(),
        log_out: log_out,
        use_color: use_color(),
        total: 0u,
        passed: 0u,
        failed: 0u,
        ignored: 0u,
        benchmarked: 0u,
        failures: ~[]
    };

    run_tests(opts, tests, |x| callback(&x, st));

    assert!(st.passed + st.failed +
                 st.ignored + st.benchmarked == st.total);
    let success = st.failed == 0u;

    if !success {
        print_failures(st);
    }

    {
      let st: &mut ConsoleTestState = st;
      st.out.write_str(fmt!("\nresult: "));
      if success {
          // There's no parallelism at this point so it's safe to use color
          write_ok(st.out, true);
      } else {
          write_failed(st.out, true);
      }
      st.out.write_str(fmt!(". %u passed; %u failed; %u ignored\n\n",
                            st.passed, st.failed, st.ignored));
    }

    return success;

    fn fmt_bench_samples(bs: &BenchSamples) -> ~str {
        if bs.mb_s != 0 {
            fmt!("%u ns/iter (+/- %u) = %u MB/s",
                 bs.ns_iter_summ.median as uint,
                 (bs.ns_iter_summ.max - bs.ns_iter_summ.min) as uint,
                 bs.mb_s)
        } else {
            fmt!("%u ns/iter (+/- %u)",
                 bs.ns_iter_summ.median as uint,
                 (bs.ns_iter_summ.max - bs.ns_iter_summ.min) as uint)
        }
    }

    fn write_log(out: @io::Writer, result: TestResult, test: &TestDesc) {
        out.write_line(fmt!("%s %s",
                    match result {
                        TrOk => ~"ok",
                        TrFailed => ~"failed",
                        TrIgnored => ~"ignored",
                        TrBench(ref bs) => fmt_bench_samples(bs)
                    }, test.name.to_str()));
    }

    fn write_ok(out: @io::Writer, use_color: bool) {
        write_pretty(out, "ok", term::color::GREEN, use_color);
    }

    fn write_failed(out: @io::Writer, use_color: bool) {
        write_pretty(out, "FAILED", term::color::RED, use_color);
    }

    fn write_ignored(out: @io::Writer, use_color: bool) {
        write_pretty(out, "ignored", term::color::YELLOW, use_color);
    }

    fn write_bench(out: @io::Writer, use_color: bool) {
        write_pretty(out, "bench", term::color::CYAN, use_color);
    }

    fn write_pretty(out: @io::Writer,
                    word: &str,
                    color: term::color::Color,
                    use_color: bool) {
        let t = term::Terminal::new(out);
        match t {
            Ok(term)  => {
                if use_color {
                    term.fg(color);
                }
                out.write_str(word);
                if use_color {
                    term.reset();
                }
            },
            Err(_) => out.write_str(word)
        }
    }
}

fn print_failures(st: &ConsoleTestState) {
    st.out.write_line("\nfailures:");
    let mut failures = ~[];
    for uint::range(0, st.failures.len()) |i| {
        let name = copy st.failures[i].name;
        failures.push(name.to_str());
    }
    sort::tim_sort(failures);
    for failures.iter().advance |name| {
        st.out.write_line(fmt!("    %s", name.to_str()));
    }
}

#[test]
fn should_sort_failures_before_printing_them() {
    fn dummy() {}

    let s = do io::with_str_writer |wr| {
        let test_a = TestDesc {
            name: StaticTestName("a"),
            ignore: false,
            should_fail: false
        };

        let test_b = TestDesc {
            name: StaticTestName("b"),
            ignore: false,
            should_fail: false
        };

        let st = @ConsoleTestState {
            out: wr,
            log_out: option::None,
            use_color: false,
            total: 0u,
            passed: 0u,
            failed: 0u,
            ignored: 0u,
            benchmarked: 0u,
            failures: ~[test_b, test_a]
        };

        print_failures(st);
    };

    let apos = s.find_str("a").get();
    let bpos = s.find_str("b").get();
    assert!(apos < bpos);
}

fn use_color() -> bool { return get_concurrency() == 1; }

enum TestEvent {
    TeFiltered(~[TestDesc]),
    TeWait(TestDesc),
    TeResult(TestDesc, TestResult),
}

type MonitorMsg = (TestDesc, TestResult);

fn run_tests(opts: &TestOpts,
             tests: ~[TestDescAndFn],
             callback: &fn(e: TestEvent)) {

    let filtered_tests = filter_tests(opts, tests);
    let filtered_descs = filtered_tests.map(|t| copy t.desc);

    callback(TeFiltered(filtered_descs));

    let (filtered_tests, filtered_benchs) =
        do filtered_tests.partition |e| {
        match e.testfn {
            StaticTestFn(_) | DynTestFn(_) => true,
            StaticBenchFn(_) | DynBenchFn(_) => false
        }
    };

    // It's tempting to just spawn all the tests at once, but since we have
    // many tests that run in other processes we would be making a big mess.
    let concurrency = get_concurrency();
    debug!("using %u test tasks", concurrency);

    let mut remaining = filtered_tests;
    remaining.reverse();
    let mut pending = 0;

    let (p, ch) = stream();
    let ch = SharedChan::new(ch);

    while pending > 0 || !remaining.is_empty() {
        while pending < concurrency && !remaining.is_empty() {
            let test = remaining.pop();
            if concurrency == 1 {
                // We are doing one test at a time so we can print the name
                // of the test before we run it. Useful for debugging tests
                // that hang forever.
                callback(TeWait(copy test.desc));
            }
            run_test(!opts.run_tests, test, ch.clone());
            pending += 1;
        }

        let (desc, result) = p.recv();
        if concurrency != 1 {
            callback(TeWait(copy desc));
        }
        callback(TeResult(desc, result));
        pending -= 1;
    }

    // All benchmarks run at the end, in serial.
    for filtered_benchs.consume_iter().advance |b| {
        callback(TeWait(copy b.desc));
        run_test(!opts.run_benchmarks, b, ch.clone());
        let (test, result) = p.recv();
        callback(TeResult(test, result));
    }
}

// Windows tends to dislike being overloaded with threads.
#[cfg(windows)]
static SCHED_OVERCOMMIT : uint = 1;

#[cfg(unix)]
static SCHED_OVERCOMMIT : uint = 4u;

fn get_concurrency() -> uint {
    use std::rt;
    let threads = rt::util::default_sched_threads();
    if threads == 1 { 1 }
    else { threads * SCHED_OVERCOMMIT }
}

#[allow(non_implicitly_copyable_typarams)]
pub fn filter_tests(
    opts: &TestOpts,
    tests: ~[TestDescAndFn]) -> ~[TestDescAndFn]
{
    let mut filtered = tests;

    // Remove tests that don't match the test filter
    filtered = if opts.filter.is_none() {
        filtered
    } else {
        let filter_str = match opts.filter {
          option::Some(ref f) => copy *f,
          option::None => ~""
        };

        fn filter_fn(test: TestDescAndFn, filter_str: &str) ->
            Option<TestDescAndFn> {
            if test.desc.name.to_str().contains(filter_str) {
                return option::Some(test);
            } else { return option::None; }
        }

        filtered.consume_iter().filter_map(|x| filter_fn(x, filter_str)).collect()
    };

    // Maybe pull out the ignored test and unignore them
    filtered = if !opts.run_ignored {
        filtered
    } else {
        fn filter(test: TestDescAndFn) -> Option<TestDescAndFn> {
            if test.desc.ignore {
                let TestDescAndFn {desc, testfn} = test;
                Some(TestDescAndFn {
                    desc: TestDesc {ignore: false, ..desc},
                    testfn: testfn
                })
            } else {
                None
            }
        };
        filtered.consume_iter().filter_map(|x| filter(x)).collect()
    };

    // Sort the tests alphabetically
    fn lteq(t1: &TestDescAndFn, t2: &TestDescAndFn) -> bool {
        t1.desc.name.to_str() < t2.desc.name.to_str()
    }
    sort::quick_sort(filtered, lteq);

    filtered
}

struct TestFuture {
    test: TestDesc,
    wait: @fn() -> TestResult,
}

pub fn run_test(force_ignore: bool,
                test: TestDescAndFn,
                monitor_ch: SharedChan<MonitorMsg>) {

    let TestDescAndFn {desc, testfn} = test;

    if force_ignore || desc.ignore {
        monitor_ch.send((desc, TrIgnored));
        return;
    }

    fn run_test_inner(desc: TestDesc,
                      monitor_ch: SharedChan<MonitorMsg>,
                      testfn: ~fn()) {
        let testfn_cell = ::std::cell::Cell::new(testfn);
        do task::spawn {
            let mut result_future = None; // task::future_result(builder);

            let mut task = task::task();
            task.unlinked();
            task.future_result(|r| { result_future = Some(r) });
            task.spawn(testfn_cell.take());

            let task_result = result_future.unwrap().recv();
            let test_result = calc_result(&desc,
                                          task_result == task::Success);
            monitor_ch.send((copy desc, test_result));
        }
    }

    match testfn {
        DynBenchFn(benchfn) => {
            let bs = ::test::bench::benchmark(benchfn);
            monitor_ch.send((desc, TrBench(bs)));
            return;
        }
        StaticBenchFn(benchfn) => {
            let bs = ::test::bench::benchmark(benchfn);
            monitor_ch.send((desc, TrBench(bs)));
            return;
        }
        DynTestFn(f) => run_test_inner(desc, monitor_ch, f),
        StaticTestFn(f) => run_test_inner(desc, monitor_ch, || f())
    }
}

fn calc_result(desc: &TestDesc, task_succeeded: bool) -> TestResult {
    if task_succeeded {
        if desc.should_fail { TrFailed }
        else { TrOk }
    } else {
        if desc.should_fail { TrOk }
        else { TrFailed }
    }
}

impl BenchHarness {
    /// Callback for benchmark functions to run in their body.
    pub fn iter(&mut self, inner:&fn()) {
        self.ns_start = precise_time_ns();
        let k = self.iterations;
        for u64::range(0, k) |_| {
            inner();
        }
        self.ns_end = precise_time_ns();
    }

    pub fn ns_elapsed(&mut self) -> u64 {
        if self.ns_start == 0 || self.ns_end == 0 {
            0
        } else {
            self.ns_end - self.ns_start
        }
    }

    pub fn ns_per_iter(&mut self) -> u64 {
        if self.iterations == 0 {
            0
        } else {
            self.ns_elapsed() / self.iterations
        }
    }

    pub fn bench_n(&mut self, n: u64, f: &fn(&mut BenchHarness)) {
        self.iterations = n;
        debug!("running benchmark for %u iterations",
               n as uint);
        f(self);
    }

    // This is the Go benchmark algorithm. It produces a single
    // datapoint and always tries to run for 1s.
    pub fn go_bench(&mut self, f: &fn(&mut BenchHarness)) {

        // Rounds a number down to the nearest power of 10.
        fn round_down_10(n: u64) -> u64 {
            let mut n = n;
            let mut res = 1;
            while n > 10 {
                n = n / 10;
                res *= 10;
            }
            res
        }

        // Rounds x up to a number of the form [1eX, 2eX, 5eX].
        fn round_up(n: u64) -> u64 {
            let base = round_down_10(n);
            if n < (2 * base) {
                2 * base
            } else if n < (5 * base) {
                5 * base
            } else {
                10 * base
            }
        }

        // Initial bench run to get ballpark figure.
        let mut n = 1_u64;
        self.bench_n(n, |x| f(x));

        while n < 1_000_000_000 &&
            self.ns_elapsed() < 1_000_000_000 {
            let last = n;

            // Try to estimate iter count for 1s falling back to 1bn
            // iterations if first run took < 1ns.
            if self.ns_per_iter() == 0 {
                n = 1_000_000_000;
            } else {
                n = 1_000_000_000 / self.ns_per_iter();
            }

            n = u64::max(u64::min(n+n/2, 100*last), last+1);
            n = round_up(n);
            self.bench_n(n, |x| f(x));
        }
    }

    // This is a more statistics-driven benchmark algorithm.  It stops as
    // quickly as 100ms, so long as the statistical properties are
    // satisfactory. If those properties are not met, it may run as long as
    // the Go algorithm.
    pub fn auto_bench(&mut self, f: &fn(&mut BenchHarness)) -> stats::Summary {

        let mut magnitude = 1000;

        let samples : &mut [f64] = [0.0_f64, ..100];
        loop {
            let loop_start = precise_time_ns();

            for samples.mut_iter().advance() |p| {
                self.bench_n(magnitude as u64, |x| f(x));
                *p = self.ns_per_iter() as f64;
            };

            // Clip top 10% and bottom 10% of outliers
            stats::winsorize(samples, 10.0);
            let summ = stats::Summary::new(samples);

            debug!("%u samples, median %f, MAD=%f, MADP=%f",
                   samples.len(),
                   summ.median as float,
                   summ.median_abs_dev as float,
                   summ.median_abs_dev_pct as float);

            let now = precise_time_ns();
            let loop_run = now - loop_start;

            // Stop early if we have a good signal after a 100ms loop.
            if loop_run > 100_000_000 && summ.median_abs_dev_pct < 5.0 {
                return summ;
            }

            // Longest we ever run for is 1s.
            if loop_run > 1_000_000_000 {
                return summ;
            }

            magnitude *= 3;
            magnitude /= 2;
        }
    }
}

pub mod bench {
    use test::{BenchHarness, BenchSamples};

    pub fn benchmark(f: &fn(&mut BenchHarness)) -> BenchSamples {

        let mut bs = BenchHarness {
            iterations: 0,
            ns_start: 0,
            ns_end: 0,
            bytes: 0
        };

        let ns_iter_summ = bs.auto_bench(f);

        let iter_s = 1_000_000_000 / (ns_iter_summ.median as u64);
        let mb_s = (bs.bytes * iter_s) / 1_000_000;

        BenchSamples {
            ns_iter_summ: ns_iter_summ,
            mb_s: mb_s as uint
        }
    }
}

#[cfg(test)]
mod tests {
    use test::{TrFailed, TrIgnored, TrOk, filter_tests, parse_opts,
               TestDesc, TestDescAndFn,
               StaticTestName, DynTestName, DynTestFn};
    use test::{TestOpts, run_test};

    use std::either;
    use std::comm::{stream, SharedChan};
    use std::option;
    use std::vec;

    #[test]
    pub fn do_not_run_ignored_tests() {
        fn f() { fail!(); }
        let desc = TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("whatever"),
                ignore: true,
                should_fail: false
            },
            testfn: DynTestFn(|| f()),
        };
        let (p, ch) = stream();
        let ch = SharedChan::new(ch);
        run_test(false, desc, ch);
        let (_, res) = p.recv();
        assert!(res != TrOk);
    }

    #[test]
    pub fn ignored_tests_result_in_ignored() {
        fn f() { }
        let desc = TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("whatever"),
                ignore: true,
                should_fail: false
            },
            testfn: DynTestFn(|| f()),
        };
        let (p, ch) = stream();
        let ch = SharedChan::new(ch);
        run_test(false, desc, ch);
        let (_, res) = p.recv();
        assert_eq!(res, TrIgnored);
    }

    #[test]
    #[ignore(cfg(windows))]
    fn test_should_fail() {
        fn f() { fail!(); }
        let desc = TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("whatever"),
                ignore: false,
                should_fail: true
            },
            testfn: DynTestFn(|| f()),
        };
        let (p, ch) = stream();
        let ch = SharedChan::new(ch);
        run_test(false, desc, ch);
        let (_, res) = p.recv();
        assert_eq!(res, TrOk);
    }

    #[test]
    fn test_should_fail_but_succeeds() {
        fn f() { }
        let desc = TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("whatever"),
                ignore: false,
                should_fail: true
            },
            testfn: DynTestFn(|| f()),
        };
        let (p, ch) = stream();
        let ch = SharedChan::new(ch);
        run_test(false, desc, ch);
        let (_, res) = p.recv();
        assert_eq!(res, TrFailed);
    }

    #[test]
    fn first_free_arg_should_be_a_filter() {
        let args = ~[~"progname", ~"filter"];
        let opts = match parse_opts(args) {
          either::Left(o) => o,
          _ => fail!("Malformed arg in first_free_arg_should_be_a_filter")
        };
        assert!("filter" == (copy opts.filter).get());
    }

    #[test]
    fn parse_ignored_flag() {
        let args = ~[~"progname", ~"filter", ~"--ignored"];
        let opts = match parse_opts(args) {
          either::Left(o) => o,
          _ => fail!("Malformed arg in parse_ignored_flag")
        };
        assert!((opts.run_ignored));
    }

    #[test]
    pub fn filter_for_ignored_option() {
        fn dummy() {}

        // When we run ignored tests the test filter should filter out all the
        // unignored tests and flip the ignore flag on the rest to false

        let opts = TestOpts {
            filter: option::None,
            run_ignored: true,
            logfile: option::None,
            run_tests: true,
            run_benchmarks: false,
            save_results: option::None,
            compare_results: option::None
        };

        let tests = ~[
            TestDescAndFn {
                desc: TestDesc {
                    name: StaticTestName("1"),
                    ignore: true,
                    should_fail: false,
                },
                testfn: DynTestFn(|| {}),
            },
            TestDescAndFn {
                desc: TestDesc {
                    name: StaticTestName("2"),
                    ignore: false,
                    should_fail: false
                },
                testfn: DynTestFn(|| {}),
            },
        ];
        let filtered = filter_tests(&opts, tests);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].desc.name.to_str(), ~"1");
        assert!(filtered[0].desc.ignore == false);
    }

    #[test]
    pub fn sort_tests() {
        let opts = TestOpts {
            filter: option::None,
            run_ignored: false,
            logfile: option::None,
            run_tests: true,
            run_benchmarks: false,
            save_results: option::None,
            compare_results: option::None
        };

        let names =
            ~[~"sha1::test", ~"int::test_to_str", ~"int::test_pow",
             ~"test::do_not_run_ignored_tests",
             ~"test::ignored_tests_result_in_ignored",
             ~"test::first_free_arg_should_be_a_filter",
             ~"test::parse_ignored_flag", ~"test::filter_for_ignored_option",
             ~"test::sort_tests"];
        let tests =
        {
            fn testfn() { }
            let mut tests = ~[];
            for names.iter().advance |name| {
                let test = TestDescAndFn {
                    desc: TestDesc {
                        name: DynTestName(copy *name),
                        ignore: false,
                        should_fail: false
                    },
                    testfn: DynTestFn(copy testfn),
                };
                tests.push(test);
            }
            tests
        };
        let filtered = filter_tests(&opts, tests);

        let expected =
            ~[~"int::test_pow", ~"int::test_to_str", ~"sha1::test",
              ~"test::do_not_run_ignored_tests",
              ~"test::filter_for_ignored_option",
              ~"test::first_free_arg_should_be_a_filter",
              ~"test::ignored_tests_result_in_ignored",
              ~"test::parse_ignored_flag",
              ~"test::sort_tests"];

        let pairs = vec::zip(expected, filtered);

        for pairs.iter().advance |p| {
            match *p {
                (ref a, ref b) => {
                    assert!(*a == b.desc.name.to_str());
                }
            }
        }
    }
}
