import { expect, test } from 'bun:test';

type FakeCloseEvent = { code: number; reason: string };

class FakeSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 3;
  static readonly instances: FakeSocket[] = [];

  readonly url: string;
  readonly sent: string[] = [];
  readyState = FakeSocket.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: FakeCloseEvent) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeSocket.instances.push(this);
  }

  send(payload: string) {
    if (this.readyState !== FakeSocket.OPEN) throw new Error('socket is not open');
    this.sent.push(payload);
  }

  open() {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.();
  }

  close() {
    this.readyState = FakeSocket.CLOSED;
    this.onclose?.({ code: 1000, reason: '' });
  }
}

test('queues disconnected config writes and replays unacknowledged writes', async () => {
  const values: Record<string, string> = {};
  const localStorageShim = {
    getItem: (key: string) => values[key] ?? null,
    setItem: (key: string, value: string) => { values[key] = value; },
    removeItem: (key: string) => { delete values[key]; },
  };
  const globals = globalThis as unknown as {
    localStorage: typeof localStorageShim;
    WebSocket: typeof FakeSocket;
  };
  globals.localStorage = localStorageShim;
  globals.WebSocket = FakeSocket;
  FakeSocket.instances.length = 0;

  const { OpenZWebSocketService } = await import('./websocket');
  const service = new OpenZWebSocketService();
  const queuedId = service.updateConfig({ streaming: false });
  expect(queuedId).toBeString();
  expect(FakeSocket.instances).toHaveLength(0);

  service.setConfig('ws://gateway.test/ws', '');
  const first = FakeSocket.instances[0];
  first.open();
  const queuedEnvelope = JSON.parse(first.sent[0]) as { request_id: string; type: string };
  expect(queuedEnvelope.type).toBe('set_config');
  expect(queuedEnvelope.request_id).toBe(queuedId);
  first.onmessage?.({
    data: JSON.stringify({
      event: 'command_ack',
      request_id: queuedId,
      command: 'set_config',
      status: 'accepted',
    }),
  });

  const inFlightId = service.updateConfig({ temperature: 0.2 });
  expect(first.sent).toHaveLength(2);
  first.close();

  service.connect();
  const second = FakeSocket.instances[1];
  second.open();
  const replayedEnvelope = JSON.parse(second.sent[0]) as { request_id: string; type: string };
  expect(replayedEnvelope.type).toBe('set_config');
  expect(replayedEnvelope.request_id).toBe(inFlightId);
  service.disconnect();
});
