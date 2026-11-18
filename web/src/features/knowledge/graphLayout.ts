import type { CognitiveEdge, CognitiveNode } from '../../types/openz';
import { getCategoryDescriptor, normalizeGraphCategory } from './coreGraphBuilder';

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
const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

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

  const canonicalNodes = [...nodes].sort((a, b) => a.name.localeCompare(b.name));
  const nodeIndex = new Map(canonicalNodes.map((node, index) => [node.name, index]));
  const degrees = computeDegrees(canonicalNodes, edges);
  const sets = new DisjointSet(canonicalNodes.length);

  edges.forEach((edge) => {
    const from = nodeIndex.get(edge.from_name);
    const to = nodeIndex.get(edge.to_name);
    if (from !== undefined && to !== undefined) sets.union(from, to);
  });

  const groups = new Map<string, number[]>();
  canonicalNodes.forEach((_, index) => {
    const root = sets.find(index).toString();
    const group = groups.get(root) || [];
    group.push(index);
    groups.set(root, group);
  });

  const isolatedByType = new Map<string, number[]>();
  const connectedGroups: number[][] = [];
  groups.forEach((group) => {
    if (group.length === 1 && (degrees.get(canonicalNodes[group[0]].name) || 0) === 0) {
      const type = normalizeGraphCategory(canonicalNodes[group[0]].entity_type);
      const bucket = isolatedByType.get(type) || [];
      bucket.push(group[0]);
      isolatedByType.set(type, bucket);
    } else {
      connectedGroups.push(group);
    }
  });
  isolatedByType.forEach((group) => connectedGroups.push(group));

  const safeWidth = Math.max(320, Number.isFinite(width) ? width : 900);
  const safeHeight = Math.max(240, Number.isFinite(height) ? height : 560);
  const cx = safeWidth * 0.5;
  const cy = safeHeight * 0.5;

  const rawClusters = connectedGroups.map((group) => {
    const sortedGroup = [...group].sort((left, right) =>
      canonicalNodes[left].name.localeCompare(canonicalNodes[right].name),
    );
    const nodeIds = sortedGroup.map((index) => canonicalNodes[index].name);
    const maxDeg = Math.max(
      ...sortedGroup.map((index) => degrees.get(canonicalNodes[index].name) || 0),
      0,
    );
    const radius = Math.max(76, Math.min(360, 56 + Math.sqrt(group.length) * 34));
    const representativeIds = [...sortedGroup]
      .sort((left, right) => {
        const degreeDelta =
          (degrees.get(canonicalNodes[right].name) || 0) -
          (degrees.get(canonicalNodes[left].name) || 0);
        return (
          degreeDelta ||
          canonicalNodes[left].name.localeCompare(canonicalNodes[right].name)
        );
      })
      .slice(0, OVERVIEW_REPRESENTATIVE_LIMIT)
      .map((index) => canonicalNodes[index].name);

    const typeCounts = new Map<string, number>();
    sortedGroup.forEach((index) => {
      const type = normalizeGraphCategory(canonicalNodes[index].entity_type);
      typeCounts.set(type, (typeCounts.get(type) || 0) + 1);
    });
    const type = [...typeCounts.entries()].sort(
      (left, right) => right[1] - left[1] || left[0].localeCompare(right[0]),
    )[0][0];

    return {
      id: `cluster:${stableHash(nodeIds.join('|')).toString(16)}`,
      type,
      nodeIds,
      maxDeg,
      radius,
      representativeIds,
    };
  });

  // Rank clusters deterministically so the richest constellation sits at galactic center
  // and satellite constellations spiral cleanly around it without clumping.
  const ranked = [...rawClusters].sort(
    (a, b) =>
      b.nodeIds.length - a.nodeIds.length ||
      b.maxDeg - a.maxDeg ||
      a.id.localeCompare(b.id),
  );

  const primaryRadius = ranked[0]?.radius ?? 120;
  const placed = ranked.map((cluster, rankIndex) => {
    if (rankIndex === 0) {
      return {
        id: cluster.id,
        type: cluster.type,
        nodeIds: cluster.nodeIds,
        x: cx,
        y: cy,
        radius: cluster.radius,
        representativeIds: cluster.representativeIds,
      };
    }
    const angle = rankIndex * GOLDEN_ANGLE;
    const dist = primaryRadius + cluster.radius + 70 + Math.sqrt(rankIndex - 1) * 150;
    return {
      id: cluster.id,
      type: cluster.type,
      nodeIds: cluster.nodeIds,
      x: cx + Math.cos(angle) * dist * 1.18,
      y: cy + Math.sin(angle) * dist * 0.88,
      radius: cluster.radius,
      representativeIds: cluster.representativeIds,
    };
  });

  return placed.sort((left, right) => left.id.localeCompare(right.id));
}

