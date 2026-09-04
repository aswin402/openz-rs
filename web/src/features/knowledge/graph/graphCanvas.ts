import { stableHash } from '../graphLayout';
import type { GraphCluster, LayoutNode } from '../graphLayout';
import type { EdgeProvenance } from '../graphSemantics';

export interface RenderNode extends LayoutNode {
  vx: number;
  vy: number;
  isPinned: boolean;
  importance: number;
  clusterId: string;
}

export interface GraphEdge {
  source: RenderNode;
  target: RenderNode;
  type: string;
  confidence: number | null;
  provenance: EdgeProvenance;
  sourceContext: string | null;
}

export interface SpaceStar {
  x: number;
  y: number;
  radius: number;
  alpha: number;
}

export interface Viewport {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export type GraphDisplayState = {
  showLabels: boolean;
  showGlow: boolean;
  showParticles: boolean;
  curvedLinks: boolean;
  showSpaceField: boolean;
  showGrid: boolean;
  showOrbits: boolean;
};

export const MAX_RENDERED_EDGES = 1800;
export const MAX_PARTICLE_EDGES = 720;
export const MAX_PARTICLES = 240;

const NODE_COLOR_PALETTE: Array<{ main: string; glow: string }> = [
  { main: '#f59e0b', glow: 'rgba(245, 158, 11, 0.45)' },
  { main: '#3b82f6', glow: 'rgba(59, 130, 246, 0.42)' },
  { main: '#10b981', glow: 'rgba(16, 185, 129, 0.42)' },
  { main: '#ec4899', glow: 'rgba(236, 72, 153, 0.42)' },
  { main: '#f43f5e', glow: 'rgba(244, 63, 94, 0.42)' },
  { main: '#a855f7', glow: 'rgba(168, 85, 247, 0.42)' },
  { main: '#06b6d4', glow: 'rgba(6, 182, 212, 0.42)' },
  { main: '#f97316', glow: 'rgba(249, 115, 22, 0.42)' },
  { main: '#38bdf8', glow: 'rgba(56, 189, 248, 0.42)' },
  { main: '#14b8a6', glow: 'rgba(20, 184, 166, 0.42)' },
  { main: '#eab308', glow: 'rgba(234, 179, 8, 0.42)' },
  { main: '#94a3b8', glow: 'rgba(148, 163, 184, 0.34)' },
];

export function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false);
}

export function colorHash(value: string): number {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}

export function getNodeColor(type: string): { main: string; glow: string } {
  const key = type.trim().toLowerCase() || 'unknown';
  return NODE_COLOR_PALETTE[colorHash(key) % NODE_COLOR_PALETTE.length];
}

export function colorWithAlpha(color: string, alpha: number): string {
  const hex = color.replace('#', '');
  const red = Number.parseInt(hex.slice(0, 2), 16);
  const green = Number.parseInt(hex.slice(2, 4), 16);
  const blue = Number.parseInt(hex.slice(4, 6), 16);
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
}

export const SPACE_STARS: SpaceStar[] = Array.from({ length: 96 }, (_, index) => {
  const hash = stableHash(`space-star:${index}`);
  const secondary = stableHash(`space-star:${index}:secondary`);
  return {
    x: (hash % 10000) / 10000,
    y: (secondary % 10000) / 10000,
    radius: 0.35 + (hash % 100) / 220,
    alpha: 0.16 + (secondary % 100) / 260,
  };
});

export function distanceToSegment(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSquared = dx * dx + dy * dy;
  if (lengthSquared === 0) return Math.hypot(px - ax, py - ay);
  const projection = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lengthSquared));
  return Math.hypot(px - (ax + projection * dx), py - (ay + projection * dy));
}

export function drawArrowhead(
  context: CanvasRenderingContext2D,
  source: RenderNode,
  target: RenderNode,
  color: string,
  scale: number,
) {
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const length = Math.hypot(dx, dy);
  if (length < 1) return;
  const unitX = dx / length;
  const unitY = dy / length;
  const tipX = target.x - unitX * (target.radius + 2);
  const tipY = target.y - unitY * (target.radius + 2);
  const size = Math.min(7, Math.max(3.5, scale * 4.5));
  const baseX = tipX - unitX * size;
  const baseY = tipY - unitY * size;
  const wingX = -unitY * size * 0.55;
  const wingY = unitX * size * 0.55;
  context.beginPath();
  context.moveTo(tipX, tipY);
  context.lineTo(baseX + wingX, baseY + wingY);
  context.lineTo(baseX - wingX, baseY - wingY);
  context.closePath();
  context.fillStyle = color;
  context.fill();
}

