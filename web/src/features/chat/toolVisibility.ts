import type { AgentActivityStep } from '../../components/beui';
import type { ToolResultStyleChoice } from '../../components/assistant-ui/elements';
import type { OpenZMessage } from '../../types';

export type AssistantTurnPhase = 'idle' | 'working' | 'complete';

export interface ToolCardsVisibilityInput {
  toolResultStyle?: ToolResultStyleChoice;
  isActivityWorking: boolean;
  showInlineActivity: boolean;
  activityAutoCollapse: boolean;
  isActivityExpanded: boolean;
}

const PHASE_LOOKUP: Record<'working' | 'complete' | 'idle', AssistantTurnPhase> = {
  working: 'working',
  complete: 'complete',
  idle: 'idle',
};

/**
 * Pure monotonic phase derivation for an assistant turn:
 * - `'working'`: Any tool or guard is active, OR `isStreaming` is active on a turn that either
 *   has recorded agentic steps (reasoning, tools, narration, approvals, notices) or is awaiting
 *   its initial token. Once agentic steps exist, the turn remains `'working'` continuously
 *   across all inter-tool gaps and pre-tool narrations until `turn_end` settles `isStreaming`.
 * - `'complete'`: Turn has settled (`!isWorking`) and carries recorded steps.
 * - `'idle'`: User messages or plain direct-text replies with zero agentic steps.
 */
export function deriveAssistantTurnPhase(message: OpenZMessage): AssistantTurnPhase {
  const isAssistant = message.role !== 'user';
  const hasActiveTool = Boolean(
    message.toolCalls?.some(
      (tc) => tc.status === 'running' || tc.status === 'awaiting_approval',
    ),
  );
  const hasPendingGuard = Boolean(
    message.securityPrompts?.some((sp) => sp.status === 'pending'),
  );
  const hasRecordedSteps = Boolean(
    (message.toolCalls?.length ?? 0) > 0 ||
      (message.reasoningContent?.trim().length ?? 0) > 0 ||
      (message.narrationSegments?.length ?? 0) > 0 ||
      (message.activityNotices?.length ?? 0) > 0 ||
      (message.securityPrompts?.length ?? 0) > 0,
  );
  const isStreamingAgenticOrInitial =
    Boolean(message.isStreaming) && (hasRecordedSteps || message.content.trim().length === 0);
  const isWorking =
    isAssistant && (hasActiveTool || hasPendingGuard || isStreamingAgenticOrInitial);
  const phaseKey = isWorking
    ? 'working'
    : isAssistant && hasRecordedSteps
      ? 'complete'
      : 'idle';

  return PHASE_LOOKUP[phaseKey];
}

export function isMessageActivityWorking(message: OpenZMessage): boolean {
  return deriveAssistantTurnPhase(message) === 'working';
}

export function filterInlineStepsForToolStyle(
  steps: ReadonlyArray<AgentActivityStep>,
  toolResultStyle: ToolResultStyleChoice,
): AgentActivityStep[] {
  const keepUnifiedRows = toolResultStyle === 'beui-activity';
  return keepUnifiedRows
    ? [...steps]
    : steps.filter((step) => step.kind !== 'tool' && !step.id.includes('-tool-'));
}

export function shouldDisplayExpandedToolCalls({
  toolResultStyle = 'beui-activity',
  isActivityWorking,
  showInlineActivity,
  activityAutoCollapse,
  isActivityExpanded,
}: ToolCardsVisibilityInput): boolean {
  const isBeuiUnified = toolResultStyle === 'beui-activity';
  return isBeuiUnified
    ? !showInlineActivity && isActivityWorking
    : isActivityWorking || (showInlineActivity && (!activityAutoCollapse || isActivityExpanded));
}
