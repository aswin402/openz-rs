# Active LLM Provider Models Directory

This directory details all active, verified models retrieved directly from each provider's `/models` API endpoint. They are categorized by coding, reasoning/intelligence, vision/multimodal, and fast/efficient tiers.

> [!NOTE]
> OpenCode Zen and Hugging Face were explicitly excluded from this verification run per user instructions.

---

## 1. Google AI Studio
*   **Reasoning & General Intelligence:**
    *   `models/gemini-2.5-pro` (Best for complex reasoning and planning)
    *   `models/gemini-3.1-pro-preview` (Next-gen reasoning preview)
    *   `models/gemini-3-pro-preview` (Advanced multimodal reasoning preview)
    *   `models/gemini-pro-latest`
    *   `models/gemini-2.5-computer-use-preview-10-2025`
    *   `models/deep-research-preview-04-2026`
    *   `models/deep-research-pro-preview-12-2025`
    *   `models/deep-research-max-preview-04-2026`
*   **Vision & Multimodal:**
    *   `models/gemini-2.0-flash`
    *   `models/gemini-2.5-flash` (Fast, high-fidelity multimodal)
    *   `models/gemini-2.5-flash-image`
    *   `models/gemini-3.1-flash-image`
    *   `models/gemini-3.1-flash-image-preview`
    *   `models/gemini-3-pro-image`
    *   `models/gemini-3-pro-image-preview`
*   **Fast & Efficient:**
    *   `models/gemini-2.0-flash-lite`
    *   `models/gemini-2.5-flash-lite`
    *   `models/gemini-3-flash-preview`
    *   `models/gemini-3.1-flash-lite`
    *   `models/gemini-3.1-flash-lite-preview`
    *   `models/gemini-3.1-flash-live-preview`
    *   `models/gemini-3.5-flash`
    *   `models/gemini-flash-lite-latest`
*   **Specialized, Open Models & Media:**
    *   `models/gemma-4-26b-a4b-it`
    *   `models/gemma-4-31b-it`
    *   `models/nano-banana-pro-preview`
    *   `models/imagen-4.0-fast-generate-001`
    *   `models/imagen-4.0-generate-001`
    *   `models/imagen-4.0-ultra-generate-001`
    *   `models/veo-2.0-generate-001`
    *   `models/veo-3.0-generate-001`
    *   `models/veo-3.1-generate-preview`

## 2. Mistral AI
*   **Coding & Development:**
    *   `codestral-latest` (Highly optimized for software engineering)
    *   `codestral-2508`
    *   `mistral-code-latest`
    *   `mistral-code-agent-latest`
    *   `mistral-code-fim-latest`
*   **Reasoning & Strongest:**
    *   `mistral-large-latest` (State-of-the-art multilingual model)
    *   `mistral-large-2512`
    *   `mistral-medium-latest`
    *   `mistral-medium-2604`
    *   `mistral-medium-3`
    *   `mistral-medium-3.5`
*   **Fast & Efficient:**
    *   `mistral-small-latest`
    *   `mistral-small-2603`
    *   `ministral-8b-latest`
    *   `ministral-8b-2512`
    *   `ministral-14b-latest`
    *   `ministral-14b-2512`
    *   `ministral-3b-latest`
    *   `mistral-tiny-latest`
*   **Specialized & Audio/Voice:**
    *   `mistral-embed`
    *   `mistral-ocr-latest`
    *   `voxtral-mini-latest`
    *   `voxtral-small-latest`

## 3. Groq
*   **Reasoning & Strongest:**
    *   `llama-3.3-70b-versatile` (Very fast 70B reasoning model)
    *   `meta-llama/llama-4-scout-17b-16e-instruct` (Next-gen Llama 4 preview)
    *   `openai/gpt-oss-120b` (Large open-source model)
*   **Fast & Efficient:**
    *   `llama-3.1-8b-instant`
    *   `groq/compound-mini`
    *   `groq/compound`
    *   `qwen/qwen3-32b`
*   **Specialized & Audio:**
    *   `meta-llama/llama-prompt-guard-2-86m`
    *   `whisper-large-v3`
    *   `whisper-large-v3-turbo`