export function drawBackground(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  showGrid: boolean,
  showSpaceField: boolean,
) {
  context.clearRect(0, 0, width, height);
  const gradient = context.createRadialGradient(width * 0.48, height * 0.42, 20, width * 0.48, height * 0.42, Math.max(width, height) * 0.9);
  gradient.addColorStop(0, '#111827');
  gradient.addColorStop(0.52, '#090d17');
  gradient.addColorStop(1, '#04060b');
  context.fillStyle = gradient;
  context.fillRect(0, 0, width, height);

  if (showGrid) {
    context.strokeStyle = 'rgba(71, 85, 105, 0.08)';
    context.lineWidth = 1;
    const gridStep = 56;
    for (let x = 0; x <= width; x += gridStep) {
      context.beginPath();
      context.moveTo(x, 0);
      context.lineTo(x, height);
      context.stroke();
    }
    for (let y = 0; y <= height; y += gridStep) {
      context.beginPath();
      context.moveTo(0, y);
      context.lineTo(width, y);
      context.stroke();
    }
  }

  if (showSpaceField) {
    SPACE_STARS.forEach((star) => {
      context.beginPath();
      context.arc(star.x * width, star.y * height, star.radius, 0, Math.PI * 2);
      context.fillStyle = `rgba(226, 232, 240, ${star.alpha})`;
      context.fill();
    });
  }
}

export function drawClusters(
  context: CanvasRenderingContext2D,
  clusters: readonly GraphCluster[],
  hoveredCluster: GraphCluster | null,
  transform: { x: number; y: number; k: number },
  canvasWidth: number,
  canvasHeight: number,
) {
  clusters.forEach((cluster) => {
    const isHovered = hoveredCluster?.id === cluster.id;
    const palette = getNodeColor(cluster.type);
    const left = -transform.x / transform.k - 100;
    const top = -transform.y / transform.k - 100;
    const right = (canvasWidth - transform.x) / transform.k + 100;
    const bottom = (canvasHeight - transform.y) / transform.k + 100;
    if (cluster.x + cluster.radius < left || cluster.x - cluster.radius > right || cluster.y + cluster.radius < top || cluster.y - cluster.radius > bottom) return;
    const halo = context.createRadialGradient(
      cluster.x,
      cluster.y,
      cluster.radius * 0.06,
      cluster.x,
      cluster.y,
      cluster.radius,
    );
    halo.addColorStop(0, colorWithAlpha(palette.main, isHovered ? 0.16 : 0.075));
    halo.addColorStop(0.48, colorWithAlpha(palette.main, isHovered ? 0.07 : 0.028));
    halo.addColorStop(1, colorWithAlpha(palette.main, 0));
    context.beginPath();
    context.arc(cluster.x, cluster.y, cluster.radius, 0, Math.PI * 2);
    context.fillStyle = halo;
    context.fill();
    context.strokeStyle = isHovered ? colorWithAlpha(palette.main, 0.52) : colorWithAlpha(palette.main, 0.16);
    context.lineWidth = isHovered ? 1.3 : 0.7;
    context.setLineDash(isHovered ? [5, 4] : [2, 8]);
    context.beginPath();
    context.arc(cluster.x, cluster.y, cluster.radius * 0.82, 0, Math.PI * 2);
    context.stroke();
    context.setLineDash([]);
    context.beginPath();
    context.arc(cluster.x, cluster.y, 2.5 + Math.min(5, Math.sqrt(cluster.nodeIds.length)), 0, Math.PI * 2);
    context.fillStyle = palette.main;
    context.globalAlpha = isHovered ? 0.95 : 0.62;
    context.fill();
    context.globalAlpha = 1;
    if (transform.k < 1.15 || isHovered) {
      context.font = '600 10px Inter, system-ui, sans-serif';
      context.fillStyle = isHovered ? '#f8fafc' : 'rgba(203, 213, 225, 0.7)';
      context.textAlign = 'center';
      context.fillText(cluster.type + ' · ' + cluster.nodeIds.length, cluster.x, cluster.y + cluster.radius + 15);
      context.textAlign = 'start';
    }
  });
}

export function collectConnectedNodeIds(activeNode: RenderNode | null, edges: readonly GraphEdge[]): Set<string> {
  const connectedIds = new Set<string>();
  if (!activeNode) return connectedIds;
  connectedIds.add(activeNode.id);
  edges.forEach((edge) => {
    if (edge.source.id === activeNode.id) connectedIds.add(edge.target.id);
    if (edge.target.id === activeNode.id) connectedIds.add(edge.source.id);
  });
  return connectedIds;
}

