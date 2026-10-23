function isAsciiAlnum(ch: string): boolean {
  const code = ch.charCodeAt(0);
  return (code >= 48 && code <= 57) || (code >= 97 && code <= 122);
}

export function collapseWhitespace(value: string): string {
  const tokens: string[] = [];
  let current = '';
  for (const ch of value) {
    const isSpace = ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r';
    if (isSpace) {
      if (current) {
        tokens.push(current);
        current = '';
      }
    } else {
      current += ch;
    }
  }
  if (current) tokens.push(current);
  return tokens.join(' ');
}

export function normalizeIdentifier(value: string, allowDash = true): string {
  const lower = value.trim().toLowerCase();
  let out = '';
  let lastUnderscore = false;
  for (const ch of lower) {
    const valid = isAsciiAlnum(ch) || ch === '_' || (allowDash && ch === '-');
    if (valid) {
      out += ch;
      lastUnderscore = ch === '_';
    } else if (!lastUnderscore && out.length > 0) {
      out += '_';
      lastUnderscore = true;
    }
  }
  while (out.startsWith('_')) out = out.slice(1);
  while (out.endsWith('_')) out = out.slice(0, -1);
  return out;
}

export function sanitizeProviderKey(value: string): string {
  let out = '';
  for (const ch of value.toLowerCase()) {
    if (isAsciiAlnum(ch) || ch === '_' || ch === '-') {
      out += ch;
    }
  }
  return out;
}

export function stripMarkdownFormatting(line: string): string {
  let trimmed = line.trimStart();
  while (trimmed.startsWith('#')) {
    trimmed = trimmed.slice(1);
  }
  trimmed = trimmed.trimStart();
  let out = '';
  for (const ch of trimmed) {
    if (ch !== '`' && ch !== '*' && ch !== '_') {
      out += ch;
    }
  }
  return out.trim();
}

export function humanizeCamelCase(value: string): string {
  if (!value) return '';
  let out = value.charAt(0).toUpperCase();
  for (let i = 1; i < value.length; i += 1) {
    const ch = value.charAt(i);
    const code = ch.charCodeAt(0);
    if (code >= 65 && code <= 90) {
      out += ' ' + ch;
    } else {
      out += ch;
    }
  }
  return out;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024 * 1024) return Math.max(1, Math.round(bytes / 1024)) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0) + ' MB';
}

export function formatDuration(ms?: number): string | null {
  if (!ms) return null;
  if (ms < 1000) return ms + 'ms';
  return (ms / 1000).toFixed(ms < 10000 ? 1 : 0) + 's';
}

export function formatObservations(value: unknown): string {
  const raw = typeof value === 'string' ? value : String(value ?? '');
  try {
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? parsed.map(String).join(' · ') : String(parsed);
  } catch {
    return raw || 'No observations recorded.';
  }
}
