export const MAX_ATTACHMENTS = 8;
export const MAX_ATTACHMENT_BYTES = 8 * 1024 * 1024;
export const MAX_ATTACHMENT_TOTAL_BYTES = 24 * 1024 * 1024;

const ALLOWED_ATTACHMENT_MIME = new Set([
  'image/png',
  'image/jpeg',
  'image/gif',
  'image/webp',
  'image/bmp',
  'image/tiff',
  'image/svg+xml',
  'application/pdf',
  'text/plain',
  'text/markdown',
  'text/csv',
  'application/json',
  'application/xml',
  'text/xml',
  'application/msword',
  'application/vnd.ms-excel',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  'application/vnd.openxmlformats-officedocument.presentationml.presentation',
]);

export function attachmentMimeAllowed(mime: string): boolean {
  const normalized = mime.trim().toLowerCase();
  const hasControlCharacter = [...normalized].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 0x1f || code === 0x7f;
  });
  return normalized.length <= 128 && !hasControlCharacter && ALLOWED_ATTACHMENT_MIME.has(normalized);
}

export function attachmentFitsQuota(currentBytes: number, nextBytes: number): boolean {
  return currentBytes >= 0
    && nextBytes > 0
    && currentBytes <= MAX_ATTACHMENT_TOTAL_BYTES
    && nextBytes <= MAX_ATTACHMENT_TOTAL_BYTES - currentBytes;
}
