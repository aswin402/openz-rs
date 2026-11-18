import type {
  CognitiveEdge,
  CognitiveFact,
  CognitiveNode,
  RuntimeInventory,
} from '../../types/openz';

export type CanonicalGraphCategory =
  | 'core'
  | 'person'
  | 'skill'
  | 'subagent'
  | 'domain'
  | 'tool'
  | 'fact'
  | 'concept'
  | 'code'
  | 'automation';

export interface CategoryDescriptor {
  id: CanonicalGraphCategory;
  label: string;
  shortLabel: string;
  darkHex: string;
  lightHex: string;
  order: number;
}

export const GRAPH_CATEGORY_CATALOG: Record<CanonicalGraphCategory, CategoryDescriptor> = {
  core: {
    id: 'core',
    label: 'Core & Architecture',
    shortLabel: 'Core',
    darkHex: '#ff9f43',
    lightHex: '#d97706',
    order: 1,
  },
  person: {
    id: 'person',
    label: 'People & Persona',
    shortLabel: 'People',
    darkHex: '#fb7185',
    lightHex: '#e11d48',
    order: 2,
  },
  skill: {
    id: 'skill',
    label: 'Procedural Skills',
    shortLabel: 'Skills',
    darkHex: '#22d3ee',
    lightHex: '#0891b2',
    order: 3,
  },
  subagent: {
    id: 'subagent',
    label: 'Subagents',
    shortLabel: 'Subagents',
    darkHex: '#a78bfa',
    lightHex: '#7c3aed',
    order: 4,
  },
  domain: {
    id: 'domain',
    label: 'Capability Domains',
    shortLabel: 'Domains',
    darkHex: '#34d399',
    lightHex: '#059669',
    order: 5,
  },
  fact: {
    id: 'fact',
    label: 'Curated Facts',
    shortLabel: 'Facts',
    darkHex: '#facc15',
    lightHex: '#ca8a04',
    order: 6,
  },
  concept: {
    id: 'concept',
    label: 'Concepts & Topics',
    shortLabel: 'Concepts',
    darkHex: '#818cf8',
    lightHex: '#4f46e5',
    order: 7,
  },
  code: {
    id: 'code',
    label: 'Code & Files',
    shortLabel: 'Code',
    darkHex: '#4ade80',
    lightHex: '#16a34a',
    order: 8,
  },
  automation: {
    id: 'automation',
    label: 'Cron & Channels',
    shortLabel: 'Automation',
    darkHex: '#f472b6',
    lightHex: '#db2777',
    order: 9,
  },
  tool: {
    id: 'tool',
    label: 'Native Tools',
    shortLabel: 'Tools',
    darkHex: '#38bdf8',
    lightHex: '#0284c7',
    order: 10,
  },
};

export function normalizeGraphCategory(rawType: string | undefined): CanonicalGraphCategory {
  const lower = (rawType ?? '').trim().toLowerCase();
  if (
    lower === 'core' ||
    lower === 'framework' ||
    lower === 'project' ||
    lower === 'architecture' ||
    lower === 'system' ||
    lower === 'runtime'
  ) {
    return 'core';
  }
  if (
    lower === 'person' ||
    lower === 'user' ||
    lower === 'creator' ||
    lower === 'author' ||
    lower === 'persona' ||
    lower === 'identity'
  ) {
    return 'person';
  }
  if (lower === 'skill' || lower === 'skills' || lower === 'playbook' || lower === 'procedure') {
    return 'skill';
  }
  if (
    lower === 'subagent' ||
    lower === 'agent' ||
    lower === 'specialist' ||
    lower === 'profile'
  ) {
    return 'subagent';
  }
  if (lower === 'domain' || lower === 'capability' || lower === 'subsystem') {
    return 'domain';
  }
  if (lower === 'tool' || lower === 'mcp') {
    return 'tool';
  }
  if (
    lower === 'fact' ||
    lower === 'memory' ||
    lower === 'preference' ||
    lower === 'observation' ||
    lower === 'rule'
  ) {
    return 'fact';
  }
  if (
    lower === 'code' ||
    lower === 'function' ||
    lower === 'struct' ||
    lower === 'enum' ||
    lower === 'trait' ||
    lower === 'impl' ||
    lower === 'module' ||
    lower === 'file' ||
    lower === 'script'
  ) {
    return 'code';
  }
  if (
    lower === 'automation' ||
    lower === 'cron' ||
    lower === 'schedule' ||
    lower === 'channel' ||
    lower === 'workflow'
  ) {
    return 'automation';
  }
  return 'concept';
}

