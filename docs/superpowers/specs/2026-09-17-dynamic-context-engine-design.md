# OpenZ Adaptive Dynamic Context Engine Specification

**Document:** `docs/superpowers/specs/2026-09-17-dynamic-context-engine-design.md`  
**Date:** September 17, 2026  
**Status:** Approved  
**Inspirations:** Nous Research Hermes Agent, Pi Agent (@earendil-works), Prime Agent (Prime Intellect RLM), Academic Papers (Verma 2026, ACM 2026, ACON 2026, Liu et al. 2023).

---

## 1. Executive Summary & Problem Statement

### 1.1 The Problem
When users run OpenZ with large-context models (such as MiniMax 204.8k, Claude 3.5 Sonnet 200k, Gemini 1M-2M, or GPT-4o 128k), the runtime artificially limits context and triggers premature truncation/compaction around **16,000 tokens**.
Four specific root causes were identified in OpenZ's codebase:
1. `resolve_prompt_budget` in `src/agent/agent_loop/build.rs:298` clamped prompt character allocation to `computed.clamp(8000, 64000)`. 64,000 chars / 4 chars per token = **16,000 tokens**.
2. Default context fallback in `build.rs:153` was `context_limit.unwrap_or(32000)` (chars) $\approx$ **8,000 tokens**.
3. Tool output limit in `src/agent/agent_loop/transcript.rs:86` defaulted to **4,000 chars** (~1,000 tokens), aggressively mutilating JSON arrays and logs even when 190k tokens remained free.
4. Compaction in `src/agent/agent_loop/compact.rs` relied on a rigid message count (`len > max_messages` / 120) instead of actual token pressure.

### 1.2 The Goal
Architect a **Zero-Hardcoding, Proportional Dynamic Context Engine** that:
- Automatically discovers and respects the active model's true context window.
- Budgets prompt overhead, tool output limits, and compaction triggers as dynamic percentages of the context window.
- Employs Hermes Agent's pre-compression hook to protect memories before lossy summarization.
- Employs Pi Agent's proportional knobs (`reserveTokens`, `keepRecentTokens`).
- Avoids destructive JSON pruning.

---

## 2. Architecture & Design

### 2.1 Dynamic Context Registry (`src/providers/context_registry.rs`)

The `DynamicContextRegistry` serves as the single source of truth for model context limits and budget calculations across CLI, TUI, channels, and the agent loop.

#### 4-Tier Resolution Hierarchy:
1. **Tier 1: Explicit User Configuration**:
   - `config.agents.defaults.context_limit` (token count) if provided in `~/.openz/config.json`.
2. **Tier 2: External Models JSON Catalog**:
   - Loads from `~/.openz/models.json` (or `$OPENZ_CONFIG_DIR/models.json`) if present. Enables users and community packs to add new models and context windows without code changes or recompilation.
3. **Tier 3: Built-In Curated Model Registry**:
   - Pattern-matched database of standard families:
     - MiniMax (`minimax-m2.7`, `minimax`): 204,800 tokens
     - Claude (`claude-3-5`, `claude-3`, `claude`): 200,000 tokens
     - Gemini Pro (`gemini-1.5-pro`, `gemini-2.5-pro`): 2,097,152 tokens
     - Gemini Flash / other: 1,048,576 tokens
     - OpenAI O-series (`o1`, `o3-mini`): 200,000 tokens
     - OpenAI GPT-4o (`gpt-4o`, `gpt-4.5`, `gpt-4`): 128,000 tokens
     - DeepSeek (`deepseek-v4`): 1,000,000 tokens; (`deepseek-v3`, `deepseek-r1`, `deepseek-chat`): 128,000 tokens (or 64,000 based on variant)
     - Llama 3.1 / 3.2 / 3.3: 128,000 tokens; Llama 3 (original): 8,192 tokens
     - Qwen 2.5: 128,000 tokens
     - Mistral Large: 128,000 tokens
4. **Tier 4: Safe Dynamic Fallback**:
   - Default 128,000 tokens for unknown models (rather than 8,000 tokens).

### 2.2 Proportional Token Budgeting Engine

All budgets are calculated proportionally from the resolved context window ($W$ tokens) with configurable ratios in `AgentDefaults`:

$$\text{Prompt Budget Chars} = W \times \text{prompt\_budget\_ratio} \times 4.0$$
$$\text{Tool Output Limit Chars} = W \times \text{tool\_output\_ratio} \times 4.0$$
$$\text{Compaction Threshold Tokens} = W \times \text{compaction\_threshold\_ratio}$$

#### Default Ratios:
- **`prompt_budget_ratio`**: `0.25` (25% of window).
  - On a 204.8k model: 51,200 tokens $\approx$ **204,800 chars** (vs old 64,000 char clamp).
  - On an 8k model: 2,000 tokens $\approx$ 8,000 chars.
- **`tool_output_ratio`**: `0.08` (8% of window).
  - On a 204.8k model: 16,384 tokens $\approx$ **65,536 chars** (vs old 4,000 char clamp).
  - On an 8k model: 640 tokens $\approx$ 2,560 chars.
- **`compaction_threshold_ratio`**: `0.80` (80% of window).
  - Compaction triggers when estimated token usage exceeds 80% of capacity.
- **`keep_recent_ratio`**: `0.20` (20% of window).
  - Keeps recent turns verbatim during compaction.

### 2.3 Hermes Pre-Compression Hook

Before `compact.rs` generates a summary and drops older messages:
1. Fire pre-compression extraction:
   - Identify decisions, user preferences, and file modification paths from the messages about to be pruned.
   - Insert identified items into session metadata (`pinned_memory` / cross-session memory) or graph memory.
2. Generate structured rolling summary.
3. Slice history safely starting at a `user` turn while preserving the uncompressed recent buffer.

### 2.4 Structural Tool Compactor Hardening

In `src/agent/context_compactor.rs`:
- Stop discarding elements $2..N$ of JSON arrays when compressing JSON.
- If a JSON array exceeds the dynamic budget, preserve head and tail elements and indicate the number of omitted middle elements, preserving schema keys.
- Preserve log error stacks and backtraces without arbitrary 1,000-char truncation.

---

## 3. Configuration Additions (`src/config/schema.rs`)

Add optional fields to `AgentDefaults`:
```rust
#[serde(default, alias = "prompt_budget_ratio")]
pub prompt_budget_ratio: Option<f64>,
#[serde(default, alias = "tool_output_ratio")]
pub tool_output_ratio: Option<f64>,
#[serde(default, alias = "compaction_threshold_ratio")]
pub compaction_threshold_ratio: Option<f64>,
#[serde(default, alias = "keep_recent_ratio")]
pub keep_recent_ratio: Option<f64>,
```

---

## 4. Verification & Testing Strategy

1. **Unit Tests (`src/providers/context_registry_tests.rs`)**:
   - Verify 4-tier resolution hierarchy.
   - Verify proportional budget calculations for 8k, 128k, 204.8k, and 1M models.
   - Verify `models.json` dynamic file overrides.
   - Verify compaction triggers under token pressure.
2. **Agent Loop Tests**:
   - Verify `build.rs` allocates proportional character budget without 64k clamp.
   - Verify `transcript.rs` allows large tool outputs up to dynamic limit.
   - Verify `compact.rs` pre-compression memory preservation.
3. **Project Invariants**:
   - Maintain exactly 260 registered native tools.
   - Zero compiler/clippy warnings.
   - Low-resource job capping (`-j 1` for test/check, `-j 2` for build).
