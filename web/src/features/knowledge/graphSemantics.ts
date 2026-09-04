import type { CognitiveEdge, CognitiveNode } from '../../types/openz';
import { stableHash } from './graphLayout';

export type EdgeProvenance = 'extracted' | 'inferred' | 'not_recorded';

export interface NodeMetrics {
  degree: number;
  importance: number;
  clusterId: string;
}

export interface EdgeSemantics {
  edge: CognitiveEdge;
  confidence: number | null;
  provenance: EdgeProvenance;
  source: string | null;
}

export interface GraphSemantics {
  nodeMetrics: Map<string, NodeMetrics>;
  edgeMetrics: Map<string, EdgeSemantics>;
  clusterIds: string[];
  malformedRecords: number;
}

function hasText(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function clampConfidence(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return Math.min(1, Math.max(0, value));
}

function normalizeProvenance(value: unknown): EdgeProvenance {
  if (typeof value !== 'string') return 'not_recorded';
  const normalized = value.trim().toLowerCase();
  if (normalized === 'extracted') return 'extracted';
  if (normalized === 'inferred') return 'inferred';
  return 'not_recorded';
}

export function edgeKey(edge: Pick<CognitiveEdge, 'from_name' | 'to_name' | 'relation_type'>): string {
  return [edge.from_name, edge.to_name, edge.relation_type].join('\u0000');
}

export function formatConfidence(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return 'Not recorded';
  return `${Math.round(Math.min(1, Math.max(0, value)) * 100)}%`;
}

export function formatProvenance(value: EdgeProvenance): string {
  if (value === 'not_recorded') return 'Not recorded';
  return value[0].toUpperCase() + value.slice(1);
}

export function buildGraphSemantics(nodes: CognitiveNode[], edges: CognitiveEdge[]): GraphSemantics {
  let malformedRecords = 0;
  const nodeByName = new Map<string, CognitiveNode>();

  [...nodes]
    .filter((node) => {
      if (!node || !hasText(node.name)) {
        malformedRecords += 1;
        return false;
      }
      return true;
    })
    .sort((left, right) => left.name.localeCompare(right.name))
    .forEach((node) => {
      if (!nodeByName.has(node.name)) nodeByName.set(node.name, node);
    });

  const parent = new Map<string, string>();
  const degree = new Map<string, number>();
  nodeByName.forEach((_, name) => {
    parent.set(name, name);
    degree.set(name, 0);
  });

  const find = (value: string): string => {
    let root = value;
    while (parent.get(root) !== root) root = parent.get(root) || root;
    let current = value;
    while (parent.get(current) !== current) {
      const next = parent.get(current) || current;
      parent.set(current, root);
      current = next;
    }
    return root;
  };

  const union = (left: string, right: string) => {
    const leftRoot = find(left);
    const rightRoot = find(right);
    if (leftRoot !== rightRoot) parent.set(rightRoot, leftRoot);
  };

  const validEdges: CognitiveEdge[] = [];
  const seenEdges = new Set<string>();
  edges.forEach((edge) => {
    if (!edge || !hasText(edge.from_name) || !hasText(edge.to_name) || !hasText(edge.relation_type)) {
      malformedRecords += 1;
      return;
    }
    if (!nodeByName.has(edge.from_name) || !nodeByName.has(edge.to_name)) {
      malformedRecords += 1;
      return;
    }
    const key = edgeKey(edge);
    if (seenEdges.has(key)) return;
    seenEdges.add(key);
    validEdges.push(edge);
    degree.set(edge.from_name, (degree.get(edge.from_name) || 0) + 1);
    degree.set(edge.to_name, (degree.get(edge.to_name) || 0) + 1);
    union(edge.from_name, edge.to_name);
  });

  const componentMap = new Map<string, string[]>();
  nodeByName.forEach((_, name) => {
    const root = find(name);
    const names = componentMap.get(root) || [];
    names.push(name);
    componentMap.set(root, names);
  });

  const isolatedByType = new Map<string, string[]>();
  const components: string[][] = [];
  componentMap.forEach((names) => {
    const onlyName = names[0];
    if (names.length === 1 && (degree.get(onlyName) || 0) === 0) {
      const type = (nodeByName.get(onlyName)?.entity_type || 'unknown').trim().toLowerCase() || 'unknown';
      const bucket = isolatedByType.get(type) || [];
      bucket.push(onlyName);
      isolatedByType.set(type, bucket);
    } else {
      components.push(names);
    }
  });
  isolatedByType.forEach((names) => components.push(names));

  const clusterByNode = new Map<string, string>();
  const clusterIds = components
    .map((names) => names.sort((left, right) => left.localeCompare(right)))
    .map((names) => {
      const clusterId = `cluster:${stableHash(names.join('|')).toString(16)}`;
      names.forEach((name) => clusterByNode.set(name, clusterId));
      return clusterId;
    })
    .sort((left, right) => left.localeCompare(right));

  const maxDegree = Math.max(...degree.values(), 0);
  const nodeMetrics = new Map<string, NodeMetrics>();
  degree.forEach((value, name) => {
    nodeMetrics.set(name, {
      degree: value,
      importance: maxDegree === 0 ? 0 : Math.min(1, value / maxDegree),
      clusterId: clusterByNode.get(name) || `cluster:${stableHash(name).toString(16)}`,
    });
  });

  const edgeMetrics = new Map<string, EdgeSemantics>();
  validEdges.forEach((edge) => {
    edgeMetrics.set(edgeKey(edge), {
      edge,
      confidence: clampConfidence(edge.confidence),
      provenance: normalizeProvenance(edge.provenance),
      source: hasText(edge.source) ? edge.source.trim() : null,
    });
  });

  return { nodeMetrics, edgeMetrics, clusterIds, malformedRecords };
}
