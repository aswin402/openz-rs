export const MAX_ATTACHMENTS = 8;
export const MAX_ATTACHMENT_BYTES = 8 * 1024 * 1024;
export const MAX_ATTACHMENT_TOTAL_BYTES = 24 * 1024 * 1024;

export interface AttachmentPolicy {
  maxCount: number;
  maxFileBytes: number;
  maxTotalBytes: number;
  allowedMimeTypes: string[];
}

export const DEFAULT_ATTACHMENT_POLICY: AttachmentPolicy = {
  maxCount: MAX_ATTACHMENTS,
  maxFileBytes: MAX_ATTACHMENT_BYTES,
  maxTotalBytes: MAX_ATTACHMENT_TOTAL_BYTES,
  allowedMimeTypes: [],
};

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

export function attachmentMimeAllowed(mime: string, allowedMimeTypes?: readonly string[]): boolean {
  const normalized = mime.trim().toLowerCase();
  const hasControlCharacter = [...normalized].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 0x1f || code === 0x7f;
  });
  const allowed = allowedMimeTypes && allowedMimeTypes.length > 0
    ? new Set(allowedMimeTypes.map((type) => type.trim().toLowerCase()))
    : ALLOWED_ATTACHMENT_MIME;
  return normalized.length <= 128 && !hasControlCharacter && allowed.has(normalized);
}

export function attachmentFitsQuota(
  currentBytes: number,
  nextBytes: number,
  maxTotalBytes = MAX_ATTACHMENT_TOTAL_BYTES,
): boolean {
  return currentBytes >= 0
    && nextBytes > 0
    && currentBytes <= maxTotalBytes
    && nextBytes <= maxTotalBytes - currentBytes;
}
