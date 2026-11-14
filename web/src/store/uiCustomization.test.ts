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
  ACTIVITY_DENSITY_OPTIONS,
  ACTIVITY_MAX_HEIGHT_OPTIONS,
  ACTIVITY_ORB_OPTIONS,
  ACTIVITY_PLACEMENT_OPTIONS,
  ACTIVITY_VARIANT_OPTIONS,
  AGENT_AVATAR_OPTIONS,
  USER_AVATAR_OPTIONS,
  extractAllMarkdownSources,
  extractMarkdownCitations,
  extractToolImagePayload,
  formatApprovalArgs,
  orchestrationRunToSubagentProps,
  parseCitationSegments,
  parseUnifiedDiff,
  resolveActivityPhase,
  resolveMaxHeightPx,
  resolveStepContentType,
  summarizeApprovalInline,
  tokenizeDiffLine,
} from '../components/beui';
import {
  buildInventoryActivitySteps,
  buildLogActivitySteps,
  buildMessageActivitySteps,
  buildOrchestrationActivitySteps,
  buildUnifiedDashboardActivitySteps,
  classifyToolContentType,
} from '../features/activity/activityAdapters';
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
    expect(state.toolResultStyle).toBe('beui-card');
    expect(state.approvalStyle).toBe('beui-tool-approval');
    expect(state.imageGenStyle).toBe('beui-frame');
    expect(state.citationsStyle).toBe('pills');
    expect(state.dataTableStyle).toBe('interactive');
    expect(state.jsonViewerStyle).toBe('tree');
    expect(state.alertStyle).toBe('beui-alert');
    expect(state.collapsibleStyle).toBe('subtle');
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
      toolResultStyle: 'compact-pill',
      approvalStyle: 'beui-approval-card',
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
    expect(updated.toolResultStyle).toBe('compact-pill');
    expect(updated.approvalStyle).toBe('beui-approval-card');
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
      approvalStyle: 'beui-approval-card',
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
    expect(useUiCustomizationStore.getState().approvalStyle).toBe('beui-approval-card');

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

  test('summarizes resolved tool permissions into a minimal single-line status summary', () => {
    const summaryFromArgs = summarizeApprovalInline({
      requestId: 'req-1',
      toolName: 'curate_skill',
      description: '',
      arguments: { action: 'list' },
      status: 'approved',
    });
    expect(summaryFromArgs).toBe('action: list');

    const summaryFromDesc = summarizeApprovalInline({
      requestId: 'req-2',
      toolName: 'exec_command',
      description: 'cargo check -p openz',
      arguments: { command: 'cargo check -p openz' },
      status: 'denied',
    });
    expect(summaryFromDesc).toBe('cargo check -p openz');
    expect(formatApprovalArgs({ action: 'list' })).toContain('"action": "list"');
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
  });
});
