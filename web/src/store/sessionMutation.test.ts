import { expect, test } from 'bun:test';
import type { OpenZMessage, OpenZSession } from '../types';
import { applySessionMutationResult } from './sessionMutation';

const sessions: OpenZSession[] = [
  { id: 'chat-a', title: 'A', createdAt: 1, lastMessageAt: 2, messageCount: 1 },
  { id: 'chat-b', title: 'B', createdAt: 3, lastMessageAt: 4, messageCount: 0 },
];
const messages: Record<string, OpenZMessage[]> = {
  'chat-a': [{ id: 'msg-a', role: 'user', content: 'hello', timestamp: 1 }],
  'chat-b': [],
};

test('keeps a session visible when its archive or delete request is rejected', () => {
  const result = applySessionMutationResult(sessions, messages, 'chat-a', false);

  expect(result.sessions).toBe(sessions);
  expect(result.messages).toBe(messages);
  expect(result.sessions.map((session) => session.id)).toEqual(['chat-a', 'chat-b']);
});

test('removes a session only after the gateway confirms the mutation', () => {
  const result = applySessionMutationResult(sessions, messages, 'chat-a', true);

  expect(result.sessions.map((session) => session.id)).toEqual(['chat-b']);
  expect(result.messages['chat-a']).toBeUndefined();
  expect(result.messages['chat-b']).toEqual([]);
});
