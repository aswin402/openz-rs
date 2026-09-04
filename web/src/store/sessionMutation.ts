import type { OpenZMessage, OpenZSession } from '../types';

export interface SessionMutationResult {
  sessions: OpenZSession[];
  messages: Record<string, OpenZMessage[]>;
}

export function applySessionMutationResult(
  sessions: OpenZSession[],
  messages: Record<string, OpenZMessage[]>,
  chatId: string,
  confirmed: boolean,
): SessionMutationResult {
  if (!confirmed) return { sessions, messages };

  const nextMessages = { ...messages };
  delete nextMessages[chatId];
  return {
    sessions: sessions.filter((session) => session.id !== chatId),
    messages: nextMessages,
  };
}
