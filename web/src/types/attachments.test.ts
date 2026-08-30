import { expect, test } from 'bun:test';
import {
  attachmentFitsQuota,
  attachmentMimeAllowed,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENT_TOTAL_BYTES,
} from './attachments';

test('attachment policy aligns MIME and aggregate size limits', () => {
  expect(attachmentMimeAllowed('image/png')).toBe(true);
  expect(attachmentMimeAllowed('application/pdf')).toBe(true);
  expect(attachmentMimeAllowed('application/x-sh')).toBe(false);
  expect(attachmentMimeAllowed('image/png', ['application/pdf'])).toBe(false);
  expect(attachmentFitsQuota(0, MAX_ATTACHMENT_BYTES)).toBe(true);
  expect(attachmentFitsQuota(MAX_ATTACHMENT_TOTAL_BYTES - 1, 2)).toBe(false);
  expect(attachmentFitsQuota(0, 2, 1)).toBe(false);
});
