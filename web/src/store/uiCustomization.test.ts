import { beforeEach, describe, expect, test } from 'bun:test';
import {
  DEFAULT_CUSTOMIZATION,
  PREBUILT_THEMES,
  UI_CUSTOMIZATION_BACKUP_KEY,
  UI_CUSTOMIZATION_STORAGE_KEY,
  extractSnapshot,
  registerUiCustomizationSyncListener,
  resolveActivePalette,
  useUiCustomizationStore,
} from './useUiCustomizationStore';
import {
  contrastRatio,
  deriveCategoricalPalette,
  relativeLuminance,
} from '../shared/lib/colorEngine';
import {
  estimateTextTokens,
  extractGatewayEndpointLabel,
  mimePolicyToAcceptAttr,
  resolveModelContextWindow,
} from '../shared/lib/runtimeMetrics';
import {
  APPROVAL_CARD_STYLE_OPTIONS,
  TOOL_CALL_STYLE_OPTIONS,
  formatApprovalArgs,
  isDestructiveToolPrompt,
  mapSecurityPromptToApprovalData,
  mapToolExecutionToToolCallData,
  normalizeApprovalStyle,
  normalizeToolResultStyle,
  pairSecurityPromptsWithToolCalls,
  summarizeApprovalInline,
} from '../components/assistant-ui/elements';
import {
  ACTIVITY_DENSITY_OPTIONS,
  ACTIVITY_MAX_HEIGHT_OPTIONS,
  ACTIVITY_ORB_OPTIONS,
  ACTIVITY_PLACEMENT_OPTIONS,
  ACTIVITY_VARIANT_OPTIONS,
  AGENT_AVATAR_OPTIONS,
  INLINE_NOTICE_STYLE_OPTIONS,
  TOAST_DURATION_OPTIONS,
  TOAST_MAX_VISIBLE_OPTIONS,
  TOAST_POSITION_OPTIONS,
  TOAST_VARIANT_OPTIONS,
  USER_AVATAR_OPTIONS,
  computeCumulativeExpandedOffsets,
  computeToastStackGeometry,
  extractAllMarkdownSources,
  extractMarkdownCitations,
  extractToolImagePayload,
  mapNoticeKindToToastCategory,
  normalizeToastPosition,
  normalizeToastVariant,
  orchestrationRunToSubagentProps,
  parseCitationSegments,
  parseUnifiedDiff,
  resolveActivityPhase,
  resolveMaxHeightPx,
  resolveStepContentType,
  resolveStepIcon,
  resolveToastDurationMs,
  toBeuiOfficialActivityItems,
  tokenizeDiffLine,
} from '../components/beui';
import {
  TOAST_CATEGORY_TOGGLE_OPTIONS,
  dispatchActivityNoticeToast,
  resolveNoticeJumpAction,
  shouldEmitCategoryToast,
  useToastStore,
} from './useToastStore';
import {
  buildInventoryActivitySteps,
  buildLogActivitySteps,
  buildMessageActivitySteps,
  buildOrchestrationActivitySteps,
  buildUnifiedDashboardActivitySteps,
  classifyBeuiToolAction,
  classifyToolContentType,
  formatBeuiToolTarget,
  stripTerminalFormatting,
} from '../features/activity/activityAdapters';
import { interpolateTemplate } from '../lib/i18n';
import { FOX_SILHOUETTE_PATH, buildFoxFaviconDataUri } from '../components/OpenZFoxIcon';

