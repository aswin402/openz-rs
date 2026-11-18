import {
  THEME_FAMILIES,
  mixHex,
  relativeLuminance,
  withHexAlpha,
} from '../../../shared/lib/colorEngine';
import { getCategoryDescriptor, normalizeGraphCategory } from '../coreGraphBuilder';
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
  depth: number;
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

interface GraphCanvasTheme {
  mode: 'dark' | 'light';
  backgroundHex: string;
  cardHex: string;
  sidebarHex: string;
  foregroundHex: string;
  mutedForegroundHex: string;
  primaryHex: string;
  infoHex: string;
}

let cachedSig = '';
let cachedTheme: GraphCanvasTheme | null = null;

function readCssHex(style: CSSStyleDeclaration | null, varName: string, fallback: string): string {
  if (!style) return fallback;
  const raw = style.getPropertyValue(varName).trim();
  return raw.startsWith('#') ? raw : fallback;
}

export function resolveGraphCanvasTheme(): GraphCanvasTheme {
  const fallback = THEME_FAMILIES[0].dark;
  const style =
    typeof document !== 'undefined' ? getComputedStyle(document.documentElement) : null;
  const backgroundHex = readCssHex(style, '--background', fallback.backgroundHex);
  const cardHex = readCssHex(style, '--card', fallback.cardHex);
  const sidebarHex = readCssHex(style, '--sidebar', fallback.sidebarHex);
  const foregroundHex = readCssHex(style, '--foreground', fallback.foregroundHex);
  const mutedForegroundHex = readCssHex(style, '--muted-foreground', fallback.mutedForegroundHex);
  const primaryHex = readCssHex(style, '--primary', fallback.primaryHex);
  const infoHex = readCssHex(style, '--info', fallback.infoHex);

  const sig = `${backgroundHex}|${primaryHex}|${foregroundHex}|${infoHex}`;
  if (cachedTheme && cachedSig === sig) {
    return cachedTheme;
  }

  const mode: 'dark' | 'light' = relativeLuminance(backgroundHex) > 0.42 ? 'light' : 'dark';

  cachedSig = sig;
  cachedTheme = {
    mode,
    backgroundHex,
    cardHex,
    sidebarHex,
    foregroundHex,
    mutedForegroundHex,
    primaryHex,
    infoHex,
  };
  return cachedTheme;
}

export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false)
  );
}

export function colorHash(value: string): number {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}

export function getNodeColor(type: string): { main: string; glow: string } {
  const theme = resolveGraphCanvasTheme();
  const descriptor = getCategoryDescriptor(type);
  const main = theme.mode === 'dark' ? descriptor.darkHex : descriptor.lightHex;
  const glowAlpha = theme.mode === 'dark' ? 0.46 : 0.24;
  return {
    main,
    glow: withHexAlpha(main, glowAlpha),
  };
}

export function colorWithAlpha(color: string, alpha: number): string {
  return withHexAlpha(color, alpha);
}

export const SPACE_STARS: SpaceStar[] = Array.from({ length: 144 }, (_, index) => {
  const hash = stableHash(`space-star:${index}`);
  const secondary = stableHash(`space-star:${index}:secondary`);
  const depth = (index % 3) + 1;
  return {
    x: (hash % 10000) / 10000,
    y: (secondary % 10000) / 10000,
    radius: depth === 3 ? 1.05 : depth === 2 ? 0.72 : 0.45,
    alpha: depth === 3 ? 0.42 : depth === 2 ? 0.24 : 0.13,
    depth,
  };
});

