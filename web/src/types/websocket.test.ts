import { describe, expect, test } from 'bun:test';
import {
  buildCommandEnvelope,
  createRequestId,
  isCommandAckEvent,
  parseWebSocketEvent,
} from './websocket';

describe('WebSocket protocol contracts', () => {
  test('creates unique request IDs and embeds them in command envelopes', () => {
    const first = createRequestId(1, 1700000000000);
    const second = createRequestId(2, 1700000000000);
    expect(first).not.toBe(second);
    expect(buildCommandEnvelope({ type: 'get_status' }, first)).toEqual({
      type: 'get_status',
      request_id: first,
    });
  });

  test('normalizes event and legacy type envelopes', () => {
    expect(parseWebSocketEvent({ event: 'ready', chat_id: 'chat-1' })).toMatchObject({
      event: 'ready',
      chat_id: 'chat-1',
    });
    expect(parseWebSocketEvent({ type: 'run_started', run_id: 'run-1' })).toMatchObject({
      event: 'run_started',
      run_id: 'run-1',
    });
    expect(parseWebSocketEvent({ detail: 'missing event' })).toBeNull();
  });

  test('recognizes only valid command acknowledgement events', () => {
    const ack = parseWebSocketEvent({
      event: 'command_ack',
      request_id: 'req-1',
      command: 'get_status',
      status: 'accepted',
    });
    expect(ack && isCommandAckEvent(ack)).toBe(true);
    expect(isCommandAckEvent(parseWebSocketEvent({ event: 'ready' }))).toBe(false);
  });

  test('uses request_id rather than a UI-only correlation field', () => {
    const envelope = buildCommandEnvelope(
      { type: 'security_response', req_id: 'sec-1', approved: false },
      'req-9',
    );
    expect(envelope.request_id).toBe('req-9');
    expect(envelope.req_id).toBe('sec-1');
  });

  test('rejected acknowledgements carry actionable detail', () => {
    const ack = parseWebSocketEvent({
      event: 'command_ack',
      request_id: 'req-1',
      command: 'set_config',
      status: 'rejected',
      detail: 'WebSocket is not connected',
    });
    expect(isCommandAckEvent(ack)).toBe(true);
    expect(ack && isCommandAckEvent(ack) && ack.detail).toBe('WebSocket is not connected');
  });
});