export function drawOrbits(
  context: CanvasRenderingContext2D,
  activeNode: RenderNode | null,
  visibleNodes: readonly RenderNode[],
  connectedIds: ReadonlySet<string>,
  showOrbits: boolean,
) {
  if (!activeNode || !showOrbits) return;
  const neighbors = visibleNodes.filter((node) => node.id !== activeNode.id && connectedIds.has(node.id)).slice(0, 160);
  context.save();
  context.setLineDash([2, 5]);
  context.strokeStyle = activeNode.color;
  context.globalAlpha = 0.22;
  neighbors.forEach((neighbor) => {
    context.beginPath();
    context.arc(neighbor.x, neighbor.y, Math.min(28, neighbor.radius + 10), 0, Math.PI * 2);
    context.stroke();
  });
  context.setLineDash([]);
  context.globalAlpha = 0.38;
  context.beginPath();
  context.arc(activeNode.x, activeNode.y, activeNode.radius + 13, 0, Math.PI * 2);
  context.stroke();
  context.restore();
}

function sameEdge(left: GraphEdge | null, right: GraphEdge | null): boolean {
  return Boolean(left && right && left.source.id === right.source.id && left.target.id === right.target.id && left.type === right.type);
}

export function drawEdges(
  context: CanvasRenderingContext2D,
  edges: readonly GraphEdge[],
  activeNode: RenderNode | null,
  hoveredEdge: GraphEdge | null,
  selectedEdge: GraphEdge | null,
  transform: { x: number; y: number; k: number },
  curvedLinks: boolean,
  particlesEnabled: boolean,
  now: number,
) {
  edges.forEach((edge, edgeIndex) => {
    const source = edge.source;
    const target = edge.target;
    const connected = Boolean(activeNode && (source.id === activeNode.id || target.id === activeNode.id));
    const edgeIsHovered = sameEdge(edge, hoveredEdge);
    const edgeIsSelected = sameEdge(edge, selectedEdge);
    const dimmed = Boolean(activeNode && !connected && !edgeIsHovered && !edgeIsSelected);
    const highConfidence = edge.confidence !== null && edge.confidence >= 0.82;
    const sourceColor = getNodeColor(source.type).main;
    const ordinaryAlpha = dimmed
      ? 0.025
      : edge.confidence === null
        ? 0.1
        : 0.1 + edge.confidence * 0.24;
    const edgeColor = edgeIsHovered
      ? '#f8fafc'
      : edgeIsSelected
        ? '#67e8f9'
        : connected
          ? colorWithAlpha(activeNode?.color || sourceColor, 0.82)
          : highConfidence
            ? colorWithAlpha(sourceColor, 0.62)
            : colorWithAlpha(sourceColor, ordinaryAlpha);
    context.beginPath();
    if (curvedLinks) {
      const midX = (source.x + target.x) / 2 + (target.y - source.y) * 0.12;
      const midY = (source.y + target.y) / 2 - (target.x - source.x) * 0.12;
      context.moveTo(source.x, source.y);
      context.quadraticCurveTo(midX, midY, target.x, target.y);
    } else {
      context.moveTo(source.x, source.y);
      context.lineTo(target.x, target.y);
    }
    context.strokeStyle = edgeColor;
    context.lineWidth = edgeIsHovered || edgeIsSelected ? 2.2 : connected ? 1.7 : highConfidence ? 1.05 : 0.62;
    context.setLineDash(edge.provenance === 'not_recorded' ? [4, 5] : []);
    context.stroke();
    context.setLineDash([]);
    if (transform.k > 0.7 && (connected || edgeIsHovered || edgeIsSelected)) {
      drawArrowhead(context, source, target, edgeColor, transform.k);
    }
    if (particlesEnabled && edgeIndex < MAX_PARTICLES && (connected || !activeNode)) {
      const pulse = (now / 2200 + colorHash(source.id + ':' + target.id) / 1000) % 1;
      context.beginPath();
      context.arc(source.x + (target.x - source.x) * pulse, source.y + (target.y - source.y) * pulse, connected ? 1.8 : 1.05, 0, Math.PI * 2);
      context.fillStyle = connected ? (activeNode?.color || sourceColor) : colorWithAlpha(sourceColor, 0.58);
      context.globalAlpha = connected ? 0.9 : 0.38;
      context.fill();
      context.globalAlpha = 1;
    }
  });
}