export function getCategoryDescriptor(rawType: string | undefined): CategoryDescriptor {
  return GRAPH_CATEGORY_CATALOG[normalizeGraphCategory(rawType)];
}

/**
 * Rejects historical unit-test probes and raw URL-scraper artifacts so only genuine
 * core OpenZ entities ever appear in the graph.
 */
export function isNoisyOrSeededNode(node: Pick<CognitiveNode, 'name' | 'entity_type' | 'observations'>): boolean {
  const name = node.name.trim();
  if (!name) return true;
  if (name.startsWith('http://') || name.startsWith('https://')) return true;
  if (
    node.entity_type === 'Entity' &&
    node.observations.trim().startsWith('["source: http')
  ) {
    return true;
  }
  if (
    name === 'ProbeOne' ||
    name === 'ProbeTwo' ||
    name === 'ProbeThree' ||
    name === 'OtherNode'
  ) {
    return true;
  }
  if (node.observations.includes('/tmp/openz_code_test')) return true;
  return false;
}

export function isNoisyOrSeededFact(fact: Pick<CognitiveFact, 'text'>): boolean {
  const text = fact.text.trim();
  if (!text) return true;
  if (text.startsWith('Project Alpha: port 9099')) return true;
  return false;
}

/**
 * Zero-regex `[[wikilink]]` extractor for `SKILL.md` and notes.
 */
export function extractMarkdownWikilinks(text: string): string[] {
  const results: string[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const start = text.indexOf('[[', cursor);
    if (start === -1) break;
    const end = text.indexOf(']]', start + 2);
    if (end === -1) break;
    const raw = text.slice(start + 2, end).trim();
    const pipeIdx = raw.indexOf('|');
    const target = (pipeIdx === -1 ? raw : raw.slice(0, pipeIdx)).trim();
    if (target && !target.includes('\n') && target.length <= 80) {
      results.push(target);
    }
    cursor = end + 2;
  }
  return Array.from(new Set(results));
}

function parseTagList(rawTags: string): string[] {
  const trimmed = rawTags.trim();
  if (!trimmed || trimmed === '[]') return [];
  if (trimmed.startsWith('[')) {
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (Array.isArray(parsed)) {
        return parsed
          .filter((item): item is string => typeof item === 'string')
          .map((t) => t.trim().toLowerCase())
          .filter(Boolean);
      }
    } catch {
      // Fall through to comma split
    }
  }
  return trimmed
    .split(',')
    .map((t) => t.trim().toLowerCase())
    .filter(Boolean);
}

function summarizeFactTitle(text: string, index: number): string {
  const firstLine = text.split('\n')[0]?.trim() ?? '';
  const clean = firstLine.startsWith('- ') ? firstLine.slice(2).trim() : firstLine;
  if (!clean) return `Memory Fact #${index + 1}`;
  if (clean.length <= 46) return clean;
  return `${clean.slice(0, 43).trimEnd()}…`;
}

function formatDomainNodeName(domain: string): string {
  const clean = domain.trim().toLowerCase() || 'general';
  return `domain:${clean}`;
}

function containsTokenBoundary(haystackLower: string, tokenLower: string): boolean {
  if (!tokenLower || tokenLower.length < 3) return false;
  let pos = 0;
  while (pos < haystackLower.length) {
    const idx = haystackLower.indexOf(tokenLower, pos);
    if (idx === -1) return false;
    const before = idx === 0 ? ' ' : haystackLower[idx - 1];
    const afterIdx = idx + tokenLower.length;
    const after = afterIdx >= haystackLower.length ? ' ' : haystackLower[afterIdx];
    const isWordChar = (ch: string) => {
      const code = ch.charCodeAt(0);
      return (
        (code >= 48 && code <= 57) ||
        (code >= 65 && code <= 90) ||
        (code >= 97 && code <= 122)
      );
    };
    if (!isWordChar(before) && !isWordChar(after)) {
      return true;
    }
    pos = idx + tokenLower.length;
  }
  return false;
}

