import { describe, expect, test } from 'bun:test';
import {
  buildCoreUniverseGraph,
  extractMarkdownWikilinks,
  isNoisyOrSeededFact,
  isNoisyOrSeededNode,
  normalizeGraphCategory,
} from './coreGraphBuilder';
import type { RuntimeInventory } from '../../types/openz';

const sampleInventory: RuntimeInventory = {
  version: '0.0.345',
  paths: {
    configDir: '/home/user/.openz',
    workspace: '/workspace',
    memoryDb: '/home/user/.openz/memory.db',
    graphDb: '/home/user/.openz/graph_memory.db',
    subagentsFile: '/home/user/.openz/subagents.json',
    skillsDir: '/home/user/.openz/skills',
    workspaceSkillsDir: '/workspace/.openz/skills',
    sessionsDir: '/home/user/.openz/sessions',
  },
  defaults: {
    model: 'MiniMax-M3.1-Flash',
    provider: 'minimax',
    streaming: true,
    cavemanMode: true,
    maxMessages: 120,
    maxToolIterations: 200,
    toolTimeoutSecs: 300,
  },
  counts: {
    subagents: 2,
    skills: 2,
    coreSubagents: 2,
    customSubagents: 0,
    channels: 2,
    enabledChannels: 1,
    tools: 2,
    cronJobs: 1,
    activeCronJobs: 1,
    runningCronJobs: 0,
    sessions: 1,
    activeUiSessions: 1,
  },
  channels: [
    { name: 'websocket', enabled: true, configured: true },
    { name: 'telegram', enabled: false, configured: false },
  ],
  sessions: {
    sessionsDir: '/home/user/.openz/sessions',
    webuiControlCenter: { connectedClients: 1, attachedChats: ['ws:main'] },
    activeUiSessions: [],
    recentSessions: [],
    channelCounts: [],
  },
  subagents: [
    {
      name: 'researcher',
      description: 'Deep web and repo research specialist',
      model: 'default',
      provider: 'auto',
      fallbacks: [],
      fallbackCount: 0,
      effectiveModel: 'MiniMax-M3.1-Flash',
      effectiveProvider: 'minimax',
      capabilities: ['search'],
      supportsVision: false,
      failureCount: 0,
      isCore: true,
      isProtected: true,
      source: 'core',
    },
  ],
  skills: [
    {
      name: 'cargo_release_workflow',
      description: 'Release verification workflow',
      content:
        'See [[openz_skill_bundle_layout]] and invoke cargo_check before release.',
      scope: 'global',
    },
    {
      name: 'openz_skill_bundle_layout',
      description: 'Canonical SKILL.md layout',
      content: 'Validates skill bundles.',
      scope: 'global',
    },
  ],
  memory: {
    memoryDb: { path: '/home/user/.openz/memory.db', exists: true },
    graphDb: { path: '/home/user/.openz/graph_memory.db', exists: true },
  },
  tools: [
    {
      name: 'cargo_check',
      domain: 'code',
      risk: 'low',
      usesNetwork: false,
      writesDisk: false,
      spawnsProcess: true,
      requiresApproval: false,
      priority: 1,
      description: 'Run cargo check',
    },
    {
      name: 'web_search',
      domain: 'search',
      risk: 'low',
      usesNetwork: true,
      writesDisk: false,
      spawnsProcess: false,
      requiresApproval: false,
      priority: 1,
      description: 'Search the web',
    },
  ],
  cron: {
    jobsFile: '/home/user/.openz/cron_jobs.json',
    runsFile: '/home/user/.openz/cron_runs.jsonl',
    recentRuns: 1,
    jobs: [
      {
        id: 'heartbeat_audit_5m',
        schedule: '*/5 * * * *',
        prompt: 'Audit workspace health',
        enabled: true,
        runOnce: false,
        status: 'idle',
        quiet: true,
        notifyOn: 'failure',
        runCount: 12,
        failureCount: 0,
      },
    ],
  },
};

