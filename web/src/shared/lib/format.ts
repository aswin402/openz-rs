export function collapseWhitespace(value: string): string {
  return value.replace(/\s+/g, ' ').trim();
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