export interface CoreUniverseBuildOptions {
  /** Include runtime skills, subagents, capability domains, curated facts, and cron/channels. */
  includeCoreArchitecture?: boolean;
  /** Also expand all ~229 leaf native Rust tools around their capability domain hubs. */
  includeLeafTools?: boolean;
}

export interface CoreUniverseGraph {
  nodes: CognitiveNode[];
  edges: CognitiveEdge[];
  cleanFacts: CognitiveFact[];
}

/**
 * Synthesizes OpenZ's authentic Core Knowledge Graph (Graphify + Obsidian style)
 * exclusively from live persisted memory stores and live runtime inventory —
 * zero seeded demo data, zero hardcoded mock entities.
 */
export function buildCoreUniverseGraph(
  rawNodes: ReadonlyArray<CognitiveNode>,
  rawEdges: ReadonlyArray<CognitiveEdge>,
  rawFacts: ReadonlyArray<CognitiveFact>,
  inventory: RuntimeInventory | null,
  options?: CoreUniverseBuildOptions,
): CoreUniverseGraph {
  const includeCore = options?.includeCoreArchitecture ?? true;
  const includeLeafTools = options?.includeLeafTools ?? false;

  const nodeMap = new Map<string, CognitiveNode>();
  const nameByLower = new Map<string, string>();
  const edgeMap = new Map<string, CognitiveEdge>();

  const upsertNode = (
    name: string,
    category: CanonicalGraphCategory | string,
    observations: string | string[],
    preferExisting = true,
  ): string => {
    const trimmed = name.trim();
    if (!trimmed) return '';
    const canonicalCat = normalizeGraphCategory(category);
    const existingCanonical = nameByLower.get(trimmed.toLowerCase());
    const obsString = Array.isArray(observations)
      ? JSON.stringify(observations)
      : observations;

    if (existingCanonical && nodeMap.has(existingCanonical)) {
      if (!preferExisting) {
        const prev = nodeMap.get(existingCanonical)!;
        nodeMap.set(existingCanonical, {
          name: existingCanonical,
          entity_type: canonicalCat,
          observations:
            prev.observations && prev.observations !== '[]'
              ? prev.observations
              : obsString,
        });
      }
      return existingCanonical;
    }

    nodeMap.set(trimmed, {
      name: trimmed,
      entity_type: canonicalCat,
      observations: obsString,
    });
    nameByLower.set(trimmed.toLowerCase(), trimmed);
    return trimmed;
  };

  const addEdge = (
    fromRaw: string,
    toRaw: string,
    relationType: string,
    confidence = 1.0,
    provenance = 'extracted',
    source = 'openz-core',
  ) => {
    const from = nameByLower.get(fromRaw.trim().toLowerCase()) ?? fromRaw.trim();
    const to = nameByLower.get(toRaw.trim().toLowerCase()) ?? toRaw.trim();
    if (!from || !to || from === to) return;
    if (!nodeMap.has(from) || !nodeMap.has(to)) return;
    const key = `${from}\u0000${to}\u0000${relationType}`;
    if (!edgeMap.has(key)) {
      edgeMap.set(key, {
        from_name: from,
        to_name: to,
        relation_type: relationType,
        confidence,
        provenance,
        source,
      });
    }
  };

  // 1. Clean persisted Knowledge Graph nodes & edges from `graph_memory.db`
  for (const node of rawNodes) {
    if (isNoisyOrSeededNode(node)) continue;
    const inferredType =
      node.name.endsWith('.sh') || node.name.endsWith('.rs') || node.name.endsWith('.ts')
        ? 'code'
        : node.entity_type;
    upsertNode(node.name, inferredType, node.observations, false);
  }

  for (const edge of rawEdges) {
    addEdge(
      edge.from_name,
      edge.to_name,
      edge.relation_type,
      edge.confidence ?? 1.0,
      edge.provenance ?? 'extracted',
      edge.source ?? 'graph_memory.db',
    );
  }

  const cleanFacts = rawFacts.filter((f) => !isNoisyOrSeededFact(f));

  if (!includeCore) {
    return {
      nodes: Array.from(nodeMap.values()),
      edges: Array.from(edgeMap.values()),
      cleanFacts,
    };
  }

  // 2. Ensure core OpenZ anchor exists if any core inventory or facts are present
  const hasAnyCoreSource = Boolean(inventory) || cleanFacts.length > 0 || nodeMap.size > 0;
  const openzRoot = hasAnyCoreSource
    ? upsertNode(
        'OpenZ',
        'core',
        inventory
          ? [
              `OpenZ v${inventory.version} async Rust AI agent runtime`,
              `Default model: ${inventory.defaults.provider}/${inventory.defaults.model}`,
              `${inventory.counts.tools} native tools · ${inventory.counts.skills} skills · ${inventory.counts.subagents} subagents`,
            ]
          : ['OpenZ local-first Rust AI agent runtime'],
        true,
      )
    : '';

  if (inventory && openzRoot) {
    // 3. Capability Domains aggregated dynamically from real registered native tools
    const toolsByDomain = new Map<string, RuntimeInventory['tools']>();
    for (const tool of inventory.tools) {
      const dom = tool.domain.trim().toLowerCase() || 'general';
      const bucket = toolsByDomain.get(dom) ?? [];
      bucket.push(tool);
      toolsByDomain.set(dom, bucket);
    }

    const domainNodeBySlug = new Map<string, string>();
    for (const [dom, domainTools] of toolsByDomain.entries()) {
      const domNodeName = upsertNode(
        formatDomainNodeName(dom),
        'domain',
        [
          `Capability domain '${dom}' (${domainTools.length} native Rust tools)`,
          `Representative tools: ${domainTools
            .slice(0, 6)
            .map((t) => t.name)
            .join(', ')}`,
        ],
        false,
      );
      domainNodeBySlug.set(dom, domNodeName);
      addEdge(openzRoot, domNodeName, 'provides_domain', 1.0, 'extracted', 'ToolRegistry');

      if (includeLeafTools) {
        for (const tool of domainTools) {
          const tNode = upsertNode(
            tool.name,
            'tool',
            [
              tool.description,
              `Domain: ${tool.domain} · Risk: ${tool.risk}`,
            ],
            true,
          );
          addEdge(domNodeName, tNode, 'registers_tool', 1.0, 'extracted', 'ToolRegistry');
        }
      }
    }

    // 4. Real Subagent Profiles (`~/.openz/subagents.json` + core specialists)
    for (const sub of inventory.subagents) {
      const subNode = upsertNode(
        sub.name,
        'subagent',
        [
          sub.description,
          `Model: ${sub.effectiveProvider}/${sub.effectiveModel} (${sub.source})`,
          ...(sub.capabilities.length > 0
            ? [`Capabilities: ${sub.capabilities.join(', ')}`]
            : []),
        ],
        true,
      );
      addEdge(openzRoot, subNode, 'delegates_to', 1.0, 'extracted', 'subagents.json');

      for (const cap of sub.capabilities) {
        const capSlug = cap.trim().toLowerCase();
        const matchingDom = domainNodeBySlug.get(capSlug);
        if (matchingDom) {
          addEdge(subNode, matchingDom, 'uses_domain', 0.95, 'extracted', 'subagents.json');
        }
      }
    }

    // 5. Real Procedural Skills (`~/.openz/skills/*/SKILL.md`)
    const skillsHub = nameByLower.get('skills');
    const toolToDomain = new Map<string, string>();
    for (const tool of inventory.tools) {
      const domNode = domainNodeBySlug.get(tool.domain.trim().toLowerCase() || 'general');
      if (domNode) {
        toolToDomain.set(tool.name.toLowerCase(), domNode);
      }
    }

    // Pass 1: register all skill nodes first so forward [[wikilinks]] always resolve
    const skillNodeByName = new Map<string, string>();
    for (const sk of inventory.skills) {
      const skNode = upsertNode(
        sk.name,
        'skill',
        [
          sk.description || `Procedural AgentSkill (${sk.scope || 'global'})`,
          ...(sk.path ? [`Path: ${sk.path}`] : []),
          ...(typeof sk.useCount === 'number' && sk.useCount > 0
            ? [`Used ${sk.useCount} times (${sk.revisionCount ?? 1} revisions)`]
            : []),
        ],
        false,
      );
      skillNodeByName.set(sk.name, skNode);

      if (sk.profile) {
        const profNode = nameByLower.get(sk.profile.toLowerCase());
        if (profNode) {
          addEdge(profNode, skNode, 'owns_skill', 1.0, 'extracted', 'skills');
        } else {
          addEdge(skillsHub ?? openzRoot, skNode, 'includes_skill', 1.0, 'extracted', 'skills');
        }
      } else {
        addEdge(skillsHub ?? openzRoot, skNode, 'includes_skill', 1.0, 'extracted', 'skills');
      }
    }

    // Pass 2: connect SKILL.md [[wikilinks]] & referenced tool domains deterministically
    for (const sk of inventory.skills) {
      const skNode = skillNodeByName.get(sk.name);
      if (!skNode || !sk.content) continue;

      const wikilinks = extractMarkdownWikilinks(sk.content);
      for (const target of wikilinks) {
        const resolved = nameByLower.get(target.toLowerCase());
        if (resolved && resolved !== skNode) {
          addEdge(skNode, resolved, 'references', 0.95, 'extracted', 'SKILL.md');
        }
      }

      const lowerContent = sk.content.toLowerCase();
      const linkedDomains = new Set<string>();
      for (const [toolNameLower, domNode] of toolToDomain.entries()) {
        if (linkedDomains.size >= 3) break;
        if (toolNameLower.includes('_') && lowerContent.includes(toolNameLower)) {
          linkedDomains.add(domNode);
        }
      }
      for (const domNode of linkedDomains) {
        addEdge(skNode, domNode, 'orchestrates_domain', 0.9, 'extracted', 'SKILL.md');
      }
    }

    // 6. Real Scheduled Cron Jobs & Enabled Communication Channels
    for (const job of inventory.cron.jobs) {
      const cronNode = upsertNode(
        `cron:${job.id}`,
        'automation',
        [
          `Schedule: ${job.schedule} (${job.enabled ? 'enabled' : 'paused'})`,
          `Runs: ${job.runCount} · Status: ${job.status}`,
          job.prompt,
        ],
        true,
      );
      addEdge(openzRoot, cronNode, 'schedules', 1.0, 'extracted', 'cron_jobs.json');
    }

    for (const ch of inventory.channels) {
      if (!ch.enabled && !ch.configured) continue;
      const chNode = upsertNode(
        `channel:${ch.name}`,
        'automation',
        [
          `Channel '${ch.name}' (${ch.enabled ? 'enabled' : 'configured'})`,
        ],
        true,
      );
      addEdge(openzRoot, chNode, 'listens_on', 1.0, 'extracted', 'config.json');
    }
  }

  // 7. Real Curated Cognitive Facts (`memory.db`) linked to tags & core entities
  const existingEntityNames = Array.from(nodeMap.keys());
  cleanFacts.forEach((fact, idx) => {
    const title = summarizeFactTitle(fact.text, idx);
    const tags = parseTagList(fact.tags);
    const factNode = upsertNode(
      title,
      'fact',
      [
        fact.text,
        `Importance: ${fact.importance.toFixed(2)} · Recorded: ${fact.timestamp}`,
      ],
      true,
    );

    let linkedCount = 0;
    for (const tag of tags.slice(0, 4)) {
      if (tag === 'openz' && openzRoot) {
        addEdge(factNode, openzRoot, 'documents', fact.importance, 'extracted', 'memory.db');
        linkedCount += 1;
        continue;
      }
      const existingMatch =
        nameByLower.get(tag) ?? nameByLower.get(`domain:${tag}`);
      const targetConcept =
        existingMatch ??
        upsertNode(tag, 'concept', [`Cognitive memory tag '${tag}'`], true);
      addEdge(factNode, targetConcept, 'tagged_with', fact.importance, 'extracted', 'memory.db');
      linkedCount += 1;
    }

    const lowerFact = fact.text.toLowerCase();
    for (const candidate of existingEntityNames) {
      if (linkedCount >= 5) break;
      if (candidate === factNode || candidate.length < 4) continue;
      if (containsTokenBoundary(lowerFact, candidate.toLowerCase())) {
        addEdge(factNode, candidate, 'about', fact.importance, 'extracted', 'memory.db');
        linkedCount += 1;
      }
    }

    if (linkedCount === 0 && openzRoot) {
      addEdge(openzRoot, factNode, 'remembers', fact.importance, 'extracted', 'memory.db');
    }
  });

  return {
    nodes: Array.from(nodeMap.values()),
    edges: Array.from(edgeMap.values()),
    cleanFacts,
  };
}
