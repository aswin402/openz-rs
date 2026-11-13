/**
 * Runtime Metrics, BPE Token Heuristics, Model Context Windows & Policy Derivations (Zero-Regex).
 */

function isAsciiWhitespace(code: number): boolean {
  return code === 32 || code === 9 || code === 10 || code === 13;
}

function isAsciiAlphaNum(code: number): boolean {
  return (
    (code >= 48 && code <= 57) ||
    (code >= 65 && code <= 90) ||
    (code >= 97 && code <= 122) ||
    code === 95
  );
}

/**
 * BPE-inspired token estimator that distinguishes identifier words, punctuation/code
 * operators, and multibyte characters instead of a static `chars / 4` constant.
 */
export function estimateTextTokens(text: string): number {
  if (!text) return 0;
  let tokens = 0;
  let wordLen = 0;

  for (let i = 0; i < text.length; i += 1) {
    const code = text.charCodeAt(i);
    if (isAsciiAlphaNum(code)) {
      wordLen += 1;
      continue;
    }

    if (wordLen > 0) {
      tokens += wordLen <= 4 ? 1 : Math.ceil(wordLen / 4.2);
      wordLen = 0;
    }

    if (isAsciiWhitespace(code)) {
      continue;
    }

    // Multibyte / CJK / emoji characters average ~1.5 BPE tokens; ASCII operators ~1 token
    tokens += code > 127 ? 1.5 : 0.75;
  }

  if (wordLen > 0) {
    tokens += wordLen <= 4 ? 1 : Math.ceil(wordLen / 4.2);
  }

  return Math.max(1, Math.round(tokens));
}

export interface TokenCountableMessage {
  content: string;
  reasoningContent?: string;
  toolCalls?: ReadonlyArray<{ name: string; output?: string }>;
}

export function estimateConversationTokens(
  messages: readonly TokenCountableMessage[],
  draftInput = '',
): number {
  let total = draftInput ? estimateTextTokens(draftInput) : 0;
  for (const msg of messages) {
    total += 4; // ChatML turn framing overhead
    total += estimateTextTokens(msg.content);
    if (msg.reasoningContent) {
      total += estimateTextTokens(msg.reasoningContent);
    }
    if (msg.toolCalls) {
      for (const tc of msg.toolCalls) {
        total += 6 + estimateTextTokens(tc.name);
        if (tc.output) {
          total += estimateTextTokens(tc.output);
        }
      }
    }
  }
  return total;
}

function stripProviderPrefix(model: string): string {
  const trimmed = model.trim().toLowerCase();
  const doubleColon = trimmed.indexOf('::');
  const afterColon = doubleColon >= 0 ? trimmed.slice(doubleColon + 2) : trimmed;
  const slash = afterColon.indexOf('/');
  return slash >= 0 ? afterColon.slice(slash + 1) : afterColon;
}

/**
 * Resolves the effective context window (in tokens) from the active model identifier
 * and configured `max_tokens` ceiling.
 */
export function resolveModelContextWindow(
  modelName: string | undefined,
  configuredMaxOutputTokens?: number,
): number {
  const m = stripProviderPrefix(modelName ?? '');
  const outputFloor = Math.max(4096, configuredMaxOutputTokens ?? 4096);

  if (m.includes('gemini-1.5') || m.includes('gemini-2') || m.includes('gemini-3')) {
    return Math.max(1_048_576, outputFloor * 4);
  }
  if (m.includes('gpt-4.1') || m.includes('1m')) {
    return Math.max(1_000_000, outputFloor * 4);
  }
  if (m.includes('minimax') || m.includes('m3.1') || m.includes('m2.5')) {
    return Math.max(245_760, outputFloor * 4);
  }
  if (
    m.includes('claude') ||
    m.startsWith('o1') ||
    m.startsWith('o3') ||
    m.startsWith('o4') ||
    m.includes('codex') ||
    m.includes('grok-4')
  ) {
    return Math.max(200_000, outputFloor * 4);
  }
  if (
    m.includes('gpt-4') ||
    m.includes('gpt-5') ||
    m.includes('deepseek') ||
    m.includes('qwen') ||
    m.includes('llama-3.1') ||
    m.includes('llama-3.3') ||
    m.includes('llama-4') ||
    m.includes('mistral') ||
    m.includes('kimi') ||
    m.includes('glm')
  ) {
    return Math.max(128_000, outputFloor * 4);
  }
  if (m.includes('gemma-2') || m.includes('phi-3')) {
    return Math.max(32_768, outputFloor * 2);
  }

  return Math.max(128_000, outputFloor * 16);
}

