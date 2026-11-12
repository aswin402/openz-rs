import { expect, test } from 'bun:test';
import { useOpenZStore } from './useOpenZStore';
import { registerChatTurnEvents } from './events/chatTurnEvents';
import { registerSessionEvents } from './events/sessionEvents';
import { wsService } from '../services/websocket';

let registered = false;
function ensureRegistered() {
  if (registered) return;
  registered = true;
  const ctx = {
    set: useOpenZStore.setState,
    get: useOpenZStore.getState,
  };
  registerSessionEvents(ctx);
  registerChatTurnEvents(ctx);
}

test('ready and sessions_list keep WebUI on fresh New Session draft without auto-switching', () => {
  ensureRegistered();
  useOpenZStore.setState({
    sessions: [],
    activeChatId: '',
    messages: {},
    activeTurnIds: {},
    isStreaming: false,
  });

  const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);

  emit('ready', {
    chat_id: 'ws_draft-fresh-1',
  });

  expect(useOpenZStore.getState().activeChatId).toBe('ws:draft-fresh-1');
  expect(useOpenZStore.getState().isStreaming).toBe(false);
  expect(useOpenZStore.getState().sessions[0]?.title).toBe('New Session');
  expect(useOpenZStore.getState().sessions[0]?.isDraft).toBe(true);

  emit('sessions_list', {
    sessions: [
      { id: 'ws_old-chat-1', title: 'hii', createdAt: 10, lastMessageAt: 20, messageCount: 2 },
      { id: 'ws_old-chat-2', title: 'so whats new', createdAt: 30, lastMessageAt: 40, messageCount: 4 },
    ],
  });

  const state = useOpenZStore.getState();
  expect(state.activeChatId).toBe('ws:draft-fresh-1');
  expect(state.sessions.map((s) => s.id)).toEqual([
    'ws:draft-fresh-1',
    'ws:old-chat-1',
    'ws:old-chat-2',
  ]);
  expect(state.isStreaming).toBe(false);
});

test('unsolicited background delta or non-active chat turn never sets isStreaming on idle active session', () => {
  ensureRegistered();
  useOpenZStore.setState({
    sessions: [{ id: 'ws:draft-idle', title: 'New Session', createdAt: 1, lastMessageAt: 1, messageCount: 0, isDraft: true }],
    activeChatId: 'ws:draft-idle',
    messages: {},
    activeTurnIds: {},
    isStreaming: false,
  });

  const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);

  // Unsolicited background cron delta without turn_started or streaming placeholder
  emit('delta', {
    chat_id: 'cron_9a8a63f0',
    content: 'background cron output',
  });

  expect(useOpenZStore.getState().isStreaming).toBe(false);
  expect(useOpenZStore.getState().messages['cron_9a8a63f0']).toBeUndefined();
  expect(useOpenZStore.getState().messages['ws:cron_9a8a63f0']).toBeUndefined();
  expect(useOpenZStore.getState().messages['ws:draft-idle']).toBeUndefined();

  // Active turn on a DIFFERENT WebUI chat should not flip isStreaming on ws:draft-idle
  emit('turn_started', {
    chat_id: 'ws_other-session',
    turn_id: 'turn-bg-1',
  });
  emit('delta', {
    chat_id: 'ws_other-session',
    turn_id: 'turn-bg-1',
    content: 'streaming in another tab',
  });

  expect(useOpenZStore.getState().isStreaming).toBe(false);
  expect(useOpenZStore.getState().messages['ws:other-session']?.[0]?.content).toBe('streaming in another tab');
});

test('sendMessage forwards streaming and caveman_mode overrides and preserves in-flight stream across delayed session_history', () => {
  ensureRegistered();
  const captured: Array<{ chatId: string; content: string; options?: { streaming?: boolean; cavemanMode?: boolean } }> = [];
  const origSendMessage = wsService.sendMessage.bind(wsService);
  wsService.sendMessage = ((chatId, content, _model, _provider, _attachments, options) => {
    captured.push({ chatId, content, options });
    return 'req-test-1';
  }) as typeof wsService.sendMessage;

  try {
    useOpenZStore.setState({
      sessions: [{ id: 'ws:stream-1', title: 'New Session', createdAt: 1, lastMessageAt: 1, messageCount: 0, isDraft: true }],
      activeChatId: 'ws:stream-1',
      messages: {},
      activeTurnIds: {},
      isStreaming: false,
      streamingMode: true,
      cavemanMode: false,
    });

    useOpenZStore.getState().sendMessage('Explain async streaming');
    expect(captured).toHaveLength(1);
    expect(captured[0]).toEqual({
      chatId: 'ws:stream-1',
      content: 'Explain async streaming',
      options: { streaming: true, cavemanMode: false },
    });

    const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);
    emit('turn_started', { chat_id: 'ws_stream-1', turn_id: 'turn-live-1' });
    emit('reasoning_delta', { chat_id: 'ws_stream-1', turn_id: 'turn-live-1', content: 'Thinking step 1... ' });

    // Simulate a late empty session_history arriving while the turn is actively streaming
    emit('session_history', { chat_id: 'ws_stream-1', messages: [] });

    emit('delta', { chat_id: 'ws_stream-1', turn_id: 'turn-live-1', content: 'Live streamed ' });
    emit('delta', { chat_id: 'ws_stream-1', turn_id: 'turn-live-1', content: 'response!' });

    const msgs = useOpenZStore.getState().messages['ws:stream-1'] || [];
    expect(msgs).toHaveLength(2);
    expect(msgs[0]?.role).toBe('user');
    expect(msgs[0]?.content).toBe('Explain async streaming');
    expect(msgs[1]?.role).toBe('assistant');
    expect(msgs[1]?.reasoningContent).toBe('Thinking step 1... ');
    expect(msgs[1]?.content).toBe('Live streamed response!');
    expect(msgs[1]?.isStreaming).toBe(true);
    expect(useOpenZStore.getState().isStreaming).toBe(true);

    emit('turn_end', { chat_id: 'ws_stream-1', turn_id: 'turn-live-1' });
    expect(useOpenZStore.getState().isStreaming).toBe(false);
    expect(useOpenZStore.getState().messages['ws:stream-1']?.[1]?.isStreaming).toBe(false);
  } finally {
    wsService.sendMessage = origSendMessage;
  }
});

