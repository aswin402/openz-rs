# Original Nanobot LLM Providers Reference 🐍

This document preserves details about the original Python LLM providers supported by `nanobot`.

---

## 1. Supported Providers

* **OpenAI Compat (`openai_compat_provider.py`)**: The primary fallback provider, wrapping standard OpenAI chat completions, supporting custom headers, body fields, and reasoning tokens.
* **Anthropic (`anthropic_provider.py`)**: Supports Claude models, Anthropic system prompts, prompt caching via headers, and extended thinking blocks.
* **Azure OpenAI (`azure_openai_provider.py`)**: Custom endpoints routing queries through deployment keys.
* **AWS Bedrock (`bedrock_provider.py`)**: Uses `boto3` to converse with Bedrock models (Claude, Llama, Mistral) via IAM credentials.
* **Google Vertex / Gemini (`base.py` / `openai_compat_provider.py`)**: Connects to Google's API.
* **OpenAI Codex (`openai_codex_provider.py`)**: Codex OAuth endpoint integrations.
* **GitHub Copilot (`github_copilot_provider.py`)**: Intercepts Copilot token endpoints to run GPT-4o models.
* **Local / Gateways:**
  * **Ollama (`base.py`)**: Custom endpoint routing for local models.
  * **vLLM / LM Studio / Ollama**: Custom base URL configurations.
  * **Groq / DeepSeek / VolcEngine / Skywork / Zhipu / Dashscope**: Predefined API base URLs and models.

---

## 2. API Schema Handlers

In the original Python implementation:
1. **API Type:** Supported `chat_completions` (OpenAI format) and `responses` (JSON schema payloads).
2. **Thinking / Reasoning Extraction:** For models like DeepSeek-R1 or Kimi, the provider extracts `<think>` blocks out of the stream content and sends them as `reasoning_content` to the client.
3. **Provider Fallbacks:** Configures `fallback_models` so if the primary LLM rate-limits (HTTP 429) or returns server errors (HTTP 500), the runner automatically swaps to the next fallback candidate.