function layoutClusterConstellation(
  cluster: GraphCluster,
  clusterNodes: CognitiveNode[],
  clusterEdges: CognitiveEdge[],
  degrees: ReadonlyMap<string, number>,
): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  const sorted = [...clusterNodes].sort((a, b) => a.name.localeCompare(b.name));
  if (sorted.length === 0) return positions;

  if (sorted.length === 1) {
    positions.set(sorted[0].name, { x: cluster.x, y: cluster.y });
    return positions;
  }

  const maxDeg = Math.max(...sorted.map((n) => degrees.get(n.name) || 0), 0);

  // Disconnected category star cluster: Fermat sunflower spiral
  if (maxDeg === 0) {
    const step = Math.max(24, Math.min(38, cluster.radius / Math.sqrt(sorted.length + 1)));
    sorted.forEach((node, idx) => {
      const r = step * Math.sqrt(idx + 0.6);
      const theta = idx * GOLDEN_ANGLE;
      positions.set(node.name, {
        x: cluster.x + Math.cos(theta) * r,
        y: cluster.y + Math.sin(theta) * r,
      });
    });
    return positions;
  }

  // Connected constellation: Primary God Node at exact cluster center + BFS orbital shells + deterministic force relaxation
  const hubNode = [...sorted].sort((a, b) => {
    const degDiff = (degrees.get(b.name) || 0) - (degrees.get(a.name) || 0);
    return degDiff || a.name.localeCompare(b.name);
  })[0];

  const adjacency = new Map<string, string[]>();
  sorted.forEach((n) => adjacency.set(n.name, []));
  const canonicalEdges = [...clusterEdges].sort(
    (a, b) =>
      a.from_name.localeCompare(b.from_name) ||
      a.to_name.localeCompare(b.to_name) ||
      a.relation_type.localeCompare(b.relation_type),
  );
  canonicalEdges.forEach((edge) => {
    const listA = adjacency.get(edge.from_name);
    const listB = adjacency.get(edge.to_name);
    if (listA && listB) {
      if (!listA.includes(edge.to_name)) listA.push(edge.to_name);
      if (!listB.includes(edge.from_name)) listB.push(edge.from_name);
    }
  });
  adjacency.forEach((neighbors) => neighbors.sort((a, b) => a.localeCompare(b)));

  const depthMap = new Map<string, number>([[hubNode.name, 0]]);
  const parentMap = new Map<string, string>();
  const childrenMap = new Map<string, string[]>();
  sorted.forEach((n) => childrenMap.set(n.name, []));

  const queue: string[] = [hubNode.name];
  for (let head = 0; head < queue.length; head += 1) {
    const curr = queue[head];
    const currDepth = depthMap.get(curr) ?? 0;
    const neighbors = adjacency.get(curr) ?? [];
    for (const next of neighbors) {
      if (!depthMap.has(next)) {
        depthMap.set(next, currDepth + 1);
        parentMap.set(next, curr);
        childrenMap.get(curr)?.push(next);
        queue.push(next);
      }
    }
  }

  // Fallback for any unreachable node in cluster
  sorted.forEach((n) => {
    if (!depthMap.has(n.name)) {
      depthMap.set(n.name, 2);
      parentMap.set(n.name, hubNode.name);
      childrenMap.get(hubNode.name)?.push(n.name);
    }
  });

  const nodeByName = new Map(sorted.map((n) => [n.name, n]));
  const subtreeWeight = new Map<string, number>();
  const computeWeight = (id: string): number => {
    const kids = childrenMap.get(id) ?? [];
    let total = 1;
    for (const k of kids) {
      total += computeWeight(k);
    }
    subtreeWeight.set(id, total);
    return total;
  };
  computeWeight(hubNode.name);

  positions.set(hubNode.name, { x: cluster.x, y: cluster.y });

  const shell1 = [...(childrenMap.get(hubNode.name) ?? [])].sort((a, b) => {
    const catA = getCategoryDescriptor(nodeByName.get(a)?.entity_type).order;
    const catB = getCategoryDescriptor(nodeByName.get(b)?.entity_type).order;
    if (catA !== catB) return catA - catB;
    const degDiff = (degrees.get(b) || 0) - (degrees.get(a) || 0);
    return degDiff || a.localeCompare(b);
  });

  const totalShell1Weight = Math.max(
    1,
    shell1.reduce((acc, id) => acc + Math.sqrt(subtreeWeight.get(id) ?? 1), 0),
  );
  const baseR1 = Math.max(68, Math.min(220, 52 + Math.sqrt(sorted.length) * 24));
  const nodeAngle = new Map<string, number>();
  const nodeSpan = new Map<string, number>();

  let cursorAngle = -Math.PI / 2;
  shell1.forEach((id, idx) => {
    const w = Math.sqrt(subtreeWeight.get(id) ?? 1);
    const span = (w / totalShell1Weight) * Math.PI * 2;
    const mid = cursorAngle + span * 0.5;
    cursorAngle += span;
    nodeAngle.set(id, mid);
    nodeSpan.set(id, span);

    const hasChildren = (childrenMap.get(id)?.length ?? 0) > 0;
    const stagger = idx % 2 === 0 ? 0 : 18;
    const r = hasChildren ? baseR1 * 1.06 : baseR1 * 0.88 + stagger;
    positions.set(id, {
      x: cluster.x + Math.cos(mid) * r,
      y: cluster.y + Math.sin(mid) * r,
    });
  });

  // Place deeper shells (depth >= 2) cleanly in radial fans around their parent angle
  const maxDepth = Math.max(...depthMap.values(), 1);
  for (let d = 2; d <= maxDepth; d += 1) {
    const parentsAtPrev = sorted
      .map((n) => n.name)
      .filter((id) => depthMap.get(id) === d - 1);

    for (const pid of parentsAtPrev) {
      const kids = [...(childrenMap.get(pid) ?? [])].sort((a, b) => {
        const catA = getCategoryDescriptor(nodeByName.get(a)?.entity_type).order;
        const catB = getCategoryDescriptor(nodeByName.get(b)?.entity_type).order;
        return catA - catB || a.localeCompare(b);
      });
      if (kids.length === 0) continue;

      const pAngle = nodeAngle.get(pid) ?? 0;
      const pSpan = Math.min(Math.PI * 0.85, Math.max(0.42, (nodeSpan.get(pid) ?? 0.6) * 1.25));
      const shellRadius = baseR1 + (d - 1) * 92 + Math.min(60, kids.length * 3.5);

      kids.forEach((kid, kIdx) => {
        const frac = kids.length === 1 ? 0.5 : kIdx / (kids.length - 1);
        const angle = pAngle - pSpan * 0.5 + frac * pSpan;
        const ringOffset = (kIdx % 3) * 16;
        const r = shellRadius + ringOffset;
        nodeAngle.set(kid, angle);
        nodeSpan.set(kid, pSpan / Math.max(1, kids.length));
        positions.set(kid, {
          x: cluster.x + Math.cos(angle) * r,
          y: cluster.y + Math.sin(angle) * r,
        });
      });
    }
  }

  // Deterministic force-directed spring + repulsion relaxation (22 iterations)
  // Computed in strict canonical ID order so coordinates never depend on input ordering.
  const nCount = sorted.length;
  const xs = sorted.map((n) => positions.get(n.name)?.x ?? cluster.x);
  const ys = sorted.map((n) => positions.get(n.name)?.y ?? cluster.y);
  const idxMap = new Map(sorted.map((n, i) => [n.name, i]));
  const hubIdx = idxMap.get(hubNode.name) ?? 0;

  const edgePairs: Array<[number, number]> = [];
  canonicalEdges.forEach((e) => {
    const ia = idxMap.get(e.from_name);
    const ib = idxMap.get(e.to_name);
    if (ia !== undefined && ib !== undefined && ia !== ib) {
      edgePairs.push([ia, ib]);
    }
  });

  const iterations = nCount <= 260 ? 22 : 10;
  const minHubClearance = 36;

  for (let iter = 0; iter < iterations; iter += 1) {
    const fx = new Float64Array(nCount);
    const fy = new Float64Array(nCount);
    const cooling = 1 - iter / (iterations + 4);

    // Pairwise Coulomb repulsion
    for (let i = 0; i < nCount; i += 1) {
      for (let j = i + 1; j < nCount; j += 1) {
        const dx = xs[j] - xs[i];
        const dy = ys[j] - ys[i];
        const distSq = dx * dx + dy * dy + 1;
        if (distSq > 90_000) continue;
        const dist = Math.sqrt(distSq);
        const force = (950 / distSq) * cooling;
        const ux = (dx / dist) * force;
        const uy = (dy / dist) * force;
        fx[i] -= ux;
        fy[i] -= uy;
        fx[j] += ux;
        fy[j] += uy;
      }
    }

    // Hooke link springs
    for (const [ia, ib] of edgePairs) {
      const dx = xs[ib] - xs[ia];
      const dy = ys[ib] - ys[ia];
      const dist = Math.hypot(dx, dy) || 1;
      const degSum =
        (degrees.get(sorted[ia].name) || 1) + (degrees.get(sorted[ib].name) || 1);
      const ideal = Math.min(185, 58 + Math.sqrt(degSum) * 11);
      const pull = (dist - ideal) * 0.045 * cooling;
      const ux = (dx / dist) * pull;
      const uy = (dy / dist) * pull;
      fx[ia] += ux;
      fy[ia] += uy;
      fx[ib] -= ux;
      fy[ib] -= uy;
    }

    for (let i = 0; i < nCount; i += 1) {
      if (i === hubIdx) {
        xs[i] = cluster.x;
        ys[i] = cluster.y;
        continue;
      }
      const stepX = Math.max(-18, Math.min(18, fx[i]));
      const stepY = Math.max(-18, Math.min(18, fy[i]));
      xs[i] += stepX;
      ys[i] += stepY;

      // Guarantee every satellite stays outside the hub's inner clearance ring
      const rx = xs[i] - cluster.x;
      const ry = ys[i] - cluster.y;
      const rDist = Math.hypot(rx, ry);
      if (rDist < minHubClearance) {
        const fallbackAngle = (i * GOLDEN_ANGLE) % (Math.PI * 2);
        const ux = rDist > 0.001 ? rx / rDist : Math.cos(fallbackAngle);
        const uy = rDist > 0.001 ? ry / rDist : Math.sin(fallbackAngle);
        xs[i] = cluster.x + ux * minHubClearance;
        ys[i] = cluster.y + uy * minHubClearance;
      }
    }
  }

  sorted.forEach((n, i) => {
    positions.set(n.name, { x: xs[i], y: ys[i] });
  });

  return positions;
}

