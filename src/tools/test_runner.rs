//! Structured multi-language test runner (`test_runner`).
//!
//! Auto-detects or executes `cargo test`, `bun test`, `vitest`, `jest`, `pytest`,
//! or `go test -json` and normalizes pass/fail/skip counts and structured failure
//! diagnostics into a uniform JSON payload without regex.

use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Command;

pub struct TestRunnerTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFramework {
    Cargo,
    Bun,
    Vitest,
    Jest,
    Pytest,
    Go,
}

impl TestFramework {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Bun => "bun",
            Self::Vitest => "vitest",
            Self::Jest => "jest",
            Self::Pytest => "pytest",
            Self::Go => "go",
        }
    }

    pub fn from_explicit(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cargo" | "rust" | "cargo_test" => Some(Self::Cargo),
            "bun" | "bun_test" => Some(Self::Bun),
            "vitest" => Some(Self::Vitest),
            "jest" => Some(Self::Jest),
            "pytest" | "python" | "py" => Some(Self::Pytest),
            "go" | "golang" | "go_test" => Some(Self::Go),
            _ => None,
        }
    }

    pub fn detect(cwd: &Path, target: Option<&str>) -> Result<Self> {
        let ext_hint = target
            .and_then(|t| Path::new(t).extension())
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());

        match ext_hint.as_deref() {
            Some("rs") => return Ok(Self::Cargo),
            Some("go") => return Ok(Self::Go),
            Some("py") => return Ok(Self::Pytest),
            Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs") => {
                return Ok(detect_js_framework(cwd));
            }
            _ => {}
        }

        let has_cargo = cwd.join("Cargo.toml").is_file();
        let has_go = cwd.join("go.mod").is_file();
        let has_pkg_json = cwd.join("package.json").is_file();
        let has_py = cwd.join("pyproject.toml").is_file()
            || cwd.join("pytest.ini").is_file()
            || cwd.join("setup.py").is_file();

        match (has_cargo, has_go, has_pkg_json, has_py) {
            (true, _, _, _) => Ok(Self::Cargo),
            (false, true, _, _) => Ok(Self::Go),
            (false, false, true, _) => Ok(detect_js_framework(cwd)),
            (false, false, false, true) => Ok(Self::Pytest),
            (false, false, false, false) => Err(anyhow!(
                "Could not auto-detect test framework in {:?}; pass explicit 'framework' ('cargo', 'bun', 'vitest', 'jest', 'pytest', or 'go')",
                cwd
            )),
        }
    }
}

