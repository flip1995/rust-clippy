use super::*;

use crate::{
    bench::Bencher,
    console::OutputLocation,
    options::OutputFormat,
    time::{TimeThreshold, TestTimeOptions},
    formatters::PrettyFormatter,
    test::{
        filter_tests, parse_opts, run_test, DynTestFn, DynTestName, MetricMap,
        RunIgnored, RunStrategy, ShouldPanic, StaticTestName, TestDesc,
        TestDescAndFn, TestOpts, TrIgnored, TrOk,
        // FIXME (introduced by #65251)
        // ShouldPanic, StaticTestName, TestDesc, TestDescAndFn, TestOpts, TestTimeOptions,
        // TestType, TrFailedMsg, TrIgnored, TrOk,
    },
};
use std::sync::mpsc::channel;
use std::time::Duration;

impl TestOpts {
    fn new() -> TestOpts {
        TestOpts {
            list: false,
            filter: None,
            filter_exact: false,
            force_run_in_process: false,
            exclude_should_panic: false,
            run_ignored: RunIgnored::No,
            run_tests: false,
            bench_benchmarks: false,
            logfile: None,
            nocapture: false,
            color: AutoColor,
            format: OutputFormat::Pretty,
            test_threads: None,
            skip: vec![],
            time_options: None,
            options: Options::new(),
        }
    }
}

fn one_ignored_one_unignored_test() -> Vec<TestDescAndFn> {
    vec![
        TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("1"),
                ignore: true,
                should_panic: ShouldPanic::No,
                allow_fail: false,
                test_type: TestType::Unknown,
            },
            testfn: DynTestFn(Box::new(move || {})),
        },
        TestDescAndFn {
            desc: TestDesc {
                name: StaticTestName("2"),
                ignore: false,
                should_panic: ShouldPanic::No,
                allow_fail: false,
                test_type: TestType::Unknown,
            },
            testfn: DynTestFn(Box::new(move || {})),
        },
    ]
}

#[test]
pub fn do_not_run_ignored_tests() {
    fn f() {
        panic!();
    }
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: true,
            should_panic: ShouldPanic::No,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result != TrOk);
}

#[test]
pub fn ignored_tests_result_in_ignored() {
    fn f() {}
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: true,
            should_panic: ShouldPanic::No,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result == TrIgnored);
}

// FIXME: Re-enable emscripten once it can catch panics again (introduced by #65251)
#[test]
#[cfg(not(target_os = "emscripten"))]
fn test_should_panic() {
    fn f() {
        panic!();
    }
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::Yes,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result == TrOk);
}

// FIXME: Re-enable emscripten once it can catch panics again (introduced by #65251)
#[test]
#[cfg(not(target_os = "emscripten"))]
fn test_should_panic_good_message() {
    fn f() {
        panic!("an error message");
    }
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::YesWithMessage("error message"),
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result == TrOk);
}

// FIXME: Re-enable emscripten once it can catch panics again (introduced by #65251)
#[test]
#[cfg(not(target_os = "emscripten"))]
fn test_should_panic_bad_message() {
    use crate::tests::TrFailedMsg;
    fn f() {
        panic!("an error message");
    }
    let expected = "foobar";
    let failed_msg = "panic did not include expected string";
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::YesWithMessage(expected),
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result == TrFailedMsg(format!("{} '{}'", failed_msg, expected)));
}

// FIXME: Re-enable emscripten once it can catch panics again (introduced by #65251)
#[test]
#[cfg(not(target_os = "emscripten"))]
fn test_should_panic_but_succeeds() {
    fn f() {}
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::Yes,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let (tx, rx) = channel();
    run_test(&TestOpts::new(), false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;
    assert!(result == TrFailedMsg("test did not panic as expected".to_string()));
}

fn report_time_test_template(report_time: bool) -> Option<TestExecTime> {
    fn f() {}
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::No,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(f)),
    };
    let time_options = if report_time {
        Some(TestTimeOptions::default())
    } else {
        None
    };

    let test_opts = TestOpts {
        time_options,
        ..TestOpts::new()
    };
    let (tx, rx) = channel();
    run_test(&test_opts, false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let exec_time = rx.recv().unwrap().exec_time;
    exec_time
}

#[test]
fn test_should_not_report_time() {
    let exec_time = report_time_test_template(false);
    assert!(exec_time.is_none());
}

#[test]
fn test_should_report_time() {
    let exec_time = report_time_test_template(true);
    assert!(exec_time.is_some());
}

fn time_test_failure_template(test_type: TestType) -> TestResult {
    fn f() {}
    let desc = TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("whatever"),
            ignore: false,
            should_panic: ShouldPanic::No,
            allow_fail: false,
            test_type
        },
        testfn: DynTestFn(Box::new(f)),
    };
    // `Default` will initialize all the thresholds to 0 milliseconds.
    let mut time_options = TestTimeOptions::default();
    time_options.error_on_excess = true;

    let test_opts = TestOpts {
        time_options: Some(time_options),
        ..TestOpts::new()
    };
    let (tx, rx) = channel();
    run_test(&test_opts, false, desc, RunStrategy::InProcess, tx, Concurrent::No);
    let result = rx.recv().unwrap().result;

    result
}

#[test]
fn test_error_on_exceed() {
    let types = [TestType::UnitTest, TestType::IntegrationTest, TestType::DocTest];

    for test_type in types.iter() {
        let result = time_test_failure_template(*test_type);

        assert_eq!(result, TestResult::TrTimedFail);
    }

    // Check that for unknown tests thresholds aren't applied.
    let result = time_test_failure_template(TestType::Unknown);
    assert_eq!(result, TestResult::TrOk);
}

fn typed_test_desc(test_type: TestType) -> TestDesc {
    TestDesc {
        name: StaticTestName("whatever"),
        ignore: false,
        should_panic: ShouldPanic::No,
        allow_fail: false,
        test_type
    }
}

fn test_exec_time(millis: u64) -> TestExecTime {
    TestExecTime(Duration::from_millis(millis))
}

#[test]
fn test_time_options_threshold() {
    let unit = TimeThreshold::new(Duration::from_millis(50), Duration::from_millis(100));
    let integration = TimeThreshold::new(Duration::from_millis(500), Duration::from_millis(1000));
    let doc = TimeThreshold::new(Duration::from_millis(5000), Duration::from_millis(10000));

    let options = TestTimeOptions {
        error_on_excess: false,
        colored: false,
        unit_threshold: unit.clone(),
        integration_threshold: integration.clone(),
        doctest_threshold: doc.clone(),
    };

    let test_vector = [
        (TestType::UnitTest, unit.warn.as_millis() - 1, false, false),
        (TestType::UnitTest, unit.warn.as_millis(), true, false),
        (TestType::UnitTest, unit.critical.as_millis(), true, true),
        (TestType::IntegrationTest, integration.warn.as_millis() - 1, false, false),
        (TestType::IntegrationTest, integration.warn.as_millis(), true, false),
        (TestType::IntegrationTest, integration.critical.as_millis(), true, true),
        (TestType::DocTest, doc.warn.as_millis() - 1, false, false),
        (TestType::DocTest, doc.warn.as_millis(), true, false),
        (TestType::DocTest, doc.critical.as_millis(), true, true),
    ];

    for (test_type, time, expected_warn, expected_critical) in test_vector.iter() {
        let test_desc = typed_test_desc(*test_type);
        let exec_time = test_exec_time(*time as u64);

        assert_eq!(options.is_warn(&test_desc, &exec_time), *expected_warn);
        assert_eq!(options.is_critical(&test_desc, &exec_time), *expected_critical);
    }
}

#[test]
fn parse_ignored_flag() {
    let args = vec![
        "progname".to_string(),
        "filter".to_string(),
        "--ignored".to_string(),
    ];
    let opts = parse_opts(&args).unwrap().unwrap();
    assert_eq!(opts.run_ignored, RunIgnored::Only);
}

#[test]
fn parse_show_output_flag() {
    let args = vec![
        "progname".to_string(),
        "filter".to_string(),
        "--show-output".to_string(),
    ];
    let opts = parse_opts(&args).unwrap().unwrap();
    assert!(opts.options.display_output);
}

#[test]
fn parse_include_ignored_flag() {
    let args = vec![
        "progname".to_string(),
        "filter".to_string(),
        "-Zunstable-options".to_string(),
        "--include-ignored".to_string(),
    ];
    let opts = parse_opts(&args).unwrap().unwrap();
    assert_eq!(opts.run_ignored, RunIgnored::Yes);
}