describe('coreGraphBuilder (Core Universe Graph & Zero-Noise Filtering)', () => {
  test('filters out scraped URL noise and historical unit-test probe artifacts', () => {
    expect(
      isNoisyOrSeededNode({
        name: 'https://www.samsung.com/in/smartphones/galaxy-s23/buy/',
        entity_type: 'Entity',
        observations: '["source: https://www.samsung.com/in/smartphones/galaxy-s23/buy/"]',
      }),
    ).toBe(true);
    expect(
      isNoisyOrSeededNode({
        name: 'Python',
        entity_type: 'Entity',
        observations: '["source: https://example.com/blog"]',
      }),
    ).toBe(true);
    expect(
      isNoisyOrSeededNode({
        name: 'ProbeOne',
        entity_type: 'Probe',
        observations: '[]',
      }),
    ).toBe(true);
    expect(
      isNoisyOrSeededNode({
        name: 'Aswin',
        entity_type: 'person',
        observations: '["Creator of OpenZ"]',
      }),
    ).toBe(false);
    expect(
      isNoisyOrSeededFact({
        text: 'Project Alpha: port 9099, secret token XYZ-456',
      }),
    ).toBe(true);
  });

  test('normalizes mixed-case entity types into canonical semantic categories', () => {
    expect(normalizeGraphCategory('Concept')).toBe('concept');
    expect(normalizeGraphCategory('concept')).toBe('concept');
    expect(normalizeGraphCategory('framework')).toBe('core');
    expect(normalizeGraphCategory('person')).toBe('person');
    expect(normalizeGraphCategory('skill')).toBe('skill');
    expect(normalizeGraphCategory('subagent')).toBe('subagent');
    expect(normalizeGraphCategory('domain')).toBe('domain');
  });

  test('extracts [[wikilinks]] without regex and synthesizes connected Core Universe graph', () => {
    expect(
      extractMarkdownWikilinks('Check [[openz_skill_bundle_layout|Bundle]] and [[Rust]].'),
    ).toEqual(['openz_skill_bundle_layout', 'Rust']);

    const universe = buildCoreUniverseGraph(
      [
        { name: 'Aswin', entity_type: 'person', observations: '["Creator of OpenZ"]' },
        { name: 'OpenZ', entity_type: 'framework', observations: '["Async Rust agent"]' },
        {
          name: 'https://noisy.example.com/article',
          entity_type: 'Entity',
          observations: '["source: https://noisy.example.com/article"]',
        },
      ],
      [{ from_name: 'Aswin', to_name: 'OpenZ', relation_type: 'created' }],
      [
        {
          text: 'OpenZ uses an event-driven actor pipeline',
          timestamp: '2026-11-18T12:00:00Z',
          tags: '["architecture","openz"]',
          importance: 0.9,
        },
        {
          text: 'Project Alpha: port 9099, secret token XYZ-456',
          timestamp: '2026-07-01T00:00:00Z',
          tags: '[]',
          importance: 0.5,
        },
      ],
      sampleInventory,
      { includeCoreArchitecture: true, includeLeafTools: false },
    );

    const names = new Set(universe.nodes.map((n) => n.name));
    expect(names.has('https://noisy.example.com/article')).toBe(false);
    expect(names.has('Aswin')).toBe(true);
    expect(names.has('OpenZ')).toBe(true);
    expect(names.has('researcher')).toBe(true);
    expect(names.has('cargo_release_workflow')).toBe(true);
    expect(names.has('domain:code')).toBe(true);
    expect(names.has('domain:search')).toBe(true);
    expect(names.has('cron:heartbeat_audit_5m')).toBe(true);
    expect(names.has('channel:websocket')).toBe(true);
    expect(universe.cleanFacts.length).toBe(1);

    // Verify SKILL.md wikilinks & tool-domain bridges
    expect(
      universe.edges.some(
        (e) =>
          e.from_name === 'cargo_release_workflow' &&
          e.to_name === 'openz_skill_bundle_layout' &&
          e.relation_type === 'references',
      ),
    ).toBe(true);
    expect(
      universe.edges.some(
        (e) =>
          e.from_name === 'cargo_release_workflow' &&
          e.to_name === 'domain:code' &&
          e.relation_type === 'orchestrates_domain',
      ),
    ).toBe(true);
  });
});