const MIME_EXTENSION_SUPPLEMENTS: Record<string, readonly string[]> = {
  'application/pdf': ['.pdf'],
  'application/json': ['.json'],
  'text/plain': ['.txt', '.log', '.rs', '.ts', '.tsx', '.js', '.jsx', '.py', '.toml', '.yaml', '.yml'],
  'text/markdown': ['.md', '.mdx'],
  'text/csv': ['.csv', '.tsv'],
};

/**
 * Derives the `<input type="file" accept="...">` attribute directly from the
 * active runtime `AttachmentPolicy.allowedMimeTypes`.
 */
export function mimePolicyToAcceptAttr(allowedMimeTypes: readonly string[]): string {
  const tokens = new Set<string>();
  for (const raw of allowedMimeTypes) {
    const mime = raw.trim().toLowerCase();
    if (!mime) continue;
    tokens.add(mime);
    if (mime === 'text/*') {
      for (const ext of MIME_EXTENSION_SUPPLEMENTS['text/plain']) tokens.add(ext);
      for (const ext of MIME_EXTENSION_SUPPLEMENTS['text/markdown']) tokens.add(ext);
      for (const ext of MIME_EXTENSION_SUPPLEMENTS['text/csv']) tokens.add(ext);
    } else if (MIME_EXTENSION_SUPPLEMENTS[mime]) {
      for (const ext of MIME_EXTENSION_SUPPLEMENTS[mime]) tokens.add(ext);
    }
  }
  return Array.from(tokens).join(',');
}

/**
 * Extracts a clean `host:port` endpoint label from a WebSocket URL without regex.
 */
export function extractGatewayEndpointLabel(wsUrl: string): string {
  const raw = wsUrl.trim();
  if (!raw) return '127.0.0.1:8765';
  const schemeIdx = raw.indexOf('://');
  const afterScheme = schemeIdx >= 0 ? raw.slice(schemeIdx + 3) : raw;
  const slashIdx = afterScheme.indexOf('/');
  const hostPort = slashIdx >= 0 ? afterScheme.slice(0, slashIdx) : afterScheme;
  return hostPort || '127.0.0.1:8765';
}

/**
 * Builds a theme-adaptive SVG preview Data URI for `@beui/image-generation`
 * so the preview reflects the active Light or Dark theme variables dynamically.
 */
export function buildThemeSampleSvgDataUri(palette: {
  backgroundHex: string;
  cardHex: string;
  borderHex: string;
  primaryHex: string;
  accentHex: string;
  foregroundHex: string;
  mutedForegroundHex: string;
}): string {
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="640" height="300" viewBox="0 0 640 300">' +
    `<rect width="640" height="300" fill="${palette.backgroundHex}"/>` +
    `<rect x="18" y="18" width="604" height="264" rx="18" fill="${palette.cardHex}" stroke="${palette.borderHex}"/>` +
    `<circle cx="220" cy="144" r="82" fill="${palette.primaryHex}" opacity="0.2"/>` +
    `<circle cx="420" cy="144" r="72" fill="${palette.accentHex}" opacity="0.18"/>` +
    `<path d="M294 68 C286 86 286 112 292 126 C278 148 264 174 260 182 L266 185 L261 190 C276 202 296 212 306 214 C312 222 328 222 334 214 C344 212 364 202 379 190 L374 185 L380 182 C376 174 362 148 348 126 C354 112 354 86 346 68 C336 78 328 96 323 104 L317 104 C312 96 304 78 294 68 Z" fill="${palette.primaryHex}"/>` +
    `<text x="320" y="252" text-anchor="middle" fill="${palette.foregroundHex}" font-family="monospace" font-size="14">OpenZ @beui/image-generation · ${palette.primaryHex}</text>` +
    '</svg>';
  return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
}
