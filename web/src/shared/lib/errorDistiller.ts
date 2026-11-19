/**
 * Zero-regex structured error distiller for OpenZ WebUI.
 *
 * Converts verbose upstream provider/gateway/tool error payloads (e.g., Google Gemini
 * RPC QuotaFailure JSON arrays, OpenAI/Anthropic/OpenRouter HTTP error envelopes, or
 * multi-retry stack traces) into concise, human-friendly summaries while preserving
 * formatted diagnostic traces behind an on-demand disclosure.
 */

export interface DistilledErrorInfo {
  isError: boolean;
  title: string;
  summary: string;
  provider?: string;
  httpCode?: number;
  statusTag?: string;
  model?: string;
  retryAfter?: string;
  helpUrl?: string;
  rawDetail: string;
  hasExtraDetail: boolean;
}

const LEADING_ERROR_PREFIXES = [
  '⚠️',
  '❌',
  '🛑',
  '**Error**:',
  '**Error**',
  'Error:',
  '[Error]',
] as const;

function stripLeadingErrorMarkers(input: string): { stripped: string; hadMarker: boolean } {
  let cur = input.trim();
  let hadMarker = false;
  let changed = true;

  while (changed) {
    changed = false;
    for (const prefix of LEADING_ERROR_PREFIXES) {
      if (cur.startsWith(prefix)) {
        cur = cur.slice(prefix.length).trimStart();
        hadMarker = true;
        changed = true;
      }
    }
  }

  return { stripped: cur, hadMarker };
}

function isAsciiDigit(ch: string): boolean {
  return ch >= '0' && ch <= '9';
}

function parseLeadingPositiveInt(text: string): number | undefined {
  let digits = '';
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i]!;
    if (isAsciiDigit(ch)) {
      digits += ch;
    } else {
      break;
    }
  }
  if (!digits) return undefined;
  const parsed = Number.parseInt(digits, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function extractProviderAndHttpFromPreamble(preamble: string): {
  provider?: string;
  httpCode?: number;
} {
  let provider: string | undefined;
  let httpCode: number | undefined;

  const provMarker = "Provider '";
  const provIdx = preamble.indexOf(provMarker);
  if (provIdx >= 0) {
    const start = provIdx + provMarker.length;
    const end = preamble.indexOf("'", start);
    if (end > start) {
      provider = preamble.slice(start, end).trim();
    }
  }

  const httpMarker = 'HTTP ';
  const httpIdx = preamble.indexOf(httpMarker);
  if (httpIdx >= 0) {
    const code = parseLeadingPositiveInt(preamble.slice(httpIdx + httpMarker.length));
    if (code && code >= 100 && code <= 599) {
      httpCode = code;
    }
  }

  return { provider, httpCode };
}

function tryExtractEmbeddedJson(text: string): {
  preamble: string;
  parsed: unknown;
  prettyJson: string;
} | null {
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (ch !== '{' && ch !== '[') continue;
    const candidate = text.slice(i).trim();
    try {
      const parsed: unknown = JSON.parse(candidate);
      if (typeof parsed === 'object' && parsed !== null) {
        return {
          preamble: text.slice(0, i).trim(),
          parsed,
          prettyJson: JSON.stringify(parsed, null, 2),
        };
      }
    } catch {
      // Continue scanning in case an earlier bracket was plain prose (e.g. "[1/3]")
    }
  }
  return null;
}

function isPlainRecord(val: unknown): val is Record<string, unknown> {
  return typeof val === 'object' && val !== null && !Array.isArray(val);
}

/**
 * Formats raw retry durations such as `"78134s"`, `"45.8s"`, or `"21h42m14.36328899s"`
 * into compact human strings like `"21h 42m"` or `"46s"` without regex.
 */
