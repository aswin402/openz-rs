# Skill: Agent Orchestration and Execution Instincts

This skill provides guidelines and operational instincts for coordinating the parent agent loop with specialized subagents, optimizing token context, ensuring execution security, and maintaining rigorous verification loops within the OpenZ framework.

---

## 1. Subagent Role & Tool Mapping

When executing complex multi-step tasks, the parent agent loop should delegate sub-tasks to the most qualified specialized subagent using their specific tool calls or `delegate_task`.

| Subagent Name | Specialized Role | Primary Tool Mapping |
| :--- | :--- | :--- |
| **`planner`** | Decomposes goals, manages milestones and progress. | *High-level task coordination.* |
| **`researcher`** | Conducts web searches and reads external docs. | `web_search`, `web_fetch`, `doc_reader` |
| **`ast_searcher`** | Structural code search and architecture analysis. | `ast_grep`, `code_outline`, `grep_search` |
| **`git_ops_agent`** | Handles repository status, staging, committing, and worktree diffs. | `git_manager` |
| **`database_specialist`** | Queries database tables, reads schemas, and updates records. | `db_inspector` |
| **`browser_operator`** | Web browser automation, scraping, E2E testing, visual QA. | `gsd_browser`, `obscura`, `crawl` |
| **`dependency_manager`** | Scaffolds stack components and installs dependencies. | `onpkg` |
| **`reviewer`** / **`code_auditor`** | Inspects code changes for bugs, security holes, and style logic. | *Source file diff reviews.* |
| **`debugger`** | Diagnoses run errors, reproduces bugs, and resolves loops. | `exec_command`, *AST/grep search* |
| **`test_engineer`** | Designs QA test suites, writes unit/integration/E2E tests. | `cargo_manager` (for Rust), `exec_command` |

---

## 2. Token & Context Optimization

To maximize speed and avoid model rate limits, agents must follow strict context slimming guidelines:
* **Avoid Bulk Reads**: Do not read entire source files unless they are small (<100 lines). When calling `view_file`, specify exact `StartLine` and `EndLine` parameters.
* **Targeted Search**: When using `grep_search`, always supply specific `Includes` filters (e.g., `*.rs`, `*.json`) instead of querying the entire directory hierarchy blindly.
* **Truncated Output Handling**: When tool outputs exceed 4,000 characters, refer to the local reference file saved under `~/.openz/tool_outputs/` rather than requesting the agent loop to repeat the output in the session history.

---

## 3. Worktree Isolation & Simulation Spaces

By default, OpenZ runs specialized subagents in isolated `git worktrees` and creates temporary DB branches:
* **Simulation Safety**: Agents can freely write, patch, test, and run build commands in the isolated workspace without affecting the user's active branch.
* **Synchronization Guard**: If the subagent succeeds, changes are automatically synced back. If it fails or crashes, the workspace is safely discarded, and database modifications are rolled back.
* **No Untracked Pollution**: Never commit temporary files or build artifacts to version control.

---

## 4. Verification Loops & Code Hygiene

Never declare a code modification complete without proving its correctness:
* **Compile First**: Immediately after editing a source file, trigger `cargo_manager` with the `check` or `build` action to ensure no syntax or type errors were introduced.
* **Run Tests**: Run unit and integration tests using the `test` action in `cargo_manager` or standard command tools.
* **Iterative Fixes**: If compiler or test output fails, analyze the stdout/stderr, isolate the offending line numbers, patch the file, and repeat the check.

---

## 5. SecurityGuard & Network Audits

The parent agent intercepts potentially destructive or privileged actions:
* **Privileged Actions**: Writing system files, executing unverified shell commands, or invoking raw socket connections will trigger the `SecurityGuard` popup for user approval.
* **Credential Protection**: Never write raw API keys or database credentials to code files. Reference environment variables or standard config files.