## 4. NVIDIA NIM
*   **Coding & Development:**
    *   `deepseek-ai/deepseek-coder-6.7b-instruct`
    *   `ibm/granite-34b-code-instruct`
    *   `ibm/granite-8b-code-instruct`
    *   `mistralai/codestral-22b-instruct-v0.1`
    *   `nvidia/nv-embedcode-7b-v1`
*   **Reasoning & Strongest:**
    *   `nvidia/cosmos-reason2-8b` (Specialized reasoning model)
    *   `nvidia/llama-3.1-nemotron-70b-instruct`
    *   `nvidia/llama-3.1-nemotron-ultra-253b-v1`
    *   `nvidia/llama-3.3-nemotron-super-49b-v1.5`
    *   `mistralai/mistral-large-3-675b-instruct-2512`
    *   `meta/llama-3.3-70b-instruct`
    *   `meta/llama-4-maverick-17b-128e-instruct`
    *   `deepseek-ai/deepseek-v4-pro`
    *   `qwen/qwen3.5-397b-a17b`
    *   `openai/gpt-oss-120b`
*   **Vision & Multimodal:**
    *   `meta/llama-3.2-90b-vision-instruct`
    *   `meta/llama-3.2-11b-vision-instruct`
    *   `microsoft/phi-3-vision-128k-instruct`
    *   `microsoft/phi-4-multimodal-instruct`
    *   `nvidia/nemotron-nano-12b-v2-vl`
*   **Fast & Efficient:**
    *   `nvidia/llama-3.1-nemotron-nano-8b-v1`
    *   `meta/llama-3.2-3b-instruct`
    *   `google/gemma-3-12b-it`
    *   `google/gemma-3-4b-it`
    *   `deepseek-ai/deepseek-v4-flash`

## 5. Z.ai (Zhipu GLM)
*   **Reasoning & Strongest:**
    *   `glm-5.1` (Latest flagship Chinese/English reasoning model)
    *   `glm-5`
    *   `glm-4.5`
*   **Fast & Efficient:**
    *   `glm-5-turbo`
    *   `glm-4.7`
    *   `glm-4.6`

## 6. Cerebras
*   **Verified Models:**
    *   `gpt-oss-120b` (Large parameter model running at ultra-high speed)
    *   `zai-glm-4.7`

## 7. SambaNova
*   **Verified Models:**
    *   `DeepSeek-V3.1`
    *   `DeepSeek-V3.2`
    *   `Meta-Llama-3.3-70B-Instruct`
    *   `MiniMax-M2.7`
    *   `gemma-4-31B-it`
    *   `gpt-oss-120b`

## 8. Cohere
*   **Reasoning & General Intelligence:**
    *   `command-r-plus-08-2024`
    *   `command-a-plus-05-2026`
    *   `c4ai-aya-expanse-32b`
*   **Fast & Efficient:**
    *   `command-r7b-12-2024`
    *   `command-a-03-2025`
*   **Embeddings & Multimodal:**
    *   `embed-english-v3.0`
    *   `embed-multilingual-v3.0`
    *   `c4ai-aya-vision-32b`

## 9. OpenRouter
*   **Reasoning & Strongest Tiers:**
    *   `deepseek/deepseek-r1`
    *   `anthropic/claude-opus-4.8`
    *   `openai/gpt-5`
    *   `openai/o3-mini`
    *   `x-ai/grok-4.20`
    *   `google/gemini-3.1-pro-preview`
    *   `google/gemini-2.5-pro`
*   **Fast / Free Tiers:**
    *   `google/gemma-4-31b-it:free`
    *   `meta-llama/llama-3.3-70b-instruct:free`
    *   `liquid/lfm-2.5-1.2b-thinking:free`

## 10. Ollama (Local)
*   **Active Models:**
    *   `gemma4:12b`

---

## 11. Excluded Providers
*   **OpenCode Zen:** Not checked/queried per user request.
*   **Hugging Face:** Not checked/queried per user request.

## 12. Inactive / Not Configured
*   **Anthropic:** Verification failed or not configured.
*   **DeepSeek:** Verification failed or not configured.
*   **Minimax:** Verification failed or not configured.
*   **LLM7:** Verification failed or not configured.