fn detect_js_framework(cwd: &Path) -> TestFramework {
    let has_bun_lock = cwd.join("bun.lock").is_file() || cwd.join("bun.lockb").is_file();
    let pkg_text = std::fs::read_to_string(cwd.join("package.json")).unwrap_or_default();
    let mentions_vitest = pkg_text.contains("\"vitest\"");
    let mentions_jest = pkg_text.contains("\"jest\"");

    match (has_bun_lock, mentions_vitest, mentions_jest) {
        (true, _, _) => TestFramework::Bun,
        (false, true, _) => TestFramework::Vitest,
        (false, false, true) => TestFramework::Jest,
        (false, false, false) => TestFramework::Bun,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestFailureRecord {
    pub test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTestSummary {
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub failures: Vec<TestFailureRecord>,
}

fn parse_number_before_word(segment: &str, word: &str) -> Option<u64> {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    tokens.windows(2).find_map(|pair| {
        let clean_word = pair[1].trim_matches(|c: char| !c.is_ascii_alphabetic());
        (clean_word.eq_ignore_ascii_case(word))
            .then(|| pair[0].trim_matches(|c: char| !c.is_ascii_digit()).parse::<u64>().ok())
            .flatten()
    })
}

pub fn parse_cargo_test_output(stdout: &str, stderr: &str) -> ParsedTestSummary {
    let mut passed = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;
    let mut panic_locations: HashMap<String, (Option<String>, String)> = HashMap::new();
    let mut current_failed_test: Option<String> = None;

    for line in stdout.lines().chain(stderr.lines()) {
        let trimmed = line.trim();

        // Capture `---- <test_name> stdout ----` blocks
        match trimmed
            .strip_prefix("---- ")
            .and_then(|rest| rest.strip_suffix(" stdout ----"))
        {
            Some(test_name) => {
                current_failed_test = Some(test_name.trim().to_string());
                continue;
            }
            None => {}
        }

        match current_failed_test.as_ref() {
            Some(active_name) => {
                let is_panic = trimmed.contains("panicked at ");
                match is_panic {
                    true => {
                        let after_at = trimmed
                            .split_once("panicked at ")
                            .map(|(_, r)| r.trim())
                            .unwrap_or(trimmed);
                        let loc = after_at
                            .split_whitespace()
                            .next()
                            .map(|tok| tok.trim_end_matches(':').to_string());
                        panic_locations.insert(active_name.clone(), (loc, trimmed.to_string()));
                    }
                    false => {
                        let has_entry = panic_locations.contains_key(active_name);
                        let is_assertion = !trimmed.is_empty() && !trimmed.starts_with("note:");
                        match (!has_entry, is_assertion) {
                            (true, true) => {
                                panic_locations
                                    .insert(active_name.clone(), (None, trimmed.to_string()));
                            }
                            _ => {}
                        }
                    }
                }
            }
            None => {}
        }

        match trimmed.starts_with("failures:") {
            true => current_failed_test = None,
            false => {}
        }

        match trimmed.starts_with("test result:") {
            true => {
                for part in trimmed.split(';') {
                    passed += parse_number_before_word(part, "passed").unwrap_or(0);
                    failed += parse_number_before_word(part, "failed").unwrap_or(0);
                    skipped += parse_number_before_word(part, "ignored").unwrap_or(0);
                }
            }
            false => {}
        }
    }

    let mut failures = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        match trimmed
            .strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
        {
            Some(test_name) => {
                let clean_name = test_name.trim().to_string();
                let (location, summary) = panic_locations
                    .remove(&clean_name)
                    .unwrap_or_else(|| (None, "Test assertion failed".to_string()));
                failures.push(TestFailureRecord {
                    test: clean_name,
                    location,
                    summary,
                });
            }
            None => {}
        }
    }

    ParsedTestSummary {
        passed,
        failed: failed.max(failures.len() as u64),
        skipped,
        failures,
    }
}

pub fn parse_bun_test_output(stdout: &str, stderr: &str) -> ParsedTestSummary {
    let mut passed = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;
    let mut failures = Vec::new();

    for line in stdout.lines().chain(stderr.lines()) {
        let trimmed = line.trim();
        match parse_number_before_word(trimmed, "pass") {
            Some(n) => passed = passed.max(n),
            None => {}
        }
        match parse_number_before_word(trimmed, "fail") {
            Some(n) => failed = failed.max(n),
            None => {}
        }
        match parse_number_before_word(trimmed, "skip") {
            Some(n) => skipped = skipped.max(n),
            None => {}
        }

        let fail_label = trimmed
            .strip_prefix("✗ ")
            .or_else(|| trimmed.strip_prefix("(fail) "));
        match fail_label {
            Some(rest) => {
                let test_name = rest
                    .rsplit_once(" [")
                    .map(|(n, _)| n.trim())
                    .unwrap_or_else(|| rest.trim());
                failures.push(TestFailureRecord {
                    test: test_name.to_string(),
                    location: None,
                    summary: "Test case failed".to_string(),
                });
            }
            None => {}
        }
    }

    ParsedTestSummary {
        passed,
        failed: failed.max(failures.len() as u64),
        skipped,
        failures,
    }
}

pub fn parse_go_json_output(stdout: &str) -> ParsedTestSummary {
    let mut passed = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;
    let mut test_logs: HashMap<String, Vec<String>> = HashMap::new();
    let mut failures = Vec::new();

    for line in stdout.lines() {
        let ev: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let action = ev.get("Action").and_then(Value::as_str).unwrap_or("");
        let test_name_opt = ev.get("Test").and_then(Value::as_str);
        let pkg = ev.get("Package").and_then(Value::as_str).unwrap_or("");

        match (action, test_name_opt) {
            ("output", Some(tname)) => {
                let key = format!("{pkg}::{tname}");
                let out_line = ev
                    .get("Output")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                match out_line.is_empty() || out_line.starts_with("=== RUN") || out_line.starts_with("--- FAIL") {
                    true => {}
                    false => {
                        test_logs
                            .entry(key)
                            .or_default()
                            .push(out_line.to_string());
                    }
                }
            }
            ("pass", Some(_)) => passed += 1,
            ("skip", Some(_)) => skipped += 1,
            ("fail", Some(tname)) => {
                failed += 1;
                let key = format!("{pkg}::{tname}");
                let lines = test_logs.remove(&key).unwrap_or_default();
                let summary = lines
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Go test failed".to_string());
                failures.push(TestFailureRecord {
                    test: key,
                    location: None,
                    summary,
                });
            }
            _ => {}
        }
    }

    ParsedTestSummary {
        passed,
        failed,
        skipped,
        failures,
    }
}

pub fn parse_jest_vitest_json(stdout: &str) -> Option<ParsedTestSummary> {
    let start = stdout.find('{')?;
    let parsed: Value = serde_json::from_str(stdout[start..].trim()).ok()?;
    let passed = parsed.get("numPassedTests")?.as_u64().unwrap_or(0);
    let failed = parsed.get("numFailedTests").and_then(Value::as_u64).unwrap_or(0);
    let skipped = parsed
        .get("numPendingTests")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let mut failures = Vec::new();
    match parsed.get("testResults").and_then(Value::as_array) {
        Some(suites) => {
            for suite in suites {
                let file_name = suite
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                match suite.get("assertionResults").and_then(Value::as_array) {
                    Some(assertions) => {
                        for item in assertions {
                            let is_fail =
                                item.get("status").and_then(Value::as_str) == Some("failed");
                            match is_fail {
                                true => {
                                    let title = item
                                        .get("fullName")
                                        .or_else(|| item.get("title"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("unnamed test")
                                        .to_string();
                                    let msg = item
                                        .get("failureMessages")
                                        .and_then(Value::as_array)
                                        .and_then(|arr| arr.first())
                                        .and_then(Value::as_str)
                                        .and_then(|s| s.lines().next())
                                        .unwrap_or("Assertion failed")
                                        .trim()
                                        .to_string();
                                    failures.push(TestFailureRecord {
                                        test: title,
                                        location: file_name.clone(),
                                        summary: msg,
                                    });
                                }
                                false => {}
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        None => {}
    }

    Some(ParsedTestSummary {
        passed,
        failed: failed.max(failures.len() as u64),
        skipped,
        failures,
    })
}

pub fn parse_pytest_output(stdout: &str, stderr: &str) -> ParsedTestSummary {
    let mut passed = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;
    let mut failures = Vec::new();

    for line in stdout.lines().chain(stderr.lines()) {
        let trimmed = line.trim();
        match trimmed.strip_prefix("FAILED ") {
            Some(rest) => {
                let (target, reason) = rest
                    .split_once(" - ")
                    .map(|(l, r)| (l.trim(), r.trim()))
                    .unwrap_or((rest.trim(), "Pytest failure"));
                let loc = target.split("::").next().map(str::to_string);
                failures.push(TestFailureRecord {
                    test: target.to_string(),
                    location: loc,
                    summary: reason.to_string(),
                });
            }
            None => {}
        }

        for part in trimmed.trim_matches('=').split(',') {
            match parse_number_before_word(part, "passed") {
                Some(n) => passed = passed.max(n),
                None => {}
            }
            match parse_number_before_word(part, "failed") {
                Some(n) => failed = failed.max(n),
                None => {}
            }
            match parse_number_before_word(part, "skipped") {
                Some(n) => skipped = skipped.max(n),
                None => {}
            }
        }
    }

    ParsedTestSummary {
        passed,
        failed: failed.max(failures.len() as u64),
        skipped,
        failures,
    }
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(max_lines);
    all[start..].join("\n")
}

#[async_trait::async_trait]
impl Tool for TestRunnerTool {
    fn name(&self) -> &str {
        "test_runner"
    }

    fn description(&self) -> &str {
        "Execute unit/integration tests across Rust (cargo), Bun, Vitest, Jest, Python (pytest), or Go and return normalized JSON pass/fail counts and structured failure diagnostics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "framework": {
                    "type": "string",
                    "enum": ["auto", "cargo", "bun", "vitest", "jest", "pytest", "go"],
                    "description": "Test runner to invoke (defaults to 'auto', inferred from target/cwd)."
                },
                "filter": {
                    "type": "string",
                    "description": "Optional test name substring or pattern to run a focused subset."
                },
                "target": {
                    "type": "string",
                    "description": "Optional test file path, package name (for cargo), or Go package path."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory in which to execute tests."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional timeout in seconds (10..=900, default 180)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let cwd_path = arguments
            .get("cwd")
            .or_else(|| arguments.get("dir"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(crate::config::resolve_path)
            .unwrap_or_else(|| crate::config::loader::resolve_path("."));

        cwd_path
            .exists()
            .then_some(())
            .ok_or_else(|| anyhow!("Test working directory does not exist: {:?}", cwd_path))?;

        let target_opt = arguments
            .get("target")
            .or_else(|| arguments.get("file"))
            .or_else(|| arguments.get("package"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('-'));

        let filter_opt = arguments
            .get("filter")
            .or_else(|| arguments.get("test"))
            .or_else(|| arguments.get("pattern"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('-'));

        let fw_raw = arguments
            .get("framework")
            .or_else(|| arguments.get("runner"))
            .and_then(Value::as_str)
            .unwrap_or("auto");

        let framework = match fw_raw.trim().eq_ignore_ascii_case("auto") {
            true => TestFramework::detect(&cwd_path, target_opt)?,
            false => TestFramework::from_explicit(fw_raw)
                .ok_or_else(|| anyhow!("Unsupported test framework '{}'", fw_raw))?,
        };

        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(180)
            .clamp(10, 900);

        let mut cmd = match framework {
            TestFramework::Cargo => {
                let mut c = Command::new("cargo");
                c.arg("test");
                let _ = target_opt.map(|pkg| c.args(["-p", pkg]));
                let _ = filter_opt.map(|flt| c.arg(flt));
                c.args(["--", "--color=never"]);
                c
            }
            TestFramework::Bun => {
                let mut c = Command::new("bun");
                c.arg("test");
                let _ = target_opt.map(|t| c.arg(t));
                let _ = filter_opt.map(|flt| c.args(["--test-name-pattern", flt]));
                c
            }
            TestFramework::Vitest => {
                let local_bin = cwd_path.join("node_modules").join(".bin").join("vitest");
                let mut c = match local_bin.is_file() {
                    true => Command::new(local_bin),
                    false => {
                        let mut npx = Command::new("npx");
                        npx.arg("vitest");
                        npx
                    }
                };
                c.args(["run", "--reporter=json"]);
                let _ = target_opt.map(|t| c.arg(t));
                let _ = filter_opt.map(|flt| c.args(["-t", flt]));
                c
            }
            TestFramework::Jest => {
                let local_bin = cwd_path.join("node_modules").join(".bin").join("jest");
                let mut c = match local_bin.is_file() {
                    true => Command::new(local_bin),
                    false => {
                        let mut npx = Command::new("npx");
                        npx.arg("jest");
                        npx
                    }
                };
                c.arg("--json");
                let _ = target_opt.map(|t| c.arg(t));
                let _ = filter_opt.map(|flt| c.args(["-t", flt]));
                c
            }
            TestFramework::Pytest => {
                let mut c = Command::new("python3");
                c.args(["-m", "pytest", "-q", "--tb=short", "--color=no"]);
                let _ = target_opt.map(|t| c.arg(t));
                let _ = filter_opt.map(|flt| c.args(["-k", flt]));
                c
            }
            TestFramework::Go => {
                let mut c = Command::new("go");
                c.args(["test", "-json"]);
                c.arg(target_opt.unwrap_or("./..."));
                let _ = filter_opt.map(|flt| c.args(["-run", flt]));
                c
            }
        };

        cmd.current_dir(&cwd_path);
        cmd.kill_on_drop(true);

        let started = std::time::Instant::now();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| anyhow!("Test execution timed out after {}s", timeout_secs))??;
        let duration_ms = started.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let summary = match framework {
            TestFramework::Cargo => parse_cargo_test_output(&stdout, &stderr),
            TestFramework::Bun => parse_bun_test_output(&stdout, &stderr),
            TestFramework::Go => parse_go_json_output(&stdout),
            TestFramework::Vitest | TestFramework::Jest => parse_jest_vitest_json(&stdout)
                .unwrap_or_else(|| parse_bun_test_output(&stdout, &stderr)),
            TestFramework::Pytest => parse_pytest_output(&stdout, &stderr),
        };

        let status = match (output.status.success(), summary.failed > 0) {
            (true, false) => "passed",
            (_, true) => "failed",
            (false, false) => "error",
        };

        Ok(json!({
            "status": status,
            "framework": framework.as_str(),
            "passed": summary.passed,
            "failed": summary.failed,
            "skipped": summary.skipped,
            "duration_ms": duration_ms,
            "exit_code": output.status.code().unwrap_or(-1),
            "failures": summary.failures,
            "stdout_tail": tail_lines(&stdout, 30),
            "stderr_tail": tail_lines(&stderr, 30),
        }))
    }
}

#[cfg(test)]
#[path = "test_runner_tests.rs"]
mod tests;