export function formatCompactRetryDelay(rawDelay: string): string {
  const clean = rawDelay.trim();
  if (!clean) return '';

  // Case 1: Pure seconds string like "78134s" or "42.5s"
  if (clean.endsWith('s') && !clean.includes('m') && !clean.includes('h')) {
    const numeric = Number.parseFloat(clean.slice(0, -1));
    if (Number.isFinite(numeric) && numeric > 0) {
      const totalSecs = Math.ceil(numeric);
      const hours = Math.floor(totalSecs / 3600);
      const mins = Math.floor((totalSecs % 3600) / 60);
      const secs = totalSecs % 60;
      if (hours > 0) {
        return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
      }
      if (mins > 0) {
        return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
      }
      return `${secs}s`;
    }
  }

  // Case 2: Compound duration like "21h42m14.36328899s" -> strip fractional seconds & space units
  let hoursPart = '';
  let minsPart = '';
  let secsPart = '';
  let buf = '';

  for (let i = 0; i < clean.length; i += 1) {
    const ch = clean[i]!;
    if (isAsciiDigit(ch) || ch === '.') {
      buf += ch;
    } else if (ch === 'h' && buf) {
      hoursPart = `${Math.floor(Number.parseFloat(buf))}h`;
      buf = '';
    } else if (ch === 'm' && buf) {
      minsPart = `${Math.floor(Number.parseFloat(buf))}m`;
      buf = '';
    } else if (ch === 's' && buf) {
      const rounded = Math.ceil(Number.parseFloat(buf));
      secsPart = `${rounded}s`;
      buf = '';
    }
  }

  if (hoursPart) {
    return minsPart ? `${hoursPart} ${minsPart}` : hoursPart;
  }
  if (minsPart) {
    return secsPart ? `${minsPart} ${secsPart}` : minsPart;
  }
  if (secsPart) {
    return secsPart;
  }

  return clean.length > 20 ? clean.slice(0, 20) : clean;
}

function unescapeLiteralNewlines(input: string): string {
  return input.split('\\n').join('\n').split('\\r').join('');
}

function extractConciseSentence(rawMessage: string): {
  sentence: string;
  inlineRetry?: string;
} {
  const normalized = unescapeLiteralNewlines(rawMessage).trim();
  const lines = normalized
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  let inlineRetry: string | undefined;
  for (const line of lines) {
    const lower = line.toLowerCase();
    const retryMarker = 'please retry in ';
    const idx = lower.indexOf(retryMarker);
    if (idx >= 0) {
      let rawDur = line.slice(idx + retryMarker.length).trim();
      if (rawDur.endsWith('.')) rawDur = rawDur.slice(0, -1);
      const formatted = formatCompactRetryDelay(rawDur);
      if (formatted) inlineRetry = formatted;
    }
  }

  const firstLine = lines[0] ?? normalized;
  const sentenceBreak = firstLine.indexOf('. ');
  const firstSentence =
    sentenceBreak > 12 ? `${firstLine.slice(0, sentenceBreak + 1)}` : firstLine;

  const clamped =
    firstSentence.length > 150
      ? `${firstSentence.slice(0, 147).trimEnd()}...`
      : firstSentence;

  return { sentence: clamped, inlineRetry };
}

function inspectStructuredErrorJson(root: unknown): {
  httpCode?: number;
  statusTag?: string;
  message?: string;
  model?: string;
  retryAfter?: string;
  helpUrl?: string;
} {
  const topItem = Array.isArray(root) ? root[0] : root;
  if (!isPlainRecord(topItem)) return {};

  const errObj = isPlainRecord(topItem.error) ? topItem.error : topItem;

  let httpCode: number | undefined;
  let statusTag: string | undefined;

  if (typeof errObj.code === 'number' && errObj.code >= 100 && errObj.code <= 599) {
    httpCode = errObj.code;
  } else if (typeof errObj.code === 'string' && errObj.code.trim()) {
    statusTag = errObj.code.trim();
  }

  if (typeof errObj.status === 'string' && errObj.status.trim()) {
    statusTag = errObj.status.trim();
  } else if (!statusTag && typeof errObj.type === 'string' && errObj.type.trim()) {
    statusTag = errObj.type.trim();
  }

  const rawMessage =
    typeof errObj.message === 'string'
      ? errObj.message
      : typeof topItem.message === 'string'
        ? topItem.message
        : typeof topItem.detail === 'string'
          ? topItem.detail
          : undefined;

  let model: string | undefined =
    typeof errObj.model === 'string' ? errObj.model : undefined;
  let retryAfter: string | undefined;
  let helpUrl: string | undefined;

  if (Array.isArray(errObj.details)) {
    for (const detailItem of errObj.details) {
      if (!isPlainRecord(detailItem)) continue;
      if (typeof detailItem.retryDelay === 'string' && detailItem.retryDelay.trim()) {
        retryAfter = formatCompactRetryDelay(detailItem.retryDelay);
      }
      if (Array.isArray(detailItem.violations)) {
        for (const v of detailItem.violations) {
          if (!isPlainRecord(v)) continue;
          if (isPlainRecord(v.quotaDimensions) && typeof v.quotaDimensions.model === 'string') {
            model = v.quotaDimensions.model;
          }
        }
      }
      if (!helpUrl && Array.isArray(detailItem.links)) {
        for (const link of detailItem.links) {
          if (
            isPlainRecord(link) &&
            typeof link.url === 'string' &&
            link.url.startsWith('https://')
          ) {
            helpUrl = link.url;
            break;
          }
        }
      }
    }
  }

  return {
    httpCode,
    statusTag,
    message: rawMessage,
    model,
    retryAfter,
    helpUrl,
  };
}

