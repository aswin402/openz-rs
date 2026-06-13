---
name: notes
description: "AI Agent Skill for Notes Indexing — explains how to scan and semantically index Markdown/Obsidian note files for long-term search."
metadata:
  version: 1.0.0
---

# notes Indexing Skill 🧠📂

This skill describes how to index local note folders (like Obsidian vaults) into semantic memory to build a self-hosted "Second Brain" inside OpenZ.

## 1. Tool Usage

Use the `index_notes` tool to parse and index local documents:

```json
{
  "path": "/path/to/obsidian/vault"
}
```
* If `path` is omitted, it defaults to the current active project directory (`.`).

---

## 2. Block-Level Indexing Rules
1. **Header Segmentation**: The parser automatically splits markdown documents into sections based on headers (`#`, `##`, `###`).
2. **descriptive Names**: Each section is stored with a header path in the format `[Note Segment] File.md > HeaderName: Content...` to preserve surrounding context.
3. **Embeddings**: Computes AllMiniLML6V2 vector embeddings and appends entries to `~/.openz/shared_memory.json` under the tag `notes`.
4. **Retrieval**: Once indexed, you can search these note segments using the standard `recall_memory` tool with a semantic query.
