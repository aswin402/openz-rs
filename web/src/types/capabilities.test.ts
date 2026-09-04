import { expect, test } from 'bun:test';
import { normalizeWebUiCapabilities } from './openz';

test('normalizes missing or malformed capability payloads to safe defaults', () => {
  const capabilities = normalizeWebUiCapabilities({
    version: 'old',
    providers: [{ name: 'anthropic', configKey: 'anthropic', display: 'Anthropic' }],
    securityModes: [{ value: 'normal', label: 'Normal' }],
    channels: [{
      name: 'whatsapp',
      label: 'WhatsApp',
      fields: [{ key: 'webhook_port', label: 'Webhook port', kind: 'number' }],
      defaults: { webhook_port: 8090 },
    }],
    attachments: {
      maxCount: 4,
      maxFileBytes: '8MiB',
      maxTotalBytes: 24 * 1024 * 1024,
      maxMessageBytes: 32 * 1024 * 1024,
      ttlSeconds: null,
      allowedMimeTypes: ['application/pdf', 42],
    },
  });

  expect(capabilities.version).toBe(0);
  expect(capabilities.providers).toEqual([{
    name: 'anthropic',
    configKey: 'anthropic',
    display: 'Anthropic',
    available: false,
    apiBaseEditable: false,
  }]);
  expect(capabilities.securityModes).toEqual([{ value: 'normal', label: 'Normal' }]);
  expect(capabilities.channels[0]?.defaults.webhook_port).toBe(8090);
  expect(capabilities.browser).toEqual({
    firefoxWebdriverPort: 0,
    firefoxAttachPort: 0,
  });
  expect(capabilities.attachments.maxCount).toBe(4);
  expect(capabilities.attachments.maxFileBytes).toBe(0);
  expect(capabilities.attachments.allowedMimeTypes).toEqual(['application/pdf']);
});

test('normalizes absent capabilities without leaving undefined nested fields', () => {
  const capabilities = normalizeWebUiCapabilities(undefined);

  expect(capabilities.version).toBe(0);
  expect(capabilities.providers).toEqual([]);
  expect(capabilities.securityModes).toEqual([]);
  expect(capabilities.channels).toEqual([]);
  expect(capabilities.browser).toEqual({
    firefoxWebdriverPort: 0,
    firefoxAttachPort: 0,
  });
  expect(capabilities.attachments).toEqual({
    maxCount: 0,
    maxFileBytes: 0,
    maxTotalBytes: 0,
    maxMessageBytes: 0,
    ttlSeconds: 0,
    allowedMimeTypes: [],
  });
});
