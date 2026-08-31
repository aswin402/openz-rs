import type { CognitiveEdge, CognitiveNode } from '../types/openz';

export type GraphMode = 'overview' | 'all' | 'neighborhood';

export interface LayoutNode {
  id: string;
  name: string;
  type: string;
  observations: string;
  degree: number;
  x: number;
  y: number;
  radius: number;
  color: string;
}

export interface GraphCluster {
  id: string;
  type: string;
  nodeIds: string[];
  x: number;
  y: number;
  radius: number;
  representativeIds: string[];
}

export interface VisibleGraph {
  nodes: LayoutNode[];
  edges: CognitiveEdge[];
  clusters: GraphCluster[];
  loadedNodeCount: number;
  visibleNodeCount: number;
  visibleEdgeCount: number;
}

const OVERVIEW_REPRESENTATIVE_LIMIT = 6;
const DEFAULT_PADDING = 72;

/** A stable, inexpensive hash suitable for layout points (not cryptography). */
export function stableHash(value: string): number {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

/** Return a deterministic point inside the canvas for a node identity. */
export function stablePoint(
  id: string,
  width: number,
  height: number,
  padding = DEFAULT_PADDING,
): { x: number; y: number } {
  const safeWidth = Math.max(1, Number.isFinite(width) ? width : 1);
  const safeHeight = Math.max(1, Number.isFinite(height) ? height : 1);
  const safePadding = Math.max(0, Math.min(padding, Math.min(safeWidth, safeHeight) / 2));
  const usableWidth = Math.max(1, safeWidth - safePadding * 2);
  const usableHeight = Math.max(1, safeHeight - safePadding * 2);
  const xFraction = stableHash(`${id}:x`) / 0xffffffff;
  const yFraction = stableHash(`${id}:y`) / 0xffffffff;

  return {
    x: safePadding + xFraction * usableWidth,
    y: safePadding + yFraction * usableHeight,
  };
}

export function computeDegrees(nodes: CognitiveNode[], edges: CognitiveEdge[]): Map<string, number> {
  const knownNames = new Set(nodes.map((node) => node.name));
  const degrees = new Map(nodes.map((node) => [node.name, 0]));
  edges.forEach((edge) => {
    if (knownNames.has(edge.from_name)) {
      degrees.set(edge.from_name, (degrees.get(edge.from_name) || 0) + 1);
    }
    if (knownNames.has(edge.to_name)) {
      degrees.set(edge.to_name, (degrees.get(edge.to_name) || 0) + 1);
    }
  });
  return degrees;
}

class DisjointSet {
  private readonly parent: number[];

  constructor(size: number) {
    this.parent = Array.from({ length: size }, (_, index) => index);
  }

  find(value: number): number {
    let root = value;
    while (this.parent[root] !== root) root = this.parent[root];
    while (this.parent[value] !== value) {
      const next = this.parent[value];
      this.parent[value] = root;
      value = next;
    }
    return root;
  }

  union(left: number, right: number): void {
    const leftRoot = this.find(left);
    const rightRoot = this.find(right);
    if (leftRoot !== rightRoot) this.parent[rightRoot] = leftRoot;
  }
}

export function buildClusters(
  nodes: CognitiveNode[],
  edges: CognitiveEdge[],
  width: number,
  height: number,
): GraphCluster[] {
  if (nodes.length === 0) return [];

  const nodeIndex = new Map(nodes.map((node, index) => [node.name, index]));
  const degrees = computeDegrees(nodes, edges);
  const sets = new DisjointSet(nodes.length);
  edges.forEach((edge) => {
    const from = nodeIndex.get(edge.from_name);
    const to = nodeIndex.get(edge.to_name);
    if (from !== undefined && to !== undefined) sets.union(from, to);
  });

  const groups = new Map<string, number[]>();
  nodes.forEach((_, index) => {
    const root = sets.find(index).toString();
    const group = groups.get(root) || [];
    group.push(index);
    groups.set(root, group);
  });

  // Combine isolated nodes by type. A graph with hundreds of unrelated records
  // should still have a useful overview instead of one cluster per record.
  const isolatedByType = new Map<string, number[]>();
  const connectedGroups: number[][] = [];
  groups.forEach((group) => {
    if (group.length === 1 && (degrees.get(nodes[group[0]].name) || 0) === 0) {
      const type = nodes[group[0]].entity_type.trim().toLowerCase() || 'unknown';
      const bucket = isolatedByType.get(type) || [];
      bucket.push(group[0]);
      isolatedByType.set(type, bucket);
    } else {
      connectedGroups.push(group);
    }
  });
  isolatedByType.forEach((group) => connectedGroups.push(group));

  const points = nodes.map((node) => stablePoint(node.name, width, height));
  return connectedGroups
    .map((group) => {
      const sortedGroup = [...group].sort((left, right) => nodes[left].name.localeCompare(nodes[right].name));
      const nodeIds = sortedGroup.map((index) => nodes[index].name);
      const center = sortedGroup.reduce(
        (acc, index) => ({ x: acc.x + points[index].x, y: acc.y + points[index].y }),
        { x: 0, y: 0 },
      );
      center.x /= group.length;
      center.y /= group.length;
      const radius = Math.max(28, Math.min(170, 22 + Math.sqrt(group.length) * 14));
      const representativeIds = [...group]
        .sort((left, right) => {
          const degreeDelta = (degrees.get(nodes[right].name) || 0) - (degrees.get(nodes[left].name) || 0);
          return degreeDelta || nodes[left].name.localeCompare(nodes[right].name);
        })
        .slice(0, OVERVIEW_REPRESENTATIVE_LIMIT)
        .map((index) => nodes[index].name);
      const typeCounts = new Map<string, number>();
      group.forEach((index) => {
        const type = nodes[index].entity_type.trim().toLowerCase() || 'unknown';
        typeCounts.set(type, (typeCounts.get(type) || 0) + 1);
      });
      const type = [...typeCounts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))[0][0];

      return {
        id: `cluster:${stableHash(nodeIds.join('|')).toString(16)}`,
        type,
        nodeIds,
        x: center.x,
        y: center.y,
        radius,
        representativeIds,
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
}

function clusterPoint(
  cluster: GraphCluster,
  node: CognitiveNode,
  degree: number,
  width: number,
  height: number,
): { x: number; y: number } {
  const safeWidth = Math.max(1, Number.isFinite(width) ? width : 1);
  const safeHeight = Math.max(1, Number.isFinite(height) ? height : 1);
  const padding = Math.max(0, Math.min(DEFAULT_PADDING, Math.min(safeWidth, safeHeight) / 2));
  if (cluster.nodeIds.length === 1) {
    return {
      x: Math.min(safeWidth - padding, Math.max(padding, cluster.x)),
      y: Math.min(safeHeight - padding, Math.max(padding, cluster.y)),
    };
  }

  const angle = (stableHash(`${node.name}:cluster-angle`) / 0xffffffff) * Math.PI * 2;
  const radialHash = stableHash(`${node.name}:cluster-radius`) / 0xffffffff;
  const radialFraction = degree <= 1
    ? 0.72 + radialHash * 0.2
    : Math.min(0.48, (0.5 + radialHash * 0.12) / degree);
  const distance = Math.max(6, cluster.radius - 8) * radialFraction;

  return {
    x: Math.min(safeWidth - padding, Math.max(padding, cluster.x + Math.cos(angle) * distance)),
    y: Math.min(safeHeight - padding, Math.max(padding, cluster.y + Math.sin(angle) * distance)),
  };
}

export function layoutNodes(
  nodes: CognitiveNode[],
  edges: CognitiveEdge[],
  width: number,
  height: number,
): LayoutNode[] {
  const degrees = computeDegrees(nodes, edges);
  const clusters = buildClusters(nodes, edges, width, height);
  const clusterByNode = new Map<string, GraphCluster>();
  clusters.forEach((cluster) => cluster.nodeIds.forEach((id) => clusterByNode.set(id, cluster)));

  return nodes.map((node) => {
    const cluster = clusterByNode.get(node.name);
    const degree = degrees.get(node.name) || 0;
    const radius = Math.min(13, Math.max(3.5, 3.5 + Math.sqrt(degree) * 1.8));
    const point = cluster
      ? clusterPoint(cluster, node, degree, width, height)
      : stablePoint(node.name, width, height);
    return {
      id: node.name,
      name: node.name,
      type: node.entity_type,
      observations: node.observations,
      degree,
      x: point.x,
      y: point.y,
      radius,
      color: '#94a3b8',
    };
  });
}

export function selectVisibleGraph(
  nodes: LayoutNode[],
  edges: CognitiveEdge[],
  clusters: GraphCluster[],
  mode: GraphMode,
  _zoom: number,
  viewport: { left: number; top: number; right: number; bottom: number },
  selectedId?: string | null,
  searchQuery = '',
): VisibleGraph {
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const visibleIds = new Set<string>();
  const normalizedSearch = searchQuery.trim().toLowerCase();
  const selectedNeighbors = new Set<string>();

  if (selectedId && nodeById.has(selectedId)) selectedNeighbors.add(selectedId);
  edges.forEach((edge) => {
    if (!selectedId) return;
    if (edge.from_name === selectedId) selectedNeighbors.add(edge.to_name);
    if (edge.to_name === selectedId) selectedNeighbors.add(edge.from_name);
  });

  const isInViewport = (node: LayoutNode) =>
    node.x + node.radius >= viewport.left &&
    node.x - node.radius <= viewport.right &&
    node.y + node.radius >= viewport.top &&
    node.y - node.radius <= viewport.bottom;

  if (mode === 'overview') {
    nodes.forEach((node) => {
      if (isInViewport(node)) visibleIds.add(node.id);
    });
  }
  if (mode === 'all') {
    nodes.forEach((node) => {
      if (isInViewport(node)) visibleIds.add(node.id);
    });
  }
  if (mode === 'neighborhood' && selectedNeighbors.size === 0) {
    clusters.forEach((cluster) => cluster.representativeIds.forEach((id) => visibleIds.add(id)));
  }

  nodes.forEach((node) => {
    const matchesSearch = normalizedSearch.length > 0 &&
      `${node.name} ${node.type} ${node.observations}`.toLowerCase().includes(normalizedSearch);
    if (matchesSearch || selectedNeighbors.has(node.id)) visibleIds.add(node.id);
  });

  // A relation search should reveal both endpoints, even when the edge itself
  // is not in the current viewport or the page is in overview mode.
  if (normalizedSearch) {
    edges.forEach((edge) => {
      if (`${edge.from_name} ${edge.to_name} ${edge.relation_type}`.toLowerCase().includes(normalizedSearch)) {
        visibleIds.add(edge.from_name);
        visibleIds.add(edge.to_name);
      }
    });
  }

  if (mode === 'neighborhood' && selectedNeighbors.size > 0) {
    selectedNeighbors.forEach((id) => visibleIds.add(id));
  }

  const visibleNodes = nodes.filter((node) => visibleIds.has(node.id));
  const visibleEdges = edges.filter((edge) => visibleIds.has(edge.from_name) && visibleIds.has(edge.to_name));

  return {
    nodes: visibleNodes,
    edges: visibleEdges,
    clusters,
    loadedNodeCount: nodes.length,
    visibleNodeCount: visibleNodes.length,
    visibleEdgeCount: visibleEdges.length,
  };
}

/**
 * Order drawable edges by the detail most useful to the current graph view.
 * The complete edge set remains available to selection/statistics callers;
 * consumers can use this only when they need a bounded render budget.
 */
export function prioritizeGraphEdges(
  edges: CognitiveEdge[],
  nodes: LayoutNode[],
  selectedId: string | null,
  zoom: number,
  maxEdges = edges.length,
): CognitiveEdge[] {
  const degree = new Map(nodes.map((item) => [item.id, item.degree]));
  const selected = selectedId
    ? new Set(edges.filter((item) => item.from_name === selectedId || item.to_name === selectedId))
    : new Set<CognitiveEdge>();
  const zoomBoost = zoom > 1 ? zoom : 1;

  return [...edges]
    .map((item, index) => ({ item, index }))
    .sort((left, right) => {
      const leftSelected = selected.has(left.item) ? 1 : 0;
      const rightSelected = selected.has(right.item) ? 1 : 0;
      const leftConfidence = left.item.confidence ?? 0;
      const rightConfidence = right.item.confidence ?? 0;
      const leftDegree = (degree.get(left.item.from_name) ?? 0) + (degree.get(left.item.to_name) ?? 0);
      const rightDegree = (degree.get(right.item.from_name) ?? 0) + (degree.get(right.item.to_name) ?? 0);

      return rightSelected - leftSelected
        || (rightConfidence - leftConfidence) * zoomBoost
        || rightDegree - leftDegree
        || left.index - right.index;
    })
    .slice(0, Math.max(0, maxEdges))
    .map(({ item }) => item);
}
