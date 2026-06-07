# Original Nanobot Skills Reference 🐍

This document preserves details about the original Python skills discovery and packaging.

---

## 1. Skill Discovery Mechanism

* **Built-in Skills:** Discovered automatically at startup using Python's `pkgutil` scanning on the `nanobot.skills` package.
* **ClawHub Integrations:** Integrates a plugin client that can search, download, and execute public agent skills from `clawhub.ai`.
* **Dynamic Loading:** A skill is composed of a YAML frontmatter descriptor (`SKILL.md`) and supporting bash scripts/executable files.

---

## 2. Dynamic Tool Registry

1. **Parameters schema:** Parses the JSON Schema out of the skill description block.
2. **Execution:** Executes skills by calling their respective scripts or commands in a subprocess.
3. **Workspace Isolation:** Restricts skill script file reads and writes inside the configured workspace path.
