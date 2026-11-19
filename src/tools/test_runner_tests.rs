use super::*;

#[test]
fn test_parsers_across_cargo_bun_go_jest_and_pytest() {
    let cargo_stdout = "\
running 3 tests
test math::adds ... ok
test math::ignored_case ... ignored
test math::divides_by_zero ... FAILED

failures:

---- math::divides_by_zero stdout ----
thread 'math::divides_by_zero' panicked at src/math.rs:42:9:
attempt to divide by zero

failures:
    math::divides_by_zero

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
    let cargo_res = parse_cargo_test_output(cargo_stdout, "");
    assert_eq!(cargo_res.passed, 1);
    assert_eq!(cargo_res.failed, 1);
    assert_eq!(cargo_res.skipped, 1);
    assert_eq!(cargo_res.failures.len(), 1);
    assert_eq!(cargo_res.failures[0].test, "math::divides_by_zero");
    assert_eq!(
        cargo_res.failures[0].location.as_deref(),
        Some("src/math.rs:42:9")
    );

    let bun_stderr = "\
✓ parses config [1.20ms]
✗ rejects invalid token [0.45ms]

 1 pass
 1 fail
 2 skip
";
    let bun_res = parse_bun_test_output("", bun_stderr);
    assert_eq!(bun_res.passed, 1);
    assert_eq!(bun_res.failed, 1);
    assert_eq!(bun_res.skipped, 2);
    assert_eq!(bun_res.failures[0].test, "rejects invalid token");

    let go_ndjson = r#"{"Action":"run","Package":"pkg/auth","Test":"TestVerify"}
{"Action":"output","Package":"pkg/auth","Test":"TestVerify","Output":"    auth_test.go:19: expected valid signature\n"}
{"Action":"fail","Package":"pkg/auth","Test":"TestVerify"}
{"Action":"pass","Package":"pkg/auth","Test":"TestRefresh"}
"#;
    let go_res = parse_go_json_output(go_ndjson);
    assert_eq!(go_res.passed, 1);
    assert_eq!(go_res.failed, 1);
    assert_eq!(go_res.failures[0].test, "pkg/auth::TestVerify");
    assert!(go_res.failures[0].summary.contains("auth_test.go:19"));

    let jest_json = r#"{"numPassedTests":4,"numFailedTests":1,"numPendingTests":1,"testResults":[{"name":"web/app.test.ts","assertionResults":[{"fullName":"renders header","status":"failed","failureMessages":["Expected 200, received 500\n    at app.test.ts:10:5"]}]}]}"#;
    let jest_res = parse_jest_vitest_json(jest_json).expect("valid jest json");
    assert_eq!(jest_res.passed, 4);
    assert_eq!(jest_res.failed, 1);
    assert_eq!(jest_res.skipped, 1);
    assert_eq!(jest_res.failures[0].test, "renders header");

    let pytest_out = "\
FAILED tests/test_api.py::test_quota - AssertionError: 429 != 200
======= 1 failed, 5 passed, 1 skipped in 0.14s =======
";
    let py_res = parse_pytest_output(pytest_out, "");
    assert_eq!(py_res.passed, 5);
    assert_eq!(py_res.failed, 1);
    assert_eq!(py_res.skipped, 1);
    assert_eq!(py_res.failures[0].test, "tests/test_api.py::test_quota");
}