#[test]
pub fn filter_for_ignored_option() {
    // When we run ignored tests the test filter should filter out all the
    // unignored tests and flip the ignore flag on the rest to false

    let mut opts = TestOpts::new();
    opts.run_tests = true;
    opts.run_ignored = RunIgnored::Only;

    let tests = one_ignored_one_unignored_test();
    let filtered = filter_tests(&opts, tests);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].desc.name.to_string(), "1");
    assert!(!filtered[0].desc.ignore);
}

#[test]
pub fn run_include_ignored_option() {
    // When we "--include-ignored" tests, the ignore flag should be set to false on
    // all tests and no test filtered out

    let mut opts = TestOpts::new();
    opts.run_tests = true;
    opts.run_ignored = RunIgnored::Yes;

    let tests = one_ignored_one_unignored_test();
    let filtered = filter_tests(&opts, tests);

    assert_eq!(filtered.len(), 2);
    assert!(!filtered[0].desc.ignore);
    assert!(!filtered[1].desc.ignore);
}

#[test]
pub fn exclude_should_panic_option() {
    let mut opts = TestOpts::new();
    opts.run_tests = true;
    opts.exclude_should_panic = true;

    let mut tests = one_ignored_one_unignored_test();
    tests.push(TestDescAndFn {
        desc: TestDesc {
            name: StaticTestName("3"),
            ignore: false,
            should_panic: ShouldPanic::Yes,
            allow_fail: false,
            test_type: TestType::Unknown,
        },
        testfn: DynTestFn(Box::new(move || {})),
    });

    let filtered = filter_tests(&opts, tests);

    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|test| test.desc.should_panic == ShouldPanic::No));
}