test('concurrent async turns across two sessions stream independently and track per-session isStreaming', () => {
  ensureRegistered();
  const origAttach = wsService.attachChat.bind(wsService);
  wsService.attachChat = (() => {}) as typeof wsService.attachChat;

  try {
    useOpenZStore.setState({
      sessions: [
        { id: 'ws:chat-a', title: 'Session A', createdAt: 1, lastMessageAt: 1, messageCount: 2 },
        { id: 'ws:chat-b', title: 'Session B', createdAt: 2, lastMessageAt: 2, messageCount: 2 },
      ],
      activeChatId: 'ws:chat-a',
      messages: {
        'ws:chat-a': [
          { id: 'u-a', role: 'user', content: 'Task A', timestamp: 1 },
          { id: 'a-a', role: 'assistant', content: '', timestamp: 2, isStreaming: true },
        ],
        'ws:chat-b': [
          { id: 'u-b', role: 'user', content: 'Task B', timestamp: 3 },
          { id: 'a-b', role: 'assistant', content: '', timestamp: 4, isStreaming: true },
        ],
      },
      activeTurnIds: {},
      isStreaming: true,
    });

    const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);
    emit('turn_started', { chat_id: 'ws_chat-a', turn_id: 'turn-a' });
    emit('turn_started', { chat_id: 'ws_chat-b', turn_id: 'turn-b' });

    // Interleaved deltas on both sessions simultaneously
    emit('delta', { chat_id: 'ws_chat-a', turn_id: 'turn-a', content: 'Alpha-1 ' });
    emit('delta', { chat_id: 'ws_chat-b', turn_id: 'turn-b', content: 'Beta-1 ' });
    emit('delta', { chat_id: 'ws_chat-a', turn_id: 'turn-a', content: 'Alpha-2' });

    // Finish Session A while Session B keeps streaming in background
    emit('turn_end', { chat_id: 'ws_chat-a', turn_id: 'turn-a' });
    expect(useOpenZStore.getState().isStreaming).toBe(false);
    expect(useOpenZStore.getState().activeTurnIds['ws:chat-b']).toBe('turn-b');

    // Switch to Session B -> isStreaming becomes true immediately
    useOpenZStore.getState().selectSession('ws:chat-b');
    expect(useOpenZStore.getState().activeChatId).toBe('ws:chat-b');
    expect(useOpenZStore.getState().isStreaming).toBe(true);

    emit('delta', { chat_id: 'ws_chat-b', turn_id: 'turn-b', content: 'Beta-2' });
    emit('turn_end', { chat_id: 'ws_chat-b', turn_id: 'turn-b' });

    const state = useOpenZStore.getState();
    expect(state.isStreaming).toBe(false);
    expect(state.messages['ws:chat-a']?.[1]?.content).toBe('Alpha-1 Alpha-2');
    expect(state.messages['ws:chat-b']?.[1]?.content).toBe('Beta-1 Beta-2');
  } finally {
    wsService.attachChat = origAttach;
  }
});

test('remote_user_message appends remote prompt and streams live response in target WebUI session', () => {
  ensureRegistered();
  useOpenZStore.setState({
    sessions: [
      { id: 'ws:remote-1', title: 'Remote Session', createdAt: 1, lastMessageAt: 1, messageCount: 0 },
    ],
    activeChatId: 'ws:remote-1',
    messages: {
      'ws:remote-1': [],
    },
    activeTurnIds: {},
    isStreaming: false,
  });

  const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);
  emit('remote_user_message', {
    chat_id: 'ws_remote-1',
    turn_id: 'turn-rem-1',
    sender: 'telegram:999',
    content: 'Check server health',
  });

  const afterPrompt = useOpenZStore.getState();
  expect(afterPrompt.messages['ws:remote-1']?.length).toBe(1);
  expect(afterPrompt.messages['ws:remote-1']?.[0]?.role).toBe('user');
  expect(afterPrompt.messages['ws:remote-1']?.[0]?.content).toBe('[Remote · telegram:999] Check server health');

  emit('turn_started', { chat_id: 'ws_remote-1', turn_id: 'turn-rem-1' });
  expect(useOpenZStore.getState().isStreaming).toBe(true);

  emit('delta', { chat_id: 'ws_remote-1', turn_id: 'turn-rem-1', content: 'All systems nominal.' });
  emit('turn_end', { chat_id: 'ws_remote-1', turn_id: 'turn-rem-1' });

  const finalState = useOpenZStore.getState();
  expect(finalState.isStreaming).toBe(false);
  expect(finalState.messages['ws:remote-1']?.length).toBe(2);
  expect(finalState.messages['ws:remote-1']?.[1]?.content).toBe('All systems nominal.');
});
