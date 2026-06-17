# Skill: OpenZ Config Access Workaround
## Goal
Ensure reliable access to configuration files when standard shell commands fail.
## Description
In certain environments, executing shell commands like `cat` via `exec_command` to read system files (e.g., `~/.openz/config.json`) may result in a "Bad system call (core dumped)" error due to environment restrictions or security sandboxing.
## Rules/Guidelines
- **Avoid `exec_command` for simple file reads**: If a shell command to read a file fails with a core dump or system call error, do not retry the same command.
- **Use Dedicated File Tools**: Immediately switch to a dedicated file system tool such as `read_file` to access the content.
- **Verification**: If `read_file` also fails, verify the file's existence using a directory listing tool before attempting further reads.
- **Handle Trivial Inputs**: If the user's input is trivial (e.g., "hii"), respond with a polite but concise instruction to provide a task or question. Avoid engaging in non-task-related conversation.
## Examples
- **Problem**: `exec_command(command="cat ~/.openz/config.json")` returns `{"status_code":159,"stdout":"","stderr":"Bad system call (core dumped)\n"}`.
- **Workaround**: Use `read_file(path="~/.openz/config.json")` instead.
- **Problem**: User input is "hii".
- **Workaround**: Respond with: "What do you need?"