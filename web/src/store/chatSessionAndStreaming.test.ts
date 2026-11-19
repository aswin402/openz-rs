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
  const captured: Array<{
    chatId: string;
    content: string;
    options?: { streaming?: boolean; cavemanMode?: boolean; cavemanLevel?: string };
  }> = [];
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
      cavemanLevel: 'ultra',
    });

    useOpenZStore.getState().sendMessage('Explain async streaming');
    expect(captured).toHaveLength(1);
    expect(captured[0]).toEqual({
      chatId: 'ws:stream-1',
      content: 'Explain async streaming',
      options: { streaming: true, cavemanMode: false, cavemanLevel: 'ultra' },
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

test('clearActiveSession and stopTurn use typed WebSocket commands without slash commands', () => {
  ensureRegistered();
  const clearedChats: string[] = [];
  const stoppedTurns: Array<{ chatId: string; turnId?: string }> = [];
  const sentMessages: Array<{ chatId: string; content: string }> = [];

  const origClear = wsService.clearSession.bind(wsService);
  const origStop = wsService.sendStop.bind(wsService);
  const origSend = wsService.sendMessage.bind(wsService);

  wsService.clearSession = ((chatId) => {
    clearedChats.push(chatId);
  }) as typeof wsService.clearSession;
  wsService.sendStop = ((chatId, turnId) => {
    stoppedTurns.push({ chatId, turnId });
    return 'req-stop-1';
  }) as typeof wsService.sendStop;
  wsService.sendMessage = ((chatId, content) => {
    sentMessages.push({ chatId, content });
    return 'req-msg-1';
  }) as typeof wsService.sendMessage;

  try {
    useOpenZStore.setState({
      sessions: [
        { id: 'ws:typed-1', title: 'Typed Session', createdAt: 1, lastMessageAt: 1, messageCount: 2 },
      ],
      activeChatId: 'ws:typed-1',
      messages: {
        'ws:typed-1': [
          { id: 'u-1', role: 'user', content: 'Hello', timestamp: 1 },
          { id: 'a-1', role: 'assistant', content: 'Streaming...', timestamp: 2, isStreaming: true },
        ],
      },
      activeTurnIds: { 'ws:typed-1': 'turn-typed-9' },
      isStreaming: true,
    });

    useOpenZStore.getState().stopTurn();
    expect(stoppedTurns).toEqual([{ chatId: 'ws:typed-1', turnId: 'turn-typed-9' }]);
    expect(sentMessages).toEqual([]);

    useOpenZStore.getState().clearActiveSession();
    expect(clearedChats).toEqual(['ws:typed-1']);
    expect(sentMessages).toEqual([]);
    expect(useOpenZStore.getState().messages['ws:typed-1']).toEqual([]);
  } finally {
    wsService.clearSession = origClear;
    wsService.sendStop = origStop;
    wsService.sendMessage = origSend;
  }
});

test('resumed session_history coalesces multi-round assistant tool iterations per turn without ghost bubbles or false-positive code errors', () => {
  ensureRegistered();
  const attachedChats: string[] = [];
  const origAttach = wsService.attachChat.bind(wsService);
  wsService.attachChat = ((chatId) => {
    attachedChats.push(chatId);
  }) as typeof wsService.attachChat;

  try {
    useOpenZStore.setState({
      sessions: [
        { id: 'ws:resume-multi', title: 'hii so whats new', createdAt: 1000, lastMessageAt: 9000, messageCount: 16, isDraft: false },
      ],
      activeChatId: 'ws:resume-multi',
      messages: {},
      activeTurnIds: {},
      isStreaming: false,
    });

    // Even when activeChatId already matches, empty non-draft session hydrates on selectSession
    useOpenZStore.getState().selectSession('ws_resume-multi');
    expect(attachedChats).toEqual(['ws:resume-multi']);

    const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);
    emit('session_history', {
      chat_id: 'ws_resume-multi',
      messages: [
        // Turn 1: User -> Assistant Round 1 (tools + partial note) -> Tool outputs -> Assistant Round 2 (tools) -> Tool outputs -> Assistant Final
        { id: 'm0', role: 'user', content: 'Audit Cargo.toml and error handling', timestamp: 1000 },
        {
          id: 'm1',
          role: 'assistant',
          content: 'Inspecting workspace files first.',
          timestamp: 1010,
          extra: {
            model: 'claude-sonnet-4-6',
            reasoning_content: 'Plan round 1',
            tool_calls: [
              { id: 'call_r1_a', function: { name: 'read_file', arguments: '{"path":"src/lib.rs"}' } },
              { id: 'call_r1_b', function: { name: 'grep_search', arguments: '{"query":"Result"}' } },
            ],
          },
        },
        {
          id: 'm2',
          role: 'tool',
          // Contains `"error"` inside source file content — must NOT false-positive as tool failure!
          content: '{"content":"pub fn parse() -> Result<(), &\'static str> { Err(\\"error\\") }"}',
          timestamp: 1020,
          extra: { tool_call_id: 'call_r1_a', name: 'read_file' },
        },
        {
          id: 'm3',
          role: 'tool',
          content: '{"matches": 4}',
          timestamp: 1030,
          extra: { tool_call_id: 'call_r1_b', name: 'grep_search' },
        },
        {
          id: 'm4',
          role: 'assistant',
          content: '',
          timestamp: 1040,
          extra: {
            model: 'claude-sonnet-4-6',
            reasoning_content: 'Plan round 2',
            tool_calls: [
              { id: 'call_r2_a', function: { name: 'exec_command', arguments: '{"command":"git status"}' } },
            ],
          },
        },
        {
          id: 'm5',
          role: 'tool',
          content: '{"exit_code": 0, "stdout": "On branch main"}',
          timestamp: 1050,
          extra: { tool_call_id: 'call_r2_a', name: 'exec_command' },
        },
        {
          id: 'm6',
          role: 'assistant',
          content: 'Audit complete: Cargo.toml and src/lib.rs are clean.',
          timestamp: 1060,
          extra: {
            model: 'claude-sonnet-4-6',
            reasoning_content: 'Synthesizing final summary',
          },
        },
        // Turn 2: Direct Q&A without tools
        { id: 'm7', role: 'user', content: 'Which branch was that?', timestamp: 2000 },
        { id: 'm8', role: 'assistant', content: 'You are on `main`.', timestamp: 2010 },
        // Turn 3: 3 empty-content tool rounds + 1 genuine tool error + final table answer
        { id: 'm9', role: 'user', content: 'hii so whats new', timestamp: 3000 },
        {
          id: 'm10',
          role: 'assistant',
          content: '',
          timestamp: 3010,
          extra: {
            reasoning_content: 'Checking git log',
            tool_calls: [{ id: 'call_t3_1', function: { name: 'git_status', arguments: '{}' } }],
          },
        },
        {
          id: 'm11',
          role: 'tool',
          content: '{"branch":"main"}',
          timestamp: 3020,
          extra: { tool_call_id: 'call_t3_1', name: 'git_status' },
        },
        {
          id: 'm12',
          role: 'assistant',
          content: '',
          timestamp: 3030,
          extra: {
            reasoning_content: 'Checking protected file',
            tool_calls: [{ id: 'call_t3_2', function: { name: 'read_file', arguments: '{"path":"/root/secret"}' } }],
          },
        },
        {
          id: 'm13',
          role: 'tool',
          content: '{"error":"Execution denied by user."}',
          timestamp: 3040,
          extra: { tool_call_id: 'call_t3_2', name: 'read_file' },
        },
        {
          id: 'm14',
          role: 'assistant',
          content: '',
          timestamp: 3050,
          extra: {
            reasoning_content: 'Checking CHANGELOG.md',
            tool_calls: [{ id: 'call_t3_3', function: { name: 'read_file', arguments: '{"path":"CHANGELOG.md"}' } }],
          },
        },
        {
          id: 'm15',
          role: 'tool',
          content: '{"content":"## [0.0.345]"}',
          timestamp: 3060,
          extra: { tool_call_id: 'call_t3_3', name: 'read_file' },
        },
        {
          id: 'm16',
          role: 'assistant',
          content: 'Hey Aswin. Three version layers, do not mix them.',
          timestamp: 3070,
        },
      ],
    });

    const restored = useOpenZStore.getState().messages['ws:resume-multi'] || [];
    // Exactly 3 user prompts + 3 coalesced assistant turns = 6 messages (no ghost bubbles!)
    expect(restored.map((m) => m.role)).toEqual([
      'user',
      'assistant',
      'user',
      'assistant',
      'user',
      'assistant',
    ]);

    // Verify Turn 1 routed pre-tool narration into narrationSegments, kept final answer clean, and ordered all 3 tools
    const turn1Assistant = restored[1]!;
    expect(turn1Assistant.content).toBe(
      'Audit complete: Cargo.toml and src/lib.rs are clean.',
    );
    expect(turn1Assistant.narrationSegments?.map((s) => s.content)).toEqual([
      'Inspecting workspace files first.',
    ]);
    expect(turn1Assistant.reasoningContent).toBe(
      'Plan round 1\n\nPlan round 2\n\nSynthesizing final summary',
    );
    expect(turn1Assistant.toolCalls?.map((t) => t.name)).toEqual([
      'read_file',
      'grep_search',
      'exec_command',
    ]);
    expect(turn1Assistant.toolCalls?.map((t) => t.status)).toEqual([
      'success',
      'success',
      'success',
    ]);
    expect(turn1Assistant.timestamp).toBeLessThan(turn1Assistant.toolCalls![0]!.startedAt!);
    expect(turn1Assistant.toolCalls![0]!.startedAt!).toBeLessThan(
      turn1Assistant.toolCalls![1]!.startedAt!,
    );
    expect(turn1Assistant.toolCalls![1]!.startedAt!).toBeLessThan(
      turn1Assistant.toolCalls![2]!.startedAt!,
    );

    // Verify Turn 2 preserved
    expect(restored[2]?.content).toBe('Which branch was that?');
    expect(restored[3]?.content).toBe('You are on `main`.');

    // Verify Turn 3 coalesced all 3 tool rounds + final answer and accurately marked only the denied tool as error
    const turn3Assistant = restored[5]!;
    expect(turn3Assistant.content).toBe('Hey Aswin. Three version layers, do not mix them.');
    expect(turn3Assistant.toolCalls).toHaveLength(3);
    expect(turn3Assistant.toolCalls?.map((t) => t.status)).toEqual([
      'success',
      'error',
      'success',
    ]);
    expect(turn3Assistant.toolCalls?.[1]?.error).toBe('Execution denied by user.');
  } finally {
    wsService.attachChat = origAttach;
  }
});

test('live multi-round agentic turn keeps AgentActivity working monotonically across inter-tool gaps and folds pre-tool narration cleanly', async () => {
  ensureRegistered();
  const { isMessageActivityWorking } = await import('../features/chat/toolVisibility');
  const { buildMessageActivitySteps } = await import('../features/activity/activityAdapters');

  useOpenZStore.setState({
    sessions: [
      { id: 'ws:nvda-live', title: 'NVDA Research', createdAt: 1, lastMessageAt: 1, messageCount: 1, isDraft: false },
    ],
    activeChatId: 'ws:nvda-live',
    messages: {
      'ws:nvda-live': [
        { id: 'u1', role: 'user', content: 'so can u research nvidia current stock value', timestamp: 10 },
        { id: 'a1', role: 'assistant', content: '', timestamp: 11, isStreaming: true },
      ],
    },
    activeTurnIds: {},
    isStreaming: true,
  });

  const emit = (wsService as unknown as { emit: (event: string, payload: unknown) => void }).emit.bind(wsService);
  const currentAssistant = () => useOpenZStore.getState().messages['ws:nvda-live']![1]!;

  emit('turn_started', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1' });
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  // Round 1: reasoning + tool_start + tool_end
  emit('reasoning_delta', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', content: 'Checking sources.' });
  emit('tool_start', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-1', name: 'web_search', args: { query: 'NVDA stock' } });
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  emit('tool_end', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-1', name: 'web_search', status: 'success', output: '{"results":[]}' });
  // Inter-tool gap #1: must remain working (never flap to complete!)
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  // Round 2: LLM streams pre-tool narration delta BEFORE calling next tool
  emit('reasoning_delta', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', content: 'Fetching quote.' });
  emit('delta', {
    chat_id: 'ws_nvda-live',
    turn_id: 'turn-nvda-1',
    content: 'Live pull. Getting exact quote + earnings detail.',
  });
  // Even while pre-tool narration sits in content before tool_start arrives, AgentActivity stays working!
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  emit('tool_start', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-2', name: 'web_fetch', args: { url: 'https://stockanalysis.com/stocks/nvda' } });
  // Pre-tool narration folded out of content and into narrationSegments!
  expect(currentAssistant().content).toBe('');
  expect(currentAssistant().narrationSegments?.map((s) => s.content)).toEqual([
    'Live pull. Getting exact quote + earnings detail.',
  ]);
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  emit('tool_end', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-2', name: 'web_fetch', status: 'success', output: '{"fetched":1}' });
  // Inter-tool gap #2: still monotonically working!
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  // Round 3: Second pre-tool narration + bookmark persistence tool
  emit('delta', {
    chat_id: 'ws_nvda-live',
    turn_id: 'turn-nvda-1',
    content: 'Persisting evidence per skill protocol (tiered bookmarks + brief), then the briefing.',
  });
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);
  emit('tool_start', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-3', name: 'knowledge_source', args: { action: 'add' } });
  expect(currentAssistant().content).toBe('');
  expect(currentAssistant().narrationSegments).toHaveLength(2);
  emit('tool_end', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1', tool_call_id: 'tc-3', name: 'knowledge_source', status: 'success', output: '{"status":"ok"}' });
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  // Final synthesis round: streams clean final Markdown answer without pre-tool narration pollution
  emit('delta', {
    chat_id: 'ws_nvda-live',
    turn_id: 'turn-nvda-1',
    content: '**NVDA — live, Nov 18 2026**\n\n**$150.86**, −2.96% today.',
  });
  expect(isMessageActivityWorking(currentAssistant())).toBe(true);

  // Turn completes: transitions monotonically once from working -> complete
  emit('turn_end', { chat_id: 'ws_nvda-live', turn_id: 'turn-nvda-1' });
  const finalMsg = currentAssistant();
  expect(isMessageActivityWorking(finalMsg)).toBe(false);
  expect(finalMsg.content).toBe('**NVDA — live, Nov 18 2026**\n\n**$150.86**, −2.96% today.');
  expect(finalMsg.reasoningContent).toBe('Checking sources.\n\nFetching quote.');

  const steps = buildMessageActivitySteps([finalMsg], { chronological: true, limit: 250 });
  expect(steps.filter((s) => s.kind === 'message').map((s) => s.title)).toEqual([
    'Live pull. Getting exact quote + earnings detail.',
    'Persisting evidence per skill protocol (tiered bookmarks + brief), then the briefing.',
  ]);
});