export function drawNodes(
  context: CanvasRenderingContext2D,
  nodes: readonly RenderNode[],
  activeNode: RenderNode | null,
  hoveredNode: RenderNode | null,
  selectedNode: RenderNode | null,
  connectedIds: ReadonlySet<string>,
  activeSearch: string,
  transform: { x: number; y: number; k: number },
  canvasWidth: number,
  showGlow: boolean,
  showLabels: boolean,
) {
  const maxDegree = Math.max(...nodes.map((node) => node.degree), 0);
  const labelBudget = Math.min(72, Math.max(18, Math.floor(canvasWidth / 18)));
  const labelThreshold = nodes.length > 80 ? Math.max(3, Math.ceil(maxDegree * 0.35)) : Math.max(1, Math.ceil(maxDegree * 0.2));
  const query = activeSearch.trim().toLowerCase();
  const matchesSearch = (node: RenderNode) => Boolean(query) &&
    (node.name + ' ' + node.type + ' ' + node.observations).toLowerCase().includes(query);
  const labelIds = new Set(
    nodes
      .filter((node) => hoveredNode?.id === node.id || selectedNode?.id === node.id || matchesSearch(node))
      .map((node) => node.id),
  );
  const maximumLabels = Math.max(labelBudget, labelIds.size);
  const labelCandidates = [...nodes].sort((left, right) => {
    const leftHub = left.degree >= labelThreshold ? 1 : 0;
    const rightHub = right.degree >= labelThreshold ? 1 : 0;
    return rightHub - leftHub || right.degree - left.degree || right.importance - left.importance || left.name.localeCompare(right.name);
  });
  for (const node of labelCandidates) {
    if (labelIds.has(node.id)) continue;
    const isHub = node.degree > 0 && node.degree >= labelThreshold;
    if (!isHub && transform.k <= 1.15) continue;
    if (labelIds.size >= maximumLabels) break;
    labelIds.add(node.id);
  }

  nodes.forEach((node) => {
    const isConnected = activeNode ? connectedIds.has(node.id) : true;
    const isHovered = hoveredNode?.id === node.id;
    const isSelected = selectedNode?.id === node.id;
    const isSearchMatch = matchesSearch(node);
    const palette = getNodeColor(node.type);
    const renderRadius = Math.max(2 / Math.max(transform.k, 0.01), Math.min(14, 3.5 + Math.sqrt(node.degree) * 1.8));
    const alpha = activeNode && !isConnected ? 0.12 : 0.72 + node.importance * 0.28;
    const isImportant = node.importance >= 0.62;
    const stronglyEmphasized = isSelected || isSearchMatch;
    context.save();
    context.globalAlpha = alpha;
    if (showGlow && (stronglyEmphasized || isHovered || (isConnected && node.degree > 0))) {
      const glowRadius = renderRadius * (stronglyEmphasized ? 4.5 : isHovered ? 3.4 : 1.9);
      const glow = context.createRadialGradient(node.x, node.y, renderRadius * 0.25, node.x, node.y, glowRadius);
      glow.addColorStop(0, stronglyEmphasized ? colorWithAlpha(palette.main, 0.68) : palette.glow);
      glow.addColorStop(1, colorWithAlpha(palette.main, 0));
      context.fillStyle = glow;
      context.beginPath();
      context.arc(node.x, node.y, glowRadius, 0, Math.PI * 2);
      context.fill();
    }
    if (isSearchMatch) {
      context.beginPath();
      context.arc(node.x, node.y, renderRadius + 5, 0, Math.PI * 2);
      context.strokeStyle = '#7dd3fc';
      context.lineWidth = 1.3;
      context.setLineDash([3, 3]);
      context.stroke();
      context.setLineDash([]);
    }
    context.beginPath();
    context.arc(node.x, node.y, renderRadius + (isSelected ? 1.5 : 0), 0, Math.PI * 2);
    context.fillStyle = palette.main;
    context.fill();
    context.lineWidth = isHovered || isSelected || isImportant ? 1.8 : 0.7;
    context.strokeStyle = isHovered || isSelected ? '#f8fafc' : isImportant ? palette.main : '#020617';
    context.stroke();
    if (isImportant && !isSelected) {
      context.beginPath();
      context.arc(node.x, node.y, renderRadius + 3.5, 0, Math.PI * 2);
      context.strokeStyle = palette.glow;
      context.lineWidth = 0.8;
      context.globalAlpha = 0.55;
      context.stroke();
      context.globalAlpha = 1;
    }
    if (isSelected) {
      context.beginPath();
      context.arc(node.x, node.y, renderRadius + 5, 0, Math.PI * 2);
      context.strokeStyle = palette.main;
      context.lineWidth = 1.2;
      context.stroke();
    }
    context.restore();

    if (showLabels && labelIds.has(node.id)) {
      context.save();
      context.globalAlpha = alpha;
      context.font = (isSelected ? '600 ' : '500 ') + (isSelected ? '11px' : '10px') + ' Inter, system-ui, sans-serif';
      context.fillStyle = isHovered || isSelected ? '#f8fafc' : '#cbd5e1';
      context.shadowColor = 'rgba(0, 0, 0, 0.95)';
      context.shadowBlur = 4;
      const label = node.name.length > 36 ? node.name.slice(0, 33) + '…' : node.name;
      context.fillText(label, node.x + renderRadius + 5, node.y + 3.5);
      context.restore();
    }
  });
}