describe('useUiCustomizationStore (@beui + shadcn/ui v5)', () => {
  beforeEach(() => {
    useUiCustomizationStore.getState().resetAllCustomization();
    useUiCustomizationStore.getState().setIsCustomizeOpen(false);
    useUiCustomizationStore.getState().setIsCommandPaletteOpen(false);
  });

  test('defaults to 100% @beui components, classic solid OpenZ Fox silhouette avatar, and icon-only Send button', () => {
    const state = useUiCustomizationStore.getState();
    expect(state.activeThemeId).toBe('obsidian-ember');
    expect(state.chatWidth).toBe('standard');
    expect(state.promptInputStyle).toBe('elevated');
    expect(state.sendButtonStyle).toBe('icon-round');
    expect(state.modelSelectorStyle).toBe('command');
    expect(state.promptSuggestionsStyle).toBe('bento');
    expect(state.contextUsageStyle).toBe('ring-pill');
    expect(state.scrollerBehavior).toBe('beui-smart');
    expect(state.agentAvatar).toBe('fox-classic');
    expect(state.userAvatar).toBe('user-default');
    expect(AGENT_AVATAR_OPTIONS.length).toBe(13);
    expect(USER_AVATAR_OPTIONS.length).toBe(6);
    expect(state.bubbleStyle).toBe('beui-card');
    expect(state.messageActionsStyle).toBe('floating');
    expect(state.showMessageActions).toBe(true);
    expect(state.streamingAnimation).toBe('beui-pulse');
    expect(state.thinkingAnimation).toBe('card');
    expect(state.activityStyle).toBe('beui-timeline');
    expect(state.activityVariant).toBe('orb-card');
    expect(state.activityOrbStyle).toBe('dotted-orb');
    expect(state.activityDensity).toBe('comfortable');
    expect(state.activityMaxHeight).toBe('standard');
    expect(state.activityAutoCollapse).toBe(true);
    expect(state.activityShowDetails).toBe(true);
    expect(state.showDashboardActivity).toBe(true);
    expect(state.planChecklistStyle).toBe('card');
    expect(state.subagentCardStyle).toBe('card');
    expect(state.codeStyle).toBe('framed');
    expect(state.diffLayout).toBe('unified');
    expect(state.terminalStyle).toBe('console');
    expect(state.toolResultStyle).toBe('assistant-collapsed');
    expect(state.approvalStyle).toBe('assistant-default');
    expect(state.imageGenStyle).toBe('beui-frame');
    expect(state.citationsStyle).toBe('pills');
    expect(state.dataTableStyle).toBe('interactive');
    expect(state.jsonViewerStyle).toBe('tree');
    expect(state.alertStyle).toBe('beui-alert');
    expect(state.collapsibleStyle).toBe('subtle');
    expect(state.toastEnabled).toBe(true);
    expect(state.toastVariant).toBe('beui-stack');
    expect(state.toastPosition).toBe('bottom-end');
    expect(state.toastDuration).toBe('standard');
    expect(state.toastMaxVisible).toBe(4);
    expect(state.inlineNoticeStyle).toBe('pills');
    expect(state.toastNotifyWorkflow).toBe(true);
    expect(state.toastNotifySkill).toBe(true);
    expect(state.toastNotifySelfImprovement).toBe(true);
    expect(state.toastNotifyMemory).toBe(true);
    expect(state.toastNotifyResearch).toBe(true);
    expect(state.toastNotifyApproval).toBe(true);
  });

  test('ships 13 dual-mode theme families where every theme supports both Light and Dark modes with WCAG contrast', () => {
    expect(PREBUILT_THEMES.length).toBe(13);
    for (const family of PREBUILT_THEMES) {
      expect(family.dark.mode).toBe('dark');
      expect(family.light.mode).toBe('light');
      expect(family.dark.primaryHex.startsWith('#')).toBe(true);
      expect(family.light.primaryHex.startsWith('#')).toBe(true);
      expect(family.dark.backgroundHex.startsWith('#')).toBe(true);
      expect(family.light.backgroundHex.startsWith('#')).toBe(true);
      expect(relativeLuminance(family.light.backgroundHex)).toBeGreaterThan(
        relativeLuminance(family.dark.backgroundHex),
      );
      expect(contrastRatio(family.dark.foregroundHex, family.dark.backgroundHex)).toBeGreaterThanOrEqual(4.5);
      expect(contrastRatio(family.light.foregroundHex, family.light.backgroundHex)).toBeGreaterThanOrEqual(4.5);

      const resolvedDark = resolveActivePalette(family.id, 'dark');
      const resolvedLight = resolveActivePalette(family.id, 'light');
      expect(resolvedDark.id).toBe(family.id);
      expect(resolvedLight.id).toBe(family.id);
      expect(resolvedDark.mode).toBe('dark');
      expect(resolvedLight.mode).toBe('light');
    }
  });

  test('commits selected @beui variants via applyCustomization and resolves both dark and light palettes cleanly', () => {
    const store = useUiCustomizationStore.getState();

    store.setIsCustomizeOpen(true);
    expect(useUiCustomizationStore.getState().isCustomizeOpen).toBe(true);

    store.applyCustomization({
      activeThemeId: 'tokyo-night',
      chatWidth: 'wide',
      promptInputStyle: 'split',
      sendButtonStyle: 'arrow-round',
      modelSelectorStyle: 'provider-badge',
      promptSuggestionsStyle: 'pills',
      contextUsageStyle: 'bar',
      scrollerBehavior: 'always-follow',
      agentAvatar: 'bot-droid',
      userAvatar: 'user-dev',
      bubbleStyle: 'beui-full',
      messageActionsStyle: 'ghost',
      showMessageActions: false,
      streamingAnimation: 'block-cursor',
      thinkingAnimation: 'inline',
      activityStyle: 'streaming-only',
      activityVariant: 'trace-log',
      activityOrbStyle: 'fox-pulse',
      activityDensity: 'compact',
      activityMaxHeight: 'tall',
      activityAutoCollapse: false,
      activityShowDetails: false,
      showDashboardActivity: false,
      planChecklistStyle: 'compact-steps',
      subagentCardStyle: 'tree',
      codeStyle: 'minimal',
      diffLayout: 'split',
      terminalStyle: 'minimal',
      toolResultStyle: 'assistant-error-open',
      approvalStyle: 'assistant-receipt',
      imageGenStyle: 'minimal',
      citationsStyle: 'cards',
      dataTableStyle: 'striped',
      jsonViewerStyle: 'lined',
      alertStyle: 'compact-pill',
      collapsibleStyle: 'bordered',
    });

    const updated = extractSnapshot(useUiCustomizationStore.getState());
    expect(updated.activeThemeId).toBe('tokyo-night');
    expect(updated.chatWidth).toBe('wide');
    expect(updated.promptInputStyle).toBe('split');
    expect(updated.sendButtonStyle).toBe('arrow-round');
    expect(updated.modelSelectorStyle).toBe('provider-badge');
    expect(updated.promptSuggestionsStyle).toBe('pills');
    expect(updated.contextUsageStyle).toBe('bar');
    expect(updated.scrollerBehavior).toBe('always-follow');
    expect(updated.agentAvatar).toBe('bot-droid');
    expect(updated.userAvatar).toBe('user-dev');
    expect(updated.bubbleStyle).toBe('beui-full');
    expect(updated.messageActionsStyle).toBe('ghost');
    expect(updated.showMessageActions).toBe(false);
    expect(updated.streamingAnimation).toBe('block-cursor');
    expect(updated.thinkingAnimation).toBe('inline');
    expect(updated.activityStyle).toBe('streaming-only');
    expect(updated.activityVariant).toBe('trace-log');
    expect(updated.activityOrbStyle).toBe('fox-pulse');
    expect(updated.activityDensity).toBe('compact');
    expect(updated.activityMaxHeight).toBe('tall');
    expect(updated.activityAutoCollapse).toBe(false);
    expect(updated.activityShowDetails).toBe(false);
    expect(updated.showDashboardActivity).toBe(false);
    expect(updated.planChecklistStyle).toBe('compact-steps');
    expect(updated.subagentCardStyle).toBe('tree');
    expect(updated.codeStyle).toBe('minimal');
    expect(updated.diffLayout).toBe('split');
    expect(updated.terminalStyle).toBe('minimal');
    expect(updated.toolResultStyle).toBe('assistant-error-open');
    expect(updated.approvalStyle).toBe('assistant-receipt');
    expect(updated.imageGenStyle).toBe('minimal');
    expect(updated.citationsStyle).toBe('cards');
    expect(updated.dataTableStyle).toBe('striped');
    expect(updated.jsonViewerStyle).toBe('lined');
    expect(updated.alertStyle).toBe('compact-pill');
    expect(updated.collapsibleStyle).toBe('bordered');

    const darkPalette = resolveActivePalette(updated.activeThemeId, 'dark');
    const lightPalette = resolveActivePalette(updated.activeThemeId, 'light');
    expect(darkPalette.id).toBe('tokyo-night');
    expect(lightPalette.id).toBe('tokyo-night');
    expect(darkPalette.mode).toBe('dark');
    expect(lightPalette.mode).toBe('light');

    useUiCustomizationStore.getState().resetAllCustomization();
    expect(extractSnapshot(useUiCustomizationStore.getState())).toEqual(DEFAULT_CUSTOMIZATION);
  });

  test('computes categorical color series and runtime metrics deterministically without hardcoded constants', () => {
    const obsidianDark = resolveActivePalette('obsidian-ember', 'dark');
    const series = deriveCategoricalPalette(obsidianDark.primaryHex, 'dark', 10);
    expect(series.length).toBe(10);
    expect(new Set(series).size).toBe(10);

    expect(estimateTextTokens('fn main() { println!("openz"); }')).toBeGreaterThan(5);
    expect(resolveModelContextWindow('gemini-2.5-pro')).toBe(1_048_576);
    expect(resolveModelContextWindow('claude-sonnet-4-5')).toBe(200_000);
    expect(mimePolicyToAcceptAttr(['image/png', 'application/pdf'])).toBe(
      'image/png,application/pdf,.pdf',
    );
    expect(extractGatewayEndpointLabel('ws://127.0.0.1:8765/ws')).toBe('127.0.0.1:8765');
  });

  test('tracks Customize UI component menu sections, Gateway Settings sections, Command Palette, and quoted reply state', () => {
    const store = useUiCustomizationStore.getState();
    expect(store.activeGatewaySection).toBe('agent');
    expect(store.activeCustomizeSection).toBe('theme');
    expect(store.isCommandPaletteOpen).toBe(false);

    store.setActiveGatewaySection('providers');
    store.setActiveCustomizeSection('code-block');
    store.setIsCommandPaletteOpen(true);
    expect(useUiCustomizationStore.getState().activeGatewaySection).toBe('providers');
    expect(useUiCustomizationStore.getState().activeCustomizeSection).toBe('code-block');
    expect(useUiCustomizationStore.getState().isCommandPaletteOpen).toBe(true);

    store.setQuotedReplyText('Check async cancellation safety');
    expect(useUiCustomizationStore.getState().quotedReplyText).toBe(
      'Check async cancellation safety',
    );

    store.resetAllCustomization();
    expect(useUiCustomizationStore.getState().quotedReplyText).toBeNull();
  });
});