#[test]
pub fn exact_filter_match() {
    fn tests() -> Vec<TestDescAndFn> {
        vec!["base", "base::test", "base::test1", "base::test2"]
            .into_iter()
            .map(|name| TestDescAndFn {
                desc: TestDesc {
                    name: StaticTestName(name),
                    ignore: false,
                    should_panic: ShouldPanic::No,
                    allow_fail: false,
                    test_type: TestType::Unknown,
                },
                testfn: DynTestFn(Box::new(move || {})),
            })
            .collect()
    }

    let substr = filter_tests(
        &TestOpts {
            filter: Some("base".into()),
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(substr.len(), 4);

    let substr = filter_tests(
        &TestOpts {
            filter: Some("bas".into()),
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(substr.len(), 4);

    let substr = filter_tests(
        &TestOpts {
            filter: Some("::test".into()),
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(substr.len(), 3);

    let substr = filter_tests(
        &TestOpts {
            filter: Some("base::test".into()),
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(substr.len(), 3);

    let exact = filter_tests(
        &TestOpts {
            filter: Some("base".into()),
            filter_exact: true,
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(exact.len(), 1);

    let exact = filter_tests(
        &TestOpts {
            filter: Some("bas".into()),
            filter_exact: true,
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(exact.len(), 0);

    let exact = filter_tests(
        &TestOpts {
            filter: Some("::test".into()),
            filter_exact: true,
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(exact.len(), 0);

    let exact = filter_tests(
        &TestOpts {
            filter: Some("base::test".into()),
            filter_exact: true,
            ..TestOpts::new()
        },
        tests(),
    );
    assert_eq!(exact.len(), 1);
}

#[test]
pub fn sort_tests() {
    let mut opts = TestOpts::new();
    opts.run_tests = true;

    let names = vec![
        "sha1::test".to_string(),
        "isize::test_to_str".to_string(),
        "isize::test_pow".to_string(),
        "test::do_not_run_ignored_tests".to_string(),
        "test::ignored_tests_result_in_ignored".to_string(),
        "test::first_free_arg_should_be_a_filter".to_string(),
        "test::parse_ignored_flag".to_string(),
        "test::parse_include_ignored_flag".to_string(),
        "test::filter_for_ignored_option".to_string(),
        "test::run_include_ignored_option".to_string(),
        "test::sort_tests".to_string(),
    ];
    let tests = {
        fn testfn() {}
        let mut tests = Vec::new();
        for name in &names {
            let test = TestDescAndFn {
                desc: TestDesc {
                    name: DynTestName((*name).clone()),
                    ignore: false,
                    should_panic: ShouldPanic::No,
                    allow_fail: false,
                    test_type: TestType::Unknown,
                },
                testfn: DynTestFn(Box::new(testfn)),
            };
            tests.push(test);
        }
        tests
    };
    let filtered = filter_tests(&opts, tests);

    let expected = vec![
        "isize::test_pow".to_string(),
        "isize::test_to_str".to_string(),
        "sha1::test".to_string(),
        "test::do_not_run_ignored_tests".to_string(),
        "test::filter_for_ignored_option".to_string(),
        "test::first_free_arg_should_be_a_filter".to_string(),
        "test::ignored_tests_result_in_ignored".to_string(),
        "test::parse_ignored_flag".to_string(),
        "test::parse_include_ignored_flag".to_string(),
        "test::run_include_ignored_option".to_string(),
        "test::sort_tests".to_string(),
    ];

    for (a, b) in expected.iter().zip(filtered) {
        assert!(*a == b.desc.name.to_string());
    }
}

#[test]
pub fn test_metricmap_compare() {
    let mut m1 = MetricMap::new();
    let mut m2 = MetricMap::new();
    m1.insert_metric("in-both-noise", 1000.0, 200.0);
    m2.insert_metric("in-both-noise", 1100.0, 200.0);

    m1.insert_metric("in-first-noise", 1000.0, 2.0);
    m2.insert_metric("in-second-noise", 1000.0, 2.0);

    m1.insert_metric("in-both-want-downwards-but-regressed", 1000.0, 10.0);
    m2.insert_metric("in-both-want-downwards-but-regressed", 2000.0, 10.0);

    m1.insert_metric("in-both-want-downwards-and-improved", 2000.0, 10.0);
    m2.insert_metric("in-both-want-downwards-and-improved", 1000.0, 10.0);

    m1.insert_metric("in-both-want-upwards-but-regressed", 2000.0, -10.0);
    m2.insert_metric("in-both-want-upwards-but-regressed", 1000.0, -10.0);

    m1.insert_metric("in-both-want-upwards-and-improved", 1000.0, -10.0);
    m2.insert_metric("in-both-want-upwards-and-improved", 2000.0, -10.0);
}

#[test]
pub fn test_bench_once_no_iter() {
    fn f(_: &mut Bencher) {}
    bench::run_once(f);
}

#[test]
pub fn test_bench_once_iter() {
    fn f(b: &mut Bencher) {
        b.iter(|| {})
    }
    bench::run_once(f);
}

#[test]
pub fn test_bench_no_iter() {
    fn f(_: &mut Bencher) {}

    let (tx, rx) = channel();

    let desc = TestDesc {
        name: StaticTestName("f"),
        ignore: false,
        should_panic: ShouldPanic::No,
        allow_fail: false,
        test_type: TestType::Unknown,
    };

    crate::bench::benchmark(desc, tx, true, f);
    rx.recv().unwrap();
}

#[test]
pub fn test_bench_iter() {
    fn f(b: &mut Bencher) {
        b.iter(|| {})
    }

    let (tx, rx) = channel();

    let desc = TestDesc {
        name: StaticTestName("f"),
        ignore: false,
        should_panic: ShouldPanic::No,
        allow_fail: false,
        test_type: TestType::Unknown,
    };

    crate::bench::benchmark(desc, tx, true, f);
    rx.recv().unwrap();
}

#[test]
fn should_sort_failures_before_printing_them() {
    let test_a = TestDesc {
        name: StaticTestName("a"),
        ignore: false,
        should_panic: ShouldPanic::No,
        allow_fail: false,
        test_type: TestType::Unknown,
    };

    let test_b = TestDesc {
        name: StaticTestName("b"),
        ignore: false,
        should_panic: ShouldPanic::No,
        allow_fail: false,
        test_type: TestType::Unknown,
    };

    let mut out = PrettyFormatter::new(OutputLocation::Raw(Vec::new()), false, 10, false, None);

    let st = console::ConsoleTestState {
        log_out: None,
        total: 0,
        passed: 0,
        failed: 0,
        ignored: 0,
        allowed_fail: 0,
        filtered_out: 0,
        measured: 0,
        metrics: MetricMap::new(),
        failures: vec![(test_b, Vec::new()), (test_a, Vec::new())],
        options: Options::new(),
        not_failures: Vec::new(),
        time_failures: Vec::new(),
    };

    out.write_failures(&st).unwrap();
    let s = match out.output_location() {
        &OutputLocation::Raw(ref m) => String::from_utf8_lossy(&m[..]),
        &OutputLocation::Pretty(_) => unreachable!(),
    };

    let apos = s.find("a").unwrap();
    let bpos = s.find("b").unwrap();
    assert!(apos < bpos);
}
