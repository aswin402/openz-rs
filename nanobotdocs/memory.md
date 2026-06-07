# Original Nanobot Memory Consolidation Reference 🐍

This document preserves details about the original Python memory systems of `nanobot`.

---

## 1. Memory Systems Overview

`nanobot` keeps two tiers of context memory:
1. **Short-Term Context:** Replayed from the active session file (`max_messages` threshold).
2. **Long-Term Memory ("Dream" consolidations):** Runs as a periodic background task to compress historical logs into a consolidated memory prompt.

---

## 2. Dream Memory Consolidation

* **Interval:** Every 2 hours (configurable via `dream.interval_h` or `dream.cron`).
* **Workflow:**
  1. The agent reads the conversation history.
  2. If the message list exceeds the consolidation ratio, a background thread starts the consolidation task.
  3. The agent calls the LLM with a template instructing it to summarize what was learned, key facts, decisions, and outcomes into a compressed Markdown list.
  4. The generated summary is saved as a persistent memory prompt in the session metadata under `memory_prompt`.
  5. The consolidated messages are pruned from the active message history and stored in an archive, keeping the active context window clean and small.