export function layoutNodes(
  nodes: CognitiveNode[],
  edges: CognitiveEdge[],
  width: number,
  height: number,
): LayoutNode[] {
  if (nodes.length === 0) return [];
  const canonicalNodes = [...nodes].sort((a, b) => a.name.localeCompare(b.name));
  const degrees = computeDegrees(canonicalNodes, edges);
  const clusters = buildClusters(canonicalNodes, edges, width, height);
  const nodeByName = new Map(canonicalNodes.map((n) => [n.name, n]));

  const coordsByName = new Map<string, { x: number; y: number }>();
  for (const cluster of clusters) {
    const idSet = new Set(cluster.nodeIds);
    const cNodes = cluster.nodeIds
      .map((id) => nodeByName.get(id))
      .filter((n): n is CognitiveNode => Boolean(n));
    const cEdges = edges.filter(
      (e) => idSet.has(e.from_name) && idSet.has(e.to_name),
    );
    const localCoords = layoutClusterConstellation(cluster, cNodes, cEdges, degrees);
    localCoords.forEach((pt, id) => coordsByName.set(id, pt));
  }

  return nodes.map((node) => {
    const degree = degrees.get(node.name) || 0;
    const radius = Math.min(15, Math.max(3.8, 3.8 + Math.sqrt(degree) * 1.9));
    const point = coordsByName.get(node.name) ?? stablePoint(node.name, width, height);
    return {
      id: node.name,
      name: node.name,
      type: normalizeGraphCategory(node.entity_type),
      observations: node.observations,
      degree,
      x: point.x,
      y: point.y,
      radius,
      color: 'var(--primary)',
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

  if (mode === 'overview' || mode === 'all') {
    nodes.forEach((node) => {
      if (isInViewport(node)) visibleIds.add(node.id);
    });
  }
  if (mode === 'neighborhood' && selectedNeighbors.size === 0) {
    clusters.forEach((cluster) => cluster.representativeIds.forEach((id) => visibleIds.add(id)));
  }

  nodes.forEach((node) => {
    const matchesSearch =
      normalizedSearch.length > 0 &&
      `${node.name} ${node.type} ${node.observations}`.toLowerCase().includes(normalizedSearch);
    if (matchesSearch || selectedNeighbors.has(node.id)) visibleIds.add(node.id);
  });

  if (normalizedSearch) {
    edges.forEach((edge) => {
      if (
        `${edge.from_name} ${edge.to_name} ${edge.relation_type}`
          .toLowerCase()
          .includes(normalizedSearch)
      ) {
        visibleIds.add(edge.from_name);
        visibleIds.add(edge.to_name);
      }
    });
  }

  if (mode === 'neighborhood' && selectedNeighbors.size > 0) {
    selectedNeighbors.forEach((id) => visibleIds.add(id));
  }

  const visibleNodes = nodes.filter((node) => visibleIds.has(node.id));
  const visibleEdges = edges.filter(
    (edge) => visibleIds.has(edge.from_name) && visibleIds.has(edge.to_name),
  );

  return {
    nodes: visibleNodes,
    edges: visibleEdges,
    clusters,
    loadedNodeCount: nodes.length,
    visibleNodeCount: visibleNodes.length,
    visibleEdgeCount: visibleEdges.length,
  };
}

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
      const leftDegree =
        (degree.get(left.item.from_name) ?? 0) + (degree.get(left.item.to_name) ?? 0);
      const rightDegree =
        (degree.get(right.item.from_name) ?? 0) + (degree.get(right.item.to_name) ?? 0);

      return (
        rightSelected - leftSelected ||
        (rightConfidence - leftConfidence) * zoomBoost ||
        rightDegree - leftDegree ||
        left.index - right.index
      );
    })
    .slice(0, Math.max(0, maxEdges))
    .map(({ item }) => item);
}
