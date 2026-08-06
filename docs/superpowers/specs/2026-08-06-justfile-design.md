# Justfile Design Spec

## 1. Problem Statement
The user experiences system lag when running global compilation commands (`cargo check`, `cargo test`, `cargo build`) on their laptop, as these trigger resource-intensive multi-threaded compilation of the entire Rust workspace at once.

## 2. Goals
- Capping resource concurrency to at most 2 parallel jobs (`-j 2`) for local dev tasks.
- Providing standard recipes to build/install globally using the balanced update mode (`localupdate.sh --balanced`).
- Providing standard recipes to compile, check, test, clippy, format, and list individual packages.

## 3. Justfile Structure
The `justfile` will be created at the root of the project `/home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/justfile`. It will contain the following recipes:
- `default`: Lists all recipes.
- `build`: Runs `bash localupdate.sh --balanced`.
- `compile`: Compiles a specific package/crate with `-j 2` concurrency.
- `list-packages`: Echoes all crates/packages in the workspace.
- `check`: Runs `cargo check` on a specific package with `-j 2`.
- `test`: Runs `cargo test` on a specific package with `-j 2`.
- `test-one`: Runs a specific test by name in a package with `-j 2`.
- `clippy`: Runs `cargo clippy` on a specific package with `-j 2`.
- `format`: Runs `cargo fmt`.
- `clean`: Runs `cargo clean`.