function deriveErrorTitle(
  httpCode?: number,
  statusTag?: string,
  provider?: string,
): string {
  const upperTag = (statusTag ?? '').toUpperCase();
  if (
    httpCode === 429 ||
    upperTag.includes('RESOURCE_EXHAUSTED') ||
    upperTag.includes('RATE_LIMIT')
  ) {
    return 'Rate limit exceeded';
  }
  if (
    httpCode === 401 ||
    upperTag.includes('UNAUTHENTICATED') ||
    upperTag.includes('INVALID_API_KEY')
  ) {
    return 'Authentication failed';
  }
  if (httpCode === 402 || upperTag.includes('BILLING') || upperTag.includes('INSUFFICIENT_QUOTA')) {
    return 'Quota or billing limit';
  }
  if (httpCode === 403 || upperTag.includes('PERMISSION_DENIED')) {
    return 'Permission denied';
  }
  if (httpCode === 404 || upperTag.includes('NOT_FOUND')) {
    return 'Model or endpoint not found';
  }
  if (httpCode === 408 || httpCode === 504 || upperTag.includes('DEADLINE_EXCEEDED')) {
    return 'Request timed out';
  }
  if (httpCode && httpCode >= 500) {
    return 'Provider temporarily unavailable';
  }
  if (provider) {
    return `${provider} error`;
  }
  return 'Request failed';
}

export function isLikelyErrorNoticeText(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed) return false;
  for (const prefix of LEADING_ERROR_PREFIXES) {
    if (trimmed.startsWith(prefix)) return true;
  }
  const lower = trimmed.toLowerCase();
  if (lower.startsWith('provider ') && lower.includes(' error')) return true;
  if (lower.startsWith('gateway error')) return true;
  return false;
}

export function distillErrorMessage(
  rawText: string,
  explicitTitle?: string,
): DistilledErrorInfo {
  const original = rawText.trim();
  const { stripped, hadMarker } = stripLeadingErrorMarkers(original);
  const embedded = tryExtractEmbeddedJson(stripped);

  const preambleSource = embedded ? embedded.preamble : stripped;
  const { provider, httpCode: preambleCode } =
    extractProviderAndHttpFromPreamble(preambleSource);

  const jsonMeta = embedded ? inspectStructuredErrorJson(embedded.parsed) : {};
  const httpCode = preambleCode ?? jsonMeta.httpCode;
  const statusTag = jsonMeta.statusTag;

  const rawBodyForSummary = jsonMeta.message ?? preambleSource;
  const { sentence, inlineRetry } = extractConciseSentence(rawBodyForSummary);
  const retryAfter = jsonMeta.retryAfter ?? inlineRetry;

  const computedTitle =
    explicitTitle && explicitTitle !== 'Step encountered an error'
      ? explicitTitle
      : deriveErrorTitle(httpCode, statusTag, provider);

  const cleanPreamble = preambleSource.endsWith('Last error:')
    ? preambleSource.slice(0, -'Last error:'.length).trim()
    : preambleSource;

  const formattedTrace = embedded
    ? cleanPreamble
      ? `${cleanPreamble}\n\n${embedded.prettyJson}`
      : embedded.prettyJson
    : stripped;

  const hasExtraDetail =
    Boolean(embedded) ||
    stripped.length > sentence.length + 18 ||
    stripped.includes('\n');

  return {
    isError: hadMarker || Boolean(httpCode) || Boolean(embedded) || isLikelyErrorNoticeText(original),
    title: computedTitle,
    summary: sentence || 'An unexpected runtime error occurred.',
    provider,
    httpCode,
    statusTag,
    model: jsonMeta.model,
    retryAfter,
    helpUrl: jsonMeta.helpUrl,
    rawDetail: formattedTrace,
    hasExtraDetail,
  };
}