export function distanceToSegment(
  px: number,
  py: number,
  ax: number,
  ay: number,
  bx: number,
  by: number,
): number {
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
  const tipX = target.x - unitX * (target.radius + 2.5);
  const tipY = target.y - unitY * (target.radius + 2.5);
  const size = Math.min(6.5, Math.max(3.2, scale * 4));
  const baseX = tipX - unitX * size;
  const baseY = tipY - unitY * size;
  const wingX = -unitY * size * 0.52;
  const wingY = unitX * size * 0.52;
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
  transform?: { x: number; y: number; k: number },
) {
  const theme = resolveGraphCanvasTheme();
  context.clearRect(0, 0, width, height);

  // Deep obsidian cosmos backdrop with subtle primary & indigo nebula depth
  const deepBase =
    theme.mode === 'dark'
      ? mixHex(theme.backgroundHex, '#05070d', 0.55)
      : theme.backgroundHex;
  const coreNebula =
    theme.mode === 'dark'
      ? mixHex(deepBase, theme.primaryHex, 0.065)
      : mixHex(theme.cardHex, theme.primaryHex, 0.03);
  const outerVoid =
    theme.mode === 'dark'
      ? mixHex(deepBase, '#030408', 0.45)
      : mixHex(theme.backgroundHex, theme.sidebarHex, 0.65);

  const gradient = context.createRadialGradient(
    width * 0.5,
    height * 0.46,
    16,
    width * 0.5,
    height * 0.46,
    Math.max(width, height) * 0.88,
  );
  gradient.addColorStop(0, coreNebula);
  gradient.addColorStop(0.52, deepBase);
  gradient.addColorStop(1, outerVoid);
  context.fillStyle = gradient;
  context.fillRect(0, 0, width, height);

  if (showGrid) {
    context.strokeStyle = colorWithAlpha(
      theme.mutedForegroundHex,
      theme.mode === 'dark' ? 0.08 : 0.13,
    );
    context.lineWidth = 0.75;
    const gridStep = 64;
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
    const starColor = theme.mode === 'dark' ? '#cbd5e1' : theme.mutedForegroundHex;
    const starMultiplier = theme.mode === 'dark' ? 1 : 0.45;
    const tx = transform?.x ?? 0;
    const ty = transform?.y ?? 0;

    SPACE_STARS.forEach((star) => {
      const parallax = star.depth * 0.035;
      const sx = (((star.x * width + tx * parallax) % width) + width) % width;
      const sy = (((star.y * height + ty * parallax) % height) + height) % height;
      context.beginPath();
      context.arc(sx, sy, star.radius, 0, Math.PI * 2);
      context.fillStyle = colorWithAlpha(starColor, star.alpha * starMultiplier);
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
  const theme = resolveGraphCanvasTheme();
  clusters.forEach((cluster) => {
    const isHovered = hoveredCluster?.id === cluster.id;
    const palette = getNodeColor(cluster.type);
    const descriptor = getCategoryDescriptor(cluster.type);
    const left = -transform.x / transform.k - 200;
    const top = -transform.y / transform.k - 200;
    const right = (canvasWidth - transform.x) / transform.k + 200;
    const bottom = (canvasHeight - transform.y) / transform.k + 200;
    if (
      cluster.x + cluster.radius < left ||
      cluster.x - cluster.radius > right ||
      cluster.y + cluster.radius < top ||
      cluster.y - cluster.radius > bottom
    ) {
      return;
    }

    // Soft cosmic nebula wash behind the constellation (no harsh dashed boxes)
    const halo = context.createRadialGradient(
      cluster.x,
      cluster.y,
      cluster.radius * 0.04,
      cluster.x,
      cluster.y,
      cluster.radius * 1.15,
    );
    halo.addColorStop(
      0,
      colorWithAlpha(palette.main, isHovered ? 0.14 : theme.mode === 'dark' ? 0.065 : 0.045),
    );
    halo.addColorStop(
      0.55,
      colorWithAlpha(palette.main, isHovered ? 0.055 : theme.mode === 'dark' ? 0.022 : 0.015),
    );
    halo.addColorStop(1, colorWithAlpha(palette.main, 0));

    context.beginPath();
    context.arc(cluster.x, cluster.y, cluster.radius * 1.15, 0, Math.PI * 2);
    context.fillStyle = halo;
    context.fill();

    if (isHovered) {
      context.strokeStyle = colorWithAlpha(palette.main, 0.35);
      context.lineWidth = 1;
      context.setLineDash([4, 6]);
      context.beginPath();
      context.arc(cluster.x, cluster.y, cluster.radius * 0.9, 0, Math.PI * 2);
      context.stroke();
      context.setLineDash([]);
    }

    if (transform.k < 0.72 || isHovered) {
      context.font = '600 10.5px Inter, system-ui, sans-serif';
      context.fillStyle = isHovered
        ? theme.foregroundHex
        : colorWithAlpha(palette.main, 0.78);
      context.textAlign = 'center';
      context.fillText(
        `${descriptor.shortLabel} · ${cluster.nodeIds.length}`,
        cluster.x,
        cluster.y + cluster.radius + 16,
      );
      context.textAlign = 'start';
    }
  });
}

export function collectConnectedNodeIds(
  activeNode: RenderNode | null,
  edges: readonly GraphEdge[],
): Set<string> {
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
  const neighbors = visibleNodes
    .filter((node) => node.id !== activeNode.id && connectedIds.has(node.id))
    .slice(0, 160);
  const palette = getNodeColor(activeNode.type);
  context.save();
  context.setLineDash([2, 5]);
  context.strokeStyle = palette.main;
  context.globalAlpha = 0.24;
  neighbors.forEach((neighbor) => {
    context.beginPath();
    context.arc(neighbor.x, neighbor.y, Math.min(24, neighbor.radius + 8), 0, Math.PI * 2);
    context.stroke();
  });
  context.setLineDash([]);
  context.globalAlpha = 0.42;
  context.beginPath();
  context.arc(activeNode.x, activeNode.y, activeNode.radius + 12, 0, Math.PI * 2);
  context.stroke();
  context.restore();
}

function sameEdge(left: GraphEdge | null, right: GraphEdge | null): boolean {
  return Boolean(
    left &&
      right &&
      left.source.id === right.source.id &&
      left.target.id === right.target.id &&
      left.type === right.type,
  );
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
  const theme = resolveGraphCanvasTheme();

  edges.forEach((edge, edgeIndex) => {
    const source = edge.source;
    const target = edge.target;
    const connected = Boolean(
      activeNode && (source.id === activeNode.id || target.id === activeNode.id),
    );
    const edgeIsHovered = sameEdge(edge, hoveredEdge);
    const edgeIsSelected = sameEdge(edge, selectedEdge);
    const dimmed = Boolean(activeNode && !connected && !edgeIsHovered && !edgeIsSelected);

    const sourceColor = getNodeColor(source.type).main;
    const targetColor = getNodeColor(target.type).main;

    const baseAlpha = dimmed
      ? 0.045
      : connected
        ? 0.78
        : theme.mode === 'dark'
          ? 0.22
          : 0.28;

    let strokeStyle: string | CanvasGradient = colorWithAlpha(sourceColor, baseAlpha);
    if (edgeIsHovered) {
      strokeStyle = theme.foregroundHex;
    } else if (edgeIsSelected) {
      strokeStyle = theme.infoHex;
    } else if (!dimmed && sourceColor !== targetColor) {
      const grad = context.createLinearGradient(source.x, source.y, target.x, target.y);
      grad.addColorStop(0, colorWithAlpha(sourceColor, connected ? 0.85 : baseAlpha));
      grad.addColorStop(1, colorWithAlpha(targetColor, connected ? 0.85 : baseAlpha));
      strokeStyle = grad;
    }

    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const midX = (source.x + target.x) * 0.5 + (curvedLinks ? dy * 0.1 : 0);
    const midY = (source.y + target.y) * 0.5 - (curvedLinks ? dx * 0.1 : 0);

    context.beginPath();
    if (curvedLinks) {
      context.moveTo(source.x, source.y);
      context.quadraticCurveTo(midX, midY, target.x, target.y);
    } else {
      context.moveTo(source.x, source.y);
      context.lineTo(target.x, target.y);
    }
    context.strokeStyle = strokeStyle;
    context.lineWidth =
      edgeIsHovered || edgeIsSelected ? 2.1 : connected ? 1.45 : dimmed ? 0.5 : 0.82;
    context.stroke();

    if (transform.k > 0.65 && (connected || edgeIsHovered || edgeIsSelected)) {
      drawArrowhead(
        context,
        source,
        target,
        edgeIsHovered ? theme.foregroundHex : targetColor,
        transform.k,
      );
    }

    // Subtle relation label on hovered/selected edges or zoomed-in active neighborhood
    if (edgeIsHovered || edgeIsSelected || (connected && transform.k >= 1.25)) {
      context.save();
      context.font = '500 8.5px ui-monospace, SFMono-Regular, Menlo, monospace';
      context.fillStyle = edgeIsHovered
        ? theme.foregroundHex
        : colorWithAlpha(theme.mutedForegroundHex, 0.9);
      context.textAlign = 'center';
      context.shadowColor = colorWithAlpha(theme.backgroundHex, 0.95);
      context.shadowBlur = 4;
      context.fillText(edge.type, midX, midY - 3);
      context.restore();
    }

    if (particlesEnabled && edgeIndex < MAX_PARTICLES && (connected || (!activeNode && edgeIndex % 2 === 0))) {
      const pulse = (now / 2800 + colorHash(`${source.id}:${target.id}`) / 1000) % 1;
      let px = source.x + dx * pulse;
      let py = source.y + dy * pulse;
      if (curvedLinks) {
        const inv = 1 - pulse;
        px = inv * inv * source.x + 2 * inv * pulse * midX + pulse * pulse * target.x;
        py = inv * inv * source.y + 2 * inv * pulse * midY + pulse * pulse * target.y;
      }
      context.beginPath();
      context.arc(px, py, connected ? 1.7 : 1.0, 0, Math.PI * 2);
      context.fillStyle = connected ? targetColor : colorWithAlpha(sourceColor, 0.55);
      context.globalAlpha = connected ? 0.92 : 0.35;
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
  checkedNodeNames?: ReadonlySet<string>,
) {
  const maxDegree = Math.max(...nodes.map((node) => node.degree), 0);
  const labelBudget = Math.min(84, Math.max(22, Math.floor(canvasWidth / 16)));
  const labelThreshold =
    nodes.length > 90
      ? Math.max(3, Math.ceil(maxDegree * 0.28))
      : Math.max(2, Math.ceil(maxDegree * 0.16));
  const query = activeSearch.trim().toLowerCase();
  const matchesSearch = (node: RenderNode) =>
    Boolean(query) &&
    `${node.name} ${node.type} ${node.observations}`.toLowerCase().includes(query);

  const labelIds = new Set(
    nodes
      .filter(
        (node) =>
          hoveredNode?.id === node.id ||
          selectedNode?.id === node.id ||
          (activeNode && connectedIds.has(node.id)) ||
          checkedNodeNames?.has(node.id) ||
          matchesSearch(node),
      )
      .map((node) => node.id),
  );

  const maximumLabels = Math.max(labelBudget, labelIds.size);
  const labelCandidates = [...nodes].sort((left, right) => {
    const leftHub = left.degree >= labelThreshold ? 1 : 0;
    const rightHub = right.degree >= labelThreshold ? 1 : 0;
    return (
      rightHub - leftHub ||
      right.degree - left.degree ||
      right.importance - left.importance ||
      left.name.localeCompare(right.name)
    );
  });

  for (const node of labelCandidates) {
    if (labelIds.has(node.id)) continue;
    const isHub = node.degree > 0 && node.degree >= labelThreshold;
    if (nodes.length > labelBudget && !isHub && transform.k <= 0.95) continue;
    if (labelIds.size >= maximumLabels) break;
    labelIds.add(node.id);
  }

  const theme = resolveGraphCanvasTheme();

  nodes.forEach((node) => {
    const isConnected = activeNode ? connectedIds.has(node.id) : true;
    const isHovered = hoveredNode?.id === node.id;
    const isSelected = selectedNode?.id === node.id;
    const isChecked = Boolean(checkedNodeNames?.has(node.id));
    const isSearchMatch = matchesSearch(node);
    const canonicalCat = normalizeGraphCategory(node.type);
    const palette = getNodeColor(canonicalCat);

    const baseRadius = Math.min(14.5, Math.max(3.6, 3.4 + Math.sqrt(node.degree) * 1.85));
    const renderRadius = Math.max(2.2 / Math.max(transform.k, 0.05), baseRadius);
    const alpha = activeNode && !isConnected ? 0.12 : 0.86 + node.importance * 0.14;
    const isGodNode = node.degree >= 6 || node.importance >= 0.55 || canonicalCat === 'core';
    const stronglyEmphasized = isSelected || isChecked || isSearchMatch;

    context.save();
    context.globalAlpha = alpha;

    // 1. Celestial Corona Halo
    if (showGlow && (stronglyEmphasized || isHovered || isGodNode || (activeNode && isConnected))) {
      const glowMult = stronglyEmphasized ? 4.4 : isHovered ? 3.6 : isGodNode ? 2.7 : 2.1;
      const glowRadius = renderRadius * glowMult;
      const glow = context.createRadialGradient(
        node.x,
        node.y,
        renderRadius * 0.2,
        node.x,
        node.y,
        glowRadius,
      );
      glow.addColorStop(
        0,
        colorWithAlpha(palette.main, stronglyEmphasized || isHovered ? 0.58 : 0.34),
      );
      glow.addColorStop(0.45, colorWithAlpha(palette.main, 0.12));
      glow.addColorStop(1, colorWithAlpha(palette.main, 0));
      context.fillStyle = glow;
      context.beginPath();
      context.arc(node.x, node.y, glowRadius, 0, Math.PI * 2);
      context.fill();
    }

    // 2. Selection / Search Orbit Ring
    if (stronglyEmphasized || isHovered) {
      context.beginPath();
      context.arc(node.x, node.y, renderRadius + 4.5, 0, Math.PI * 2);
      context.strokeStyle = isSelected || isHovered ? palette.main : theme.infoHex;
      context.lineWidth = isSelected ? 1.5 : 1.1;
      context.setLineDash(isSearchMatch && !isSelected ? [3, 3] : []);
      context.stroke();
      context.setLineDash([]);
    }

    // 3. Crisp Luminous Star Sphere
    const coreRadius = renderRadius + (isSelected || isHovered ? 1.2 : 0);
    context.beginPath();
    context.arc(node.x, node.y, coreRadius, 0, Math.PI * 2);
    context.fillStyle = palette.main;
    context.fill();

    // Subtle crisp rim
    context.lineWidth = isHovered || isSelected ? 1.5 : 0.8;
    context.strokeStyle =
      isHovered || isSelected
        ? theme.foregroundHex
        : colorWithAlpha(theme.backgroundHex, 0.75);
    context.stroke();

    // 4. Bright Stellar Core Highlight on God Nodes / Hubs
    if (isGodNode || isSelected || isHovered) {
      context.beginPath();
      context.arc(node.x, node.y, Math.max(1.3, coreRadius * 0.36), 0, Math.PI * 2);
      context.fillStyle = theme.mode === 'dark' ? '#ffffff' : '#fffbeb';
      context.globalAlpha = alpha * (isSelected || isHovered ? 0.92 : 0.72);
      context.fill();
    }

    context.restore();

    // 5. Clean Obsidian-style Floating Label
    if (showLabels && labelIds.has(node.id)) {
      context.save();
      context.globalAlpha = activeNode && !isConnected ? 0.14 : isHovered || isSelected || isGodNode ? 0.98 : 0.82;
      const fontSize = isSelected || isHovered ? 11 : isGodNode ? 10.5 : 9.5;
      const fontWeight = isSelected || isHovered || isGodNode ? '600' : '500';
      context.font = `${fontWeight} ${fontSize}px Inter, system-ui, sans-serif`;
      context.fillStyle =
        isHovered || isSelected || isGodNode
          ? theme.foregroundHex
          : mixHex(theme.foregroundHex, theme.mutedForegroundHex, 0.38);
      context.shadowColor = colorWithAlpha(theme.backgroundHex, 0.96);
      context.shadowBlur = 5;

      const rawLabel = node.name.startsWith('domain:')
        ? node.name.slice(7)
        : node.name.startsWith('cron:')
          ? node.name.slice(5)
          : node.name.startsWith('channel:')
            ? node.name.slice(8)
            : node.name;
      const label = rawLabel.length > 34 ? `${rawLabel.slice(0, 31)}…` : rawLabel;

      context.textAlign = 'center';
      context.fillText(label, node.x, node.y + coreRadius + 12);
      context.textAlign = 'start';
      context.restore();
    }
  });
}
