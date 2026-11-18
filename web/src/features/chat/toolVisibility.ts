import type { AgentActivityStep } from '../../components/beui';
import type { ToolResultStyleChoice } from '../../components/assistant-ui/elements';
import type { OpenZMessage } from '../../types';

export interface ToolCardsVisibilityInput {
  toolResultStyle?: ToolResultStyleChoice;
  isActivityWorking: boolean;
  showInlineActivity: boolean;
  activityAutoCollapse: boolean;
  isActivityExpanded: boolean;
}

/**
 * Returns true while an assistant message is actively reasoning, executing tools,
 * or awaiting SecurityGuard approval — and flips to false as soon as tools finish
 * and final output text begins streaming (or the turn completes).
 */
export function isMessageActivityWorking(message: OpenZMessage): boolean {
  if (message.role === 'user') return false;

  const hasActiveTool = Boolean(
    message.toolCalls?.some(
      (tc) => tc.status === 'running' || tc.status === 'awaiting_approval',
    ),
  );
  const hasPendingGuard = Boolean(
    message.securityPrompts?.some((sp) => sp.status === 'pending'),
  );

  return (
    hasActiveTool ||
    hasPendingGuard ||
    (Boolean(message.isStreaming) && message.content.trim().length === 0)
  );
}

/**
 * Prevents duplicate tool call rendering between `@beui/agent-activity` and
 * `@assistant-ui/elements-tool-call`:
 * - In default `beui-activity` mode, `@beui/agent-activity` renders all tool rows (`📄 Read ...`)
 *   and standalone `ToolExecutionCard` rows are suppressed.
 * - In interactive `@assistant-ui` modes (`assistant-collapsed`, `assistant-error-open`,
 *   `assistant-expanded`), tool items are omitted from inline `AgentActivity` so only the
 *   interactive `ToolExecutionCard` rows appear.
 */
export function filterInlineStepsForToolStyle(
  steps: ReadonlyArray<AgentActivityStep>,
  toolResultStyle: ToolResultStyleChoice,
): AgentActivityStep[] {
  if (toolResultStyle === 'beui-activity') {
    return [...steps];
  }
  return steps.filter(
    (step) => step.kind !== 'tool' && !step.id.includes('-tool-'),
  );
}

/**
 * Controls whether standalone `ToolExecutionCard` rows are shown in the message body:
 * - Never duplicated in default `beui-activity` mode when inline `AgentActivity` handles tools.
 * - In `@assistant-ui` interactive card modes, visible live while tools are executing
 *   (`isActivityWorking === true`), then auto-hidden once output starts streaming or completes
 *   unless manually expanded (`isActivityExpanded`).
 */
export function shouldDisplayExpandedToolCalls({
  toolResultStyle = 'beui-activity',
  isActivityWorking,
  showInlineActivity,
  activityAutoCollapse,
  isActivityExpanded,
}: ToolCardsVisibilityInput): boolean {
  if (toolResultStyle === 'beui-activity') {
    return !showInlineActivity && isActivityWorking;
  }
  if (isActivityWorking) return true;
  if (!showInlineActivity) return false;
  if (!activityAutoCollapse) return true;
  return isActivityExpanded;
}