describe('zero-regex @beui parsers & adapters', () => {
  test('extracts inline markdown links and citation footers for @beui/citations', () => {
    const withLinks =
      'Explore [BeUI Docs](https://beui.dev/components/agents/agent-activity) and [shadcn](https://ui.shadcn.com).';
    const extracted = extractAllMarkdownSources(withLinks);
    expect(extracted.refs.length).toBe(2);
    expect(extracted.refs[0].host).toBe('beui.dev');
    expect(extracted.refs[1].host).toBe('ui.shadcn.com');
  });

  test('detects image tool payloads for @beui/image-generation without regex', () => {
    const payload = extractToolImagePayload(
      'generate_image',
      { prompt: 'Full red-orange OpenZ fox badge' },
      '{"image_url":"data:image/png;base64,iVBORw0KGgo="}',
    );
    expect(payload.isImageTool).toBe(true);
    expect(payload.prompt).toBe('Full red-orange OpenZ fox badge');
    expect(payload.src).toBe('data:image/png;base64,iVBORw0KGgo=');
  });

  test('tokenizes code lines and parses unified diffs without regex', () => {
    const tokens = tokenizeDiffLine('const count = calc(42); // ok');
    expect(tokens.some((t) => t.t === 'kw' && t.v === 'const')).toBe(true);
    expect(tokens.some((t) => t.t === 'fn' && t.v === 'calc')).toBe(true);
    expect(tokens.some((t) => t.t === 'num' && t.v === '42')).toBe(true);
    expect(tokens.some((t) => t.t === 'cm')).toBe(true);

    const patch = [
      'diff --git a/src/lib.rs b/src/lib.rs',
      '--- a/src/lib.rs',
      '+++ b/src/lib.rs',
      '@@ -10,2 +10,2 @@',
      '-let old_val = 1;',
      '+let new_val = 2;',
    ].join('\n');
    const parsed = parseUnifiedDiff(patch);
    expect(parsed.file).toBe('src/lib.rs');
    expect(parsed.rows.length).toBe(2);
    expect(parsed.rows[0]?.type).toBe('del');
    expect(parsed.rows[1]?.type).toBe('add');
  });

  test('parses inline citations and markdown reference footers', () => {
    const md = [
      'Async Rust scales cleanly[1] and minimizes tail latency[2].',
      '',
      '[1]: Tokio Runtime Guide https://tokio.rs/tokio/tutorial',
      '[2]: Rust Async Book https://rust-lang.github.io/async-book/',
    ].join('\n');

    const extracted = extractMarkdownCitations(md);
    expect(extracted.hasInlineCitations).toBe(true);
    expect(extracted.refs.length).toBe(2);
    expect(extracted.refs[0].host).toBe('tokio.rs');

    const segments = parseCitationSegments(extracted.bodyText);
    expect(segments.filter((s) => s.kind === 'cite').length).toBe(2);
  });

  test('converts orchestration run state to @beui/subagent-card props', () => {
    const mapped = orchestrationRunToSubagentProps({
      id: 'run-42',
      mode: 'parallel',
      goal: 'Verify zero regex usage across WebUI',
      status: 'running',
      startedAt: 1000,
      steps: [
        {
          id: 'researcher',
          agent: 'researcher',
          status: 'completed',
          output: 'Verified zero regex usage',
        },
        {
          id: 'reviewer',
          agent: 'reviewer',
          status: 'running',
        },
      ],
    });

    expect(mapped.agentName).toBe('parallel');
    expect(mapped.mode).toBe('parallel');
    expect(mapped.goal).toBe('Verify zero regex usage across WebUI');
    expect(mapped.status).toBe('running');
    expect(mapped.steps?.length).toBe(2);
    expect(mapped.steps?.[0]?.status).toBe('completed');
    expect(mapped.steps?.[1]?.status).toBe('running');
  });

  test('exports and imports durable UI backup JSON and hydrates gateway snapshots without echo loops', () => {
    expect(UI_CUSTOMIZATION_STORAGE_KEY).toBe('openz-ui-customization');
    expect(UI_CUSTOMIZATION_BACKUP_KEY).toBe('openz-ui-customization-backup');

    const syncCalls: string[] = [];
    const unsubscribe = registerUiCustomizationSyncListener((snapshot) => {
      syncCalls.push(snapshot.activeThemeId);
    });

    useUiCustomizationStore.getState().applyCustomization({
      activeThemeId: 'catppuccin-mocha',
      agentAvatar: 'fox-ember',
      approvalStyle: 'assistant-receipt',
      codeStyle: 'minimal',
    });
    expect(syncCalls).toEqual(['catppuccin-mocha']);

    const exported = useUiCustomizationStore.getState().exportBackupJson({
      themeMode: 'light',
      gatewayDefaults: { streaming: true, caveman_mode: false },
    });

    useUiCustomizationStore.getState().resetAllCustomization();
    expect(useUiCustomizationStore.getState().activeThemeId).toBe('obsidian-ember');

    // Hydrating from Gateway must NOT re-trigger outbound sync listeners (prevents echo loops)
    const countBeforeHydrate = syncCalls.length;
    useUiCustomizationStore.getState().hydrateFromGateway(
      { codeStyle: 'minimal', bubbleStyle: 'editorial' },
      'tokyo-night',
    );
    expect(useUiCustomizationStore.getState().activeThemeId).toBe('tokyo-night');
    expect(useUiCustomizationStore.getState().bubbleStyle).toBe('editorial');
    expect(syncCalls.length).toBe(countBeforeHydrate);

    // Restoring exported backup JSON restores full theme + component state + themeMode
    const restored = useUiCustomizationStore.getState().importBackupJson(exported);
    expect(restored.ok).toBe(true);
    expect(restored.themeMode).toBe('light');
    expect(useUiCustomizationStore.getState().activeThemeId).toBe('catppuccin-mocha');
    expect(useUiCustomizationStore.getState().agentAvatar).toBe('fox-ember');
    expect(useUiCustomizationStore.getState().approvalStyle).toBe('assistant-receipt');

    // Legacy @beui backup values migrate seamlessly to @assistant-ui element choices
    const legacyBackup = JSON.stringify({
      ui_customization: {
        toolResultStyle: 'expanded-terminal',
        approvalStyle: 'beui-approval-card',
      },
    });
    expect(useUiCustomizationStore.getState().importBackupJson(legacyBackup).ok).toBe(true);
    expect(useUiCustomizationStore.getState().toolResultStyle).toBe('assistant-expanded');
    expect(useUiCustomizationStore.getState().approvalStyle).toBe('assistant-receipt');

    // Also accepts ~/.openz/webui_preferences.json format (`active_theme_id` + `ui_customization`)
    const serverDiskBackup = JSON.stringify({
      theme: 'dark',
      active_theme_id: 'nordic-frost',
      ui_customization: { chatWidth: 'full', diffLayout: 'split' },
    });
    const diskRestored = useUiCustomizationStore.getState().importBackupJson(serverDiskBackup);
    expect(diskRestored.ok).toBe(true);
    expect(diskRestored.themeMode).toBe('dark');
    expect(useUiCustomizationStore.getState().activeThemeId).toBe('nordic-frost');
    expect(useUiCustomizationStore.getState().chatWidth).toBe('full');
    expect(useUiCustomizationStore.getState().diffLayout).toBe('split');

    unsubscribe();
  });

  test('maps SecurityGuard prompts and ToolExecution calls to @assistant-ui elements cleanly without duplicate rows', () => {
    expect(TOOL_CALL_STYLE_OPTIONS.length).toBe(3);
    expect(APPROVAL_CARD_STYLE_OPTIONS.length).toBe(3);
    expect(normalizeToolResultStyle('beui-card')).toBe('assistant-collapsed');
    expect(normalizeToolResultStyle('expanded-terminal')).toBe('assistant-expanded');
    expect(normalizeApprovalStyle('beui-tool-approval')).toBe('assistant-default');
    expect(normalizeApprovalStyle('beui-approval-card')).toBe('assistant-receipt');

    const summaryFromArgs = summarizeApprovalInline({
      description: '',
      arguments: { action: 'list' },
    });
    expect(summaryFromArgs).toBe('action: list');

    const summaryFromDesc = summarizeApprovalInline({
      description: 'cargo check -p openz',
      arguments: { command: 'cargo check -p openz' },
    });
    expect(summaryFromDesc).toBe('cargo check -p openz');
    expect(formatApprovalArgs({ action: 'list' })).toContain('"action": "list"');

    expect(isDestructiveToolPrompt('exec_command', { command: 'rm -rf target/tmp' })).toBe(true);
    expect(isDestructiveToolPrompt('exec_command', { command: 'cargo check -p openz' })).toBe(false);

    const pendingApproval = mapSecurityPromptToApprovalData({
      id: 'sec-pending',
      toolName: 'exec_command',
      description: 'cargo check -p openz',
      arguments: { command: 'cargo check -p openz', workdir: '/workspace/openz' },
      status: 'pending',
    });
    expect(pendingApproval.state).toBe('request');
    expect(pendingApproval.command).toBe('$ cargo check -p openz');
    expect(pendingApproval.description).toBeUndefined();
    expect(pendingApproval.details).toEqual([{ label: 'workdir', value: '/workspace/openz' }]);
    expect(pendingApproval.variant).toBe('default');

    const mappedTool = mapToolExecutionToToolCallData({
      id: 'tc-1',
      name: 'exec_command',
      args: { command: 'echo "=== RSS NEWS ==="; curl -s https://example.com' },
      status: 'success',
      output: JSON.stringify({ stdout: '\x1b[1mHeadline OK\x1b[0m', status_code: 0 }),
      durationMs: 85,
    });
    expect(mappedTool.label).toBe('Ran exec_command');
    expect(mappedTool.activeLabel).toBe('Running exec_command');
    expect(mappedTool.query).toBe('$ echo "=== RSS NEWS ==="; curl -s https://example.com');
    expect(mappedTool.result).toContain('Headline OK');
    expect(mappedTool.running).toBe(false);
    expect(mappedTool.failed).toBe(false);

    // Lifecycle coordination: pending -> running -> completed handoff to ToolCall
    const cmdArgs = { command: 'curl -s https://example.com' };
    const stepPending = pairSecurityPromptsWithToolCalls(
      [{ id: 't1', name: 'exec_command', args: cmdArgs, status: 'running' }],
      [{ id: 'p1', toolName: 'exec_command', description: '', arguments: cmdArgs, status: 'pending' }],
    );
    expect(stepPending.visibleToolCalls.length).toBe(0);
    expect(stepPending.visibleApprovals.map((a) => a.state)).toEqual(['request']);

    const stepRunning = pairSecurityPromptsWithToolCalls(
      [{ id: 't1', name: 'exec_command', args: cmdArgs, status: 'running' }],
      [{ id: 'p1', toolName: 'exec_command', description: '', arguments: cmdArgs, status: 'approved' }],
    );
    expect(stepRunning.visibleToolCalls.length).toBe(0);
    expect(stepRunning.visibleApprovals.map((a) => a.state)).toEqual(['running']);

    const stepFinishedHandoff = pairSecurityPromptsWithToolCalls(
      [{ id: 't1', name: 'exec_command', args: cmdArgs, status: 'success', output: 'ok' }],
      [{ id: 'p1', toolName: 'exec_command', description: '', arguments: cmdArgs, status: 'approved' }],
    );
    expect(stepFinishedHandoff.visibleToolCalls.length).toBe(1);
    expect(stepFinishedHandoff.visibleApprovals.length).toBe(0);

    const stepFinishedWithReceipt = pairSecurityPromptsWithToolCalls(
      [{ id: 't1', name: 'exec_command', args: cmdArgs, status: 'success', output: 'ok' }],
      [{ id: 'p1', toolName: 'exec_command', description: '', arguments: cmdArgs, status: 'approved' }],
      { keepApprovedReceipt: true },
    );
    expect(stepFinishedWithReceipt.visibleToolCalls.length).toBe(1);
    expect(stepFinishedWithReceipt.visibleApprovals.map((a) => a.state)).toEqual(['done']);
  });

  test('builds cache-busting inline OpenZ Fox SVG favicon data URI for live browser tabs', () => {
    const defaultPalette = resolveActivePalette('obsidian-ember', 'dark');
    const dataUri = buildFoxFaviconDataUri(defaultPalette.primaryHex);
    expect(dataUri.startsWith('data:image/svg+xml;charset=utf-8,')).toBe(true);
    const decoded = decodeURIComponent(dataUri.slice('data:image/svg+xml;charset=utf-8,'.length));
    expect(decoded).toContain(defaultPalette.primaryHex);
    expect(decoded).toContain(FOX_SILHOUETTE_PATH);
  });

  test('resolves @beui/agent-activity phases, options, and multi-surface streams without regex', () => {
    expect(ACTIVITY_PLACEMENT_OPTIONS.length).toBe(4);
    expect(ACTIVITY_VARIANT_OPTIONS.length).toBe(4);
    expect(ACTIVITY_ORB_OPTIONS.length).toBe(3);
    expect(ACTIVITY_DENSITY_OPTIONS.length).toBe(2);
    expect(ACTIVITY_MAX_HEIGHT_OPTIONS.length).toBe(4);

    expect(resolveMaxHeightPx('compact')).toBe(176);
    expect(resolveMaxHeightPx('standard')).toBe(264);
    expect(resolveMaxHeightPx('tall')).toBe(384);
    expect(resolveMaxHeightPx('unlimited')).toBe(0);
    expect(resolveMaxHeightPx(220)).toBe(220);

    expect(classifyToolContentType('semantic_search')).toBe('search');
    expect(classifyToolContentType('cargo_check')).toBe('tool');

    const msgSteps = buildMessageActivitySteps(
      [
        {
          id: 'm1',
          role: 'assistant',
          content: '',
          timestamp: 1000,
          isStreaming: true,
          model: 'claude-sonnet-4-5',
          reasoningContent: 'Evaluating tool graph\nChecking types',
          toolCalls: [
            {
              id: 't1',
              name: 'ast_grep',
              args: { pattern: 'AgentActivity' },
              status: 'success',
              durationMs: 85,
              startedAt: 1100,
            },
            {
              id: 't2',
              name: 'delegate_task',
              args: { profile: 'reviewer', task: 'Verify UI' },
              status: 'running',
              startedAt: 1200,
            },
          ],
        },
      ],
      { chronological: true },
    );

    expect(msgSteps.length).toBe(3);
    expect(resolveStepContentType(msgSteps[1])).toBe('search');
    expect(msgSteps[2].actor).toBe('reviewer');
    expect(resolveActivityPhase(msgSteps, true)).toBe('tool');

    const orchSteps = buildOrchestrationActivitySteps(
      [
        {
          id: 'r1',
          goal: 'Audit WebUI surfaces',
          mode: 'parallel',
          status: 'running',
          startedAt: 2000,
          steps: [
            { id: 's1', agent: 'researcher', status: 'success', output: 'Done' },
            { id: 's2', agent: 'reviewer', status: 'running' },
          ],
        },
      ],
      { agentName: 'reviewer' },
    );
    expect(orchSteps.length).toBe(1);
    expect(orchSteps[0].actor).toBe('reviewer');

    const logSteps = buildLogActivitySteps(
      [
        {
          id: 'l1',
          timestamp: '2026-11-14T20:00:00Z',
          level: 'WARN',
          target: 'openz::channels::websocket',
          message: 'Client reconnected cleanly',
        },
      ],
      { levelFilter: 'WARN', query: 'reconnected' },
    );
    expect(logSteps.length).toBe(1);
    expect(logSteps[0].title).toBe('websocket');
    expect(logSteps[0].status).toBe('pending');

    const invSteps = buildInventoryActivitySteps(null, []);
    expect(invSteps.length).toBe(0);

    const unified = buildUnifiedDashboardActivitySteps({
      messagesByChat: {
        chat1: [
          {
            id: 'm2',
            role: 'assistant',
            content: 'Ready',
            timestamp: 3000,
            toolCalls: [{ id: 't3', name: 'web_search', status: 'success', durationMs: 120 }],
          },
        ],
      },
      activeChatId: 'chat1',
      orchestrationRunsByChat: {},
      logs: [
        {
          id: 'l2',
          timestamp: '2026-11-14T20:01:00Z',
          level: 'INFO',
          target: 'openz::gateway',
          message: 'Gateway listening',
        },
      ],
    });
    expect(unified.length).toBe(2);

    const skillNoticeSteps = buildMessageActivitySteps([
      {
        id: 'm3',
        role: 'assistant',
        content: 'Applied Rust zero-branching skill.',
        timestamp: 4000,
        activityNotices: [
          {
            id: 'n-skill-1',
            kind: 'skill',
            title: 'Skill used',
            detail: 'rust-pro',
            timestamp: 4005,
          },
        ],
      },
    ]);
    expect(skillNoticeSteps.length).toBe(1);
    expect(skillNoticeSteps[0]?.noticeKind).toBe('skill');
    expect(typeof resolveStepIcon(skillNoticeSteps[0]!)).toBe('object');
  });

  test('computes @beui/animated-toast-stack 3D deck geometry, cumulative heights, and telemetry routing', () => {
    useToastStore.getState().resetToastStore();
    useUiCustomizationStore.getState().resetAllCustomization();

    expect(TOAST_VARIANT_OPTIONS.length).toBe(10);
    expect(TOAST_VARIANT_OPTIONS.map((opt) => opt.id)).toEqual([
      'beui-stack',
      'compact-pills',
      'dynamic-island',
      'minimal-glass',
      'accent-banner',
      'terminal-hud',
      'split-action',
      'soft-tinted',
      'brutalist-outline',
      'micro-badge',
    ]);
    expect(TOAST_POSITION_OPTIONS.length).toBe(6);
    expect(TOAST_POSITION_OPTIONS.map((opt) => opt.id)).toEqual([
      'top-start',
      'top-center',
      'top-end',
      'bottom-start',
      'bottom-center',
      'bottom-end',
    ]);
    expect(normalizeToastPosition('bottom-right')).toBe('bottom-end');
    expect(normalizeToastPosition('bottom-left')).toBe('bottom-start');
    expect(normalizeToastPosition('top-right')).toBe('top-end');
    expect(normalizeToastPosition('bottom-center')).toBe('bottom-center');
    expect(normalizeToastPosition('top-start')).toBe('top-start');
    expect(normalizeToastVariant('dynamic-island')).toBe('dynamic-island');
    expect(normalizeToastVariant('terminal-hud')).toBe('terminal-hud');
    expect(normalizeToastVariant('unknown-style')).toBe('beui-stack');

    useUiCustomizationStore.getState().setActiveGatewaySection('caveman');
    expect(useUiCustomizationStore.getState().activeGatewaySection).toBe('caveman');
    useUiCustomizationStore.getState().setActiveGatewaySection('agent');

    expect(TOAST_DURATION_OPTIONS.length).toBe(4);
    expect(TOAST_MAX_VISIBLE_OPTIONS.length).toBe(3);
    expect(INLINE_NOTICE_STYLE_OPTIONS.length).toBe(3);
    expect(TOAST_CATEGORY_TOGGLE_OPTIONS.length).toBe(6);

    expect(resolveToastDurationMs('fast')).toBe(3200);
    expect(resolveToastDurationMs('standard')).toBe(5000);
    expect(resolveToastDurationMs('extended')).toBe(8000);
    expect(resolveToastDurationMs('sticky')).toBe(0);
    expect(resolveToastDurationMs('fast', 9000)).toBe(9000);

    expect(computeCumulativeExpandedOffsets([80, 92, 74], 8)).toEqual([0, 88, 188]);

    const frontBottom = computeToastStackGeometry({
      index: 0,
      isExpanded: false,
      expandedOffsetPx: 0,
      position: 'bottom-right',
      maxVisible: 4,
    });
    const depthBottom = computeToastStackGeometry({
      index: 2,
      isExpanded: false,
      expandedOffsetPx: 180,
      position: 'bottom-right',
      maxVisible: 4,
    });
    const depthTop = computeToastStackGeometry({
      index: 2,
      isExpanded: false,
      expandedOffsetPx: 180,
      position: 'top-right',
      maxVisible: 4,
    });
    const expandedBottom = computeToastStackGeometry({
      index: 2,
      isExpanded: true,
      expandedOffsetPx: 180,
      position: 'bottom-right',
      maxVisible: 4,
    });
    const overflowHidden = computeToastStackGeometry({
      index: 4,
      isExpanded: false,
      expandedOffsetPx: 320,
      position: 'bottom-right',
      maxVisible: 4,
    });

    expect(frontBottom.scale).toBe(1);
    expect(frontBottom.translateY).toBe(-0);
    expect(frontBottom.hidden).toBe(false);
    expect(depthBottom.scale).toBeLessThan(1);
    expect(depthBottom.translateY).toBeLessThan(0);
    expect(depthTop.translateY).toBeGreaterThan(0);
    expect(expandedBottom.scale).toBe(1);
    expect(expandedBottom.translateY).toBe(-180);
    expect(overflowHidden.hidden).toBe(true);
    expect(overflowHidden.opacity).toBe(0);

    expect(mapNoticeKindToToastCategory('workflow')).toBe('workflow');
    expect(mapNoticeKindToToastCategory('skill')).toBe('skill');
    expect(mapNoticeKindToToastCategory('self_improvement')).toBe('self_improvement');

    // Burst deduplication + unread badge + Notification Center read state
    useToastStore.getState().pushToast({
      dedupeKey: 'workflow:matched:deploy_release',
      category: 'workflow',
      title: 'Workflow matched',
      detail: 'deploy_release',
    });
    useToastStore.getState().pushToast({
      dedupeKey: 'workflow:matched:deploy_release',
      category: 'workflow',
      title: 'Workflow matched',
      detail: 'deploy_release',
    });

    expect(useToastStore.getState().toasts.length).toBe(1);
    expect(useToastStore.getState().toasts[0]?.count).toBe(2);
    expect(useToastStore.getState().unreadCount).toBe(2);

    const activeId = useToastStore.getState().toasts[0]!.id;
    useToastStore.getState().dismissToast(activeId);
    expect(useToastStore.getState().toasts.length).toBe(0);
    expect(useToastStore.getState().history.length).toBe(1);

    useToastStore.getState().setIsCenterOpen(true);
    expect(useToastStore.getState().unreadCount).toBe(0);
    useToastStore.getState().setIsCenterOpen(false);

    // Category mute toggles suppress active popup stack while preserving Bell history
    useUiCustomizationStore.getState().applyCustomization({
      toastVariant: 'minimal-glass',
      toastPosition: 'top-center',
      toastDuration: 'extended',
      toastMaxVisible: 5,
      inlineNoticeStyle: 'cards',
      toastNotifySkill: false,
    });
    expect(useUiCustomizationStore.getState().toastVariant).toBe('minimal-glass');
    expect(useUiCustomizationStore.getState().toastMaxVisible).toBe(5);
    expect(shouldEmitCategoryToast('skill', useUiCustomizationStore.getState())).toBe(false);
    expect(shouldEmitCategoryToast('workflow', useUiCustomizationStore.getState())).toBe(true);

    const jumps: string[] = [];
    const nav = {
      openSkills: () => jumps.push('skills'),
      openKnowledge: () => jumps.push('knowledge'),
      openMemory: () => jumps.push('memory'),
      openActivity: () => jumps.push('activity'),
      openAgents: () => jumps.push('agents'),
      openLogs: () => jumps.push('logs'),
      focusChat: () => jumps.push('chat'),
    };

    dispatchActivityNoticeToast(
      {
        id: 'n1',
        kind: 'skill',
        title: 'Skill used',
        detail: 'rust-pro',
        timestamp: 5000,
      },
      'skill',
      nav,
    );
    // Muted category: 0 active popups, but recorded in Notification Center history
    expect(useToastStore.getState().toasts.length).toBe(0);
    expect(useToastStore.getState().history[0]?.category).toBe('skill');

    const skillCuratorJump = resolveNoticeJumpAction(
      'self_improvement',
      'Skill updated',
      nav,
    );
    expect(skillCuratorJump.actionLabel).toBe('Open Skills');
    skillCuratorJump.onAction();

    const memoryCuratorJump = resolveNoticeJumpAction(
      'self_improvement',
      'Self-improvement memory updated',
      nav,
    );
    expect(memoryCuratorJump.actionLabel).toBe('Open Memory');
    memoryCuratorJump.onAction();

    expect(jumps).toEqual(['skills', 'memory']);

    useToastStore.getState().resetToastStore();
    useUiCustomizationStore.getState().resetAllCustomization();
  });

  test('maps steps into official @beui/agent-activity registry items, strips ANSI codes, and drops Progress spam without regex', () => {
    expect(interpolateTemplate('Ran {count} tools', { count: 3 })).toBe('Ran 3 tools');
    expect(
      interpolateTemplate('{toolCalls}, {messages}', {
        toolCalls: '2 tool calls',
        messages: '1 message',
      }),
    ).toBe('2 tool calls, 1 message');
    expect(interpolateTemplate('Keep {missing} and {bad-key}', { count: 1 })).toBe(
      'Keep {missing} and {bad-key}',
    );

    expect(stripTerminalFormatting('*[1mopenz_inventory[0m(runtime identity)*')).toBe(
      '*openz_inventory(runtime identity)*',
    );
    expect(stripTerminalFormatting('\x1b[1mgit_manager\x1b[0m')).toBe('git_manager');

    expect(classifyBeuiToolAction('read_file')).toBe('read');
    expect(classifyBeuiToolAction('openz_inventory')).toBe('read');
    expect(classifyBeuiToolAction('patch_file')).toBe('edit');
    expect(classifyBeuiToolAction('write_file')).toBe('write');
    expect(classifyBeuiToolAction('delegate_task')).toBe('delegate');
    expect(classifyBeuiToolAction('git_manager')).toBe('run');
    expect(formatBeuiToolTarget('git_manager', { action: 'log' })).toBe('git log');
    expect(formatBeuiToolTarget('read_file', { path: 'src/main.rs' })).toBe('src/main.rs');

    const steps = buildMessageActivitySteps(
      [
        {
          id: 'msg-official',
          role: 'assistant',
          content: 'Summary ready.',
          timestamp: 1000,
          isStreaming: false,
          reasoningContent: 'Checking runtime inventory and recent commits.',
          activityNotices: [
            {
              id: 'progress-spam-1',
              kind: 'system',
              title: 'Progress',
              detail: '▶ Running *[1mopenz_inventory[0m(runtime identity)*...',
              timestamp: 1010,
            },
            {
              id: 'skill-1',
              kind: 'skill',
              title: 'Skill used',
              detail: 'rust-pro',
              timestamp: 1020,
            },
          ],
          toolCalls: [
            {
              id: 'tc-inv',
              name: 'openz_inventory',
              args: { section: 'runtime' },
              status: 'success',
              durationMs: 42,
              startedAt: 1030,
            },
            {
              id: 'tc-search',
              name: 'web_search',
              args: { query: 'beui agent-activity' },
              output: JSON.stringify({
                results: [
                  { title: 'Agent Activity - BeUI', url: 'https://beui.dev/components/agents/agent-activity' },
                ],
              }),
              status: 'success',
              durationMs: 180,
              startedAt: 1040,
            },
          ],
        },
      ],
      { chronological: true },
    );

    // Raw Progress notice must be completely excluded
    expect(steps.some((s) => s.title === 'Progress')).toBe(false);
    expect(steps.length).toBe(4);
    expect(steps[0]?.status).toBe('info');

    const adaptiveItems = toBeuiOfficialActivityItems(steps, 'orb-card', true);
    expect(adaptiveItems.map((i) => i.type)).toEqual(['text', 'step', 'tool', 'search']);

    const checklistItems = toBeuiOfficialActivityItems(steps, 'rail-timeline', true);
    expect(checklistItems.every((i) => i.type === 'step')).toBe(true);

    const compactItems = toBeuiOfficialActivityItems(steps, 'compact-pills', false);
    expect(compactItems.length).toBe(4);

    const traceItems = toBeuiOfficialActivityItems(steps, 'trace-log', true);
    expect(traceItems.every((i) => i.type === 'trace')).toBe(true);
  });
});
