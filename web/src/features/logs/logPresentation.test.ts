import { describe, expect, it } from 'bun:test';
import type { LogEntry } from '../../types';
import {
  buildLogActivitySteps,
  buildUnifiedDashboardActivitySteps,
} from '../activity/activityAdapters';
import {
  classifyLogScope,
  extractLogMessageAndFields,
  prepareScopedLogs,
  presentLogItem,
} from './logPresentation';

describe('WebUI Scoped & High-Signal Logs (v0.0.339)', () => {
  it('strictly separates active session, background cron/curator, other sessions, and gateway system logs', () => {
    const activeChat = 'ws:chat-101';

    const sessionTool: LogEntry = {
      id: '1',
      timestamp: '01:00:01',
      level: 'INFO',
      category: 'tool',
      target: 'openz::agent::agent_loop::tool_execution',
      message: 'Tool call completed  status=success tool=read_file',
      session: 'ws:chat-101',
    };
    const cronHeartbeat: LogEntry = {
      id: '2',
      timestamp: '01:00:02',
      level: 'INFO',
      category: 'cron',
      target: 'openz::cron::scheduler',
      message: 'cron job completed successfully  job_id=heartbeat_audit_5m',
      session: 'cron:heartbeat_audit_5m',
    };
    const curatorBg: LogEntry = {
      id: '3',
      timestamp: '01:00:03',
      level: 'INFO',
      category: 'curator',
      target: 'openz::agent::agent_loop::save',
      message: 'Self-improvement curator: started processing.',
      session: 'ws:chat-101',
    };
    const otherChat: LogEntry = {
      id: '4',
      timestamp: '01:00:04',
      level: 'INFO',
      category: 'session',
      target: 'openz::agent::agent_loop::run',
      message: 'Received LLM response (finish_reason: stop)  duration_ms=512',
      session: 'ws:chat-999',
    };
    const gatewaySys: LogEntry = {
      id: '5',
      timestamp: '01:00:05',
      level: 'INFO',
      category: 'channel',
      target: 'openz::channels::websocket',
      message: 'Gateway server bound to 127.0.0.1:8765',
    };

    expect(classifyLogScope(sessionTool, activeChat)).toBe('session');
    expect(classifyLogScope(sessionTool, 'chat-101')).toBe('session');
    expect(classifyLogScope(cronHeartbeat, activeChat)).toBe('background');
    expect(classifyLogScope(curatorBg, activeChat)).toBe('background');
    expect(classifyLogScope(otherChat, activeChat)).toBe('other_session');
    expect(classifyLogScope(gatewaySys, activeChat)).toBe('system');
  });

  it('extracts inline escaped fields cleanly without regex and merges with structured fields', () => {
    const entry: LogEntry = {
      id: '10',
      timestamp: '01:02:00',
      level: 'INFO',
      category: 'tool',
      target: 'openz::agent::agent_loop::run',
      message:
        'Executing tool call  arguments="{\\"command\\":\\"cargo check\\"}" tool=exec_command',
      session: 'ws:chat-1',
    };

    const parsed = extractLogMessageAndFields(entry);
    expect(parsed.baseMessage).toBe('Executing tool call');
    expect(parsed.fields.tool).toBe('exec_command');
    expect(parsed.fields.arguments).toBe('{"command":"cargo check"}');
  });

  it('merges tool start + completion and hides boilerplate in Essential mode while keeping warnings/errors', () => {
    // Ordered newest-first like logs.db ORDER BY id DESC
    const rawTurnLogs: LogEntry[] = [
      {
        id: '6',
        timestamp: '01:03:06',
        level: 'INFO',
        category: 'session',
        target: 'openz::agent::agent_loop::save',
        message: 'Session saved successfully. Turn complete.',
        session: 'ws:chat-1',
      },
      {
        id: '5',
        timestamp: '01:03:05',
        level: 'INFO',
        category: 'session',
        target: 'openz::agent::agent_loop::run',
        message:
          'Received LLM response (finish_reason: stop)  duration_ms=420 has_content=true tool_calls=0',
        session: 'ws:chat-1',
        fields: { duration_ms: '420', has_content: 'true', tool_calls: '0' },
      },
      {
        id: '4',
        timestamp: '01:03:04',
        level: 'INFO',
        category: 'tool',
        target: 'openz::agent::agent_loop::tool_execution',
        message: 'Tool call completed  status=success tool=exec_command',
        session: 'ws:chat-1',
        fields: { status: 'success', tool: 'exec_command' },
      },
      {
        id: '3',
        timestamp: '01:03:03',
        level: 'INFO',
        category: 'tool',
        target: 'openz::agent::agent_loop::run',
        message:
          'Executing tool call  arguments="{\\"command\\":\\"git status -sb\\"}" tool=exec_command',
        session: 'ws:chat-1',
        fields: { arguments: '{"command":"git status -sb"}', tool: 'exec_command' },
      },
      {
        id: '2',
        timestamp: '01:03:02',
        level: 'INFO',
        category: 'session',
        target: 'openz::agent::agent_loop::run',
        message: 'Sending completion request to LLM (model: MiniMax-M3.1-Flash)  iteration=0',
        session: 'ws:chat-1',
      },
      {
        id: '1',
        timestamp: '01:03:01',
        level: 'INFO',
        category: 'session',
        target: 'openz::agent::agent_loop::restore',
        message: 'Restored session history (4 messages). User prompt: "check git"',
        session: 'ws:chat-1',
      },
      {
        id: '0',
        timestamp: '01:03:00',
        level: 'WARN',
        category: 'security',
        target: 'openz::agent::security',
        message: 'Elevated command required user confirmation',
        session: 'ws:chat-1',
      },
    ];

    const { items, hiddenCount } = prepareScopedLogs(rawTurnLogs, {
      essentialOnly: true,
    });

    // Kept: Reply completed (#5), Merged tool completion (#4), Security warning (#0)
    // Hidden (4): Session saved (#6), Executing tool call (#3), Sending request (#2), Restored (#1)
    expect(hiddenCount).toBe(4);
    expect(items.length).toBe(3);

    const replyPresented = presentLogItem(items[0]);
    expect(replyPresented.title).toBe('Assistant reply completed');
    expect(replyPresented.detail).toContain('420ms');

    const toolPresented = presentLogItem(items[1]);
    expect(toolPresented.title).toBe('exec_command');
    expect(toolPresented.detail).toBe('git status -sb');
    expect(toolPresented.status).toBe('success');

    const warnPresented = presentLogItem(items[2]);
    expect(warnPresented.status).toBe('pending');
  });

  it('excludes background cron logs from Dashboard unified activity steps', () => {
    const steps = buildUnifiedDashboardActivitySteps({
      messagesByChat: {},
      activeChatId: 'ws:chat-1',
      orchestrationRunsByChat: {},
      logs: [
        {
          id: 'c1',
          timestamp: '2026-11-18T01:05:00Z',
          level: 'INFO',
          category: 'cron',
          target: 'openz::cron::scheduler',
          message: 'cron job completed successfully  job_id=heartbeat_audit_5m',
          session: 'cron:heartbeat_audit_5m',
        },
        {
          id: 'g1',
          timestamp: '2026-11-18T01:06:00Z',
          level: 'INFO',
          category: 'channel',
          target: 'openz::gateway',
          message: 'Gateway listening',
        },
      ],
    });

    expect(steps.length).toBe(1);
    expect(steps[0].detail).toBe('Gateway listening');

    const bgSteps = buildLogActivitySteps(
      [
        {
          id: 'c1',
          timestamp: '2026-11-18T01:05:00Z',
          level: 'INFO',
          category: 'cron',
          target: 'openz::cron::scheduler',
          message: 'cron job completed successfully  job_id=heartbeat_audit_5m',
          session: 'cron:heartbeat_audit_5m',
        },
      ],
      { essentialOnly: true },
    );
    expect(bgSteps.length).toBe(1);
    expect(bgSteps[0].title).toBe('Cron · heartbeat_audit_5m');
    expect(bgSteps[0].status).toBe('success');
  });
});
