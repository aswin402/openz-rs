import { describe, expect, it } from 'bun:test';
import type { AgentActivityStep } from '../../components/beui';
import type { OpenZMessage } from '../../types';
import {
  filterInlineStepsForToolStyle,
  isMessageActivityWorking,
  shouldDisplayExpandedToolCalls,
} from './toolVisibility';

describe('Zero-Duplication Tool Call Customization & Auto-Collapse', () => {
  const sampleSteps: AgentActivityStep[] = [
    {
      id: 'm1-reasoning',
      kind: 'reasoning',
      title: 'Thought process',
      status: 'info',
    },
    {
      id: 'm1-tool-t1',
      kind: 'tool',
      title: 'openz_inventory',
      toolAction: 'read',
      toolTarget: 'openz_inventory',
      status: 'success',
    },
  ];

  it('defaults to beui-activity row (Read openz_inventory) with zero standalone duplicate cards', () => {
    const steps = filterInlineStepsForToolStyle(sampleSteps, 'beui-activity');
    expect(steps.length).toBe(2);
    expect(steps[1]?.toolTarget).toBe('openz_inventory');

    expect(
      shouldDisplayExpandedToolCalls({
        toolResultStyle: 'beui-activity',
        isActivityWorking: true,
        showInlineActivity: true,
        activityAutoCollapse: true,
        isActivityExpanded: true,
      }),
    ).toBe(false);
  });

  it('omits beui tool rows in interactive assistant-ui modes and auto-hides cards on output/completion', () => {
    const steps = filterInlineStepsForToolStyle(sampleSteps, 'assistant-collapsed');
    expect(steps.map((s) => s.id)).toEqual(['m1-reasoning']);

    const runningTurn: OpenZMessage = {
      id: 'm1',
      role: 'assistant',
      content: '',
      timestamp: 1000,
      isStreaming: true,
      toolCalls: [
        { id: 't1', name: 'openz_inventory', status: 'running', args: { include_subagents: false } },
      ],
    };

    const workingDuringTools = isMessageActivityWorking(runningTurn);
    expect(workingDuringTools).toBe(true);
    expect(
      shouldDisplayExpandedToolCalls({
        toolResultStyle: 'assistant-collapsed',
        isActivityWorking: workingDuringTools,
        showInlineActivity: true,
        activityAutoCollapse: true,
        isActivityExpanded: false,
      }),
    ).toBe(true);

    // Output starts streaming: auto-hides immediately
    const streamingFinalOutput: OpenZMessage = {
      ...runningTurn,
      content: 'Hey, I am Mivi.',
      toolCalls: [
        { id: 't1', name: 'openz_inventory', status: 'success', args: { include_subagents: false } },
      ],
    };

    const workingDuringOutput = isMessageActivityWorking(streamingFinalOutput);
    expect(workingDuringOutput).toBe(false);
    expect(
      shouldDisplayExpandedToolCalls({
        toolResultStyle: 'assistant-collapsed',
        isActivityWorking: workingDuringOutput,
        showInlineActivity: true,
        activityAutoCollapse: true,
        isActivityExpanded: false,
      }),
    ).toBe(false);

    // Clicking "Completed N steps" expands interactive cards on demand
    expect(
      shouldDisplayExpandedToolCalls({
        toolResultStyle: 'assistant-collapsed',
        isActivityWorking: false,
        showInlineActivity: true,
        activityAutoCollapse: true,
        isActivityExpanded: true,
      }),
    ).toBe(true);
  });
});
