import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  BrainCircuit,
  Focus,
  Layers3,
  Maximize2,
  MousePointer2,
  RotateCcw,
  Search,
  Sliders,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import type { CognitiveEdge, CognitiveFact, CognitiveNode } from '../types/openz';
import { cn } from '../lib/utils';
import {
  buildClusters,
  layoutNodes,
  selectVisibleGraph,
  stableHash,
  type GraphCluster,
  type GraphMode,
  type LayoutNode,
} from './graphLayout';
import {
  buildGraphSemantics,
  edgeKey,
  formatConfidence,
  formatProvenance,
  type EdgeProvenance,
} from './graphSemantics';

export interface ObsidianGraphProps {
  nodes: CognitiveNode[];
  edges: CognitiveEdge[];
  facts?: CognitiveFact[];
  mode?: GraphMode;
  searchQuery?: string;
  selectedNodeName?: string | null;
  onSelectNode?: (nodeName: string | null) => void;
  onModeChange?: (mode: GraphMode) => void;
  onVisibleStatsChange?: (stats: { loaded: number; visible: number; edges: number }) => void;
  className?: string;
  height?: number | string;
}

interface RenderNode extends LayoutNode {
  vx: number;
  vy: number;
  isPinned: boolean;
  importance: number;
  clusterId: string;
}

interface GraphEdge {
  source: RenderNode;
  target: RenderNode;
  type: string;
  confidence: number | null;
  provenance: EdgeProvenance;
  sourceContext: string | null;
}

interface SpaceStar {
  x: number;
  y: number;
  radius: number;
  alpha: number;
}

interface Viewport {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

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

function colorHash(value: string): number {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}

function getNodeColor(type: string) {
  const key = type.trim().toLowerCase() || 'unknown';
  return NODE_COLOR_PALETTE[colorHash(key) % NODE_COLOR_PALETTE.length];
}

const SPACE_STARS: SpaceStar[] = Array.from({ length: 96 }, (_, index) => {
  const hash = stableHash(`space-star:${index}`);
  const secondary = stableHash(`space-star:${index}:secondary`);
  return {
    x: (hash % 10000) / 10000,
    y: (secondary % 10000) / 10000,
    radius: 0.35 + (hash % 100) / 220,
    alpha: 0.16 + (secondary % 100) / 260,
  };
});

function distanceToSegment(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSquared = dx * dx + dy * dy;
  if (lengthSquared === 0) return Math.hypot(px - ax, py - ay);
  const projection = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lengthSquared));
  return Math.hypot(px - (ax + projection * dx), py - (ay + projection * dy));
}

function drawArrowhead(
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

function observationsPreview(value: string): string {
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) return parsed.map(String).join(' · ');
    return String(parsed);
  } catch {
    return value || 'No observations recorded.';
  }
}

export const ObsidianGraph: React.FC<ObsidianGraphProps> = ({
  nodes,
  edges,
  mode,
  searchQuery,
  selectedNodeName,
  onSelectNode,
  onModeChange,
  onVisibleStatsChange,
  className,
  height = 560,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const layoutRef = useRef<Map<string, RenderNode>>(new Map());
  const edgesRef = useRef<GraphEdge[]>([]);
  const edgeDetailsRef = useRef<Map<string, GraphEdge>>(new Map());
  const clustersRef = useRef<GraphCluster[]>([]);
  const visibleNodesRef = useRef<RenderNode[]>([]);
  const visibleEdgesRef = useRef<GraphEdge[]>([]);
  const transformRef = useRef({ x: 0, y: 0, k: 1 });
  const focusTargetRef = useRef<{ x: number; y: number; k: number } | null>(null);
  const selectedNodeRef = useRef<RenderNode | null>(null);
  const hoveredNodeRef = useRef<RenderNode | null>(null);
  const hoveredEdgeRef = useRef<GraphEdge | null>(null);
  const hoveredClusterRef = useRef<GraphCluster | null>(null);
  const draggedNodeRef = useRef<RenderNode | null>(null);
  const isDraggingRef = useRef(false);
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0 });
  const settleFramesRef = useRef(0);
  const animationFrameRef = useRef<number | null>(null);
  const needsRenderRef = useRef(true);
  const requestRenderRef = useRef<() => void>(() => undefined);
  const lastReportedStatsRef = useRef('');

  const [localMode, setLocalMode] = useState<GraphMode>('overview');
  const [localSearch, setLocalSearch] = useState('');
  const [hoveredNode, setHoveredNode] = useState<RenderNode | null>(null);
  const [hoveredEdge, setHoveredEdge] = useState<GraphEdge | null>(null);
  const [selectedNode, setSelectedNode] = useState<RenderNode | null>(null);
  const [visibleStatus, setVisibleStatus] = useState({ visible: 0, edges: 0 });
  const [showSettings, setShowSettings] = useState(false);
  const [display, setDisplay] = useState({
    showLabels: true,
    showGlow: true,
    showParticles: true,
    curvedLinks: false,
    showSpaceField: true,
    showGrid: true,
    showOrbits: true,
  });

  const activeMode = mode ?? localMode;
  const activeSearch = searchQuery ?? localSearch;
  const activeSelectionName = selectedNodeName !== undefined ? selectedNodeName : selectedNode?.id || null;

  const graphData = useMemo(() => {
    const nodeMap = new Map<string, CognitiveNode>();
    nodes.forEach((node) => {
      if (node.name.trim()) nodeMap.set(node.name, node);
    });
    const knownNames = new Set(nodeMap.keys());
    const edgeMap = new Map<string, CognitiveEdge>();
    edges.forEach((edge) => {
      if (!knownNames.has(edge.from_name) || !knownNames.has(edge.to_name)) return;
      const key = [edge.from_name, edge.to_name, edge.relation_type].join('\u0000');
      edgeMap.set(key, edge);
    });
    return { nodes: Array.from(nodeMap.values()), edges: Array.from(edgeMap.values()) };
  }, [edges, nodes]);

  const graphSemantics = useMemo(
    () => buildGraphSemantics(graphData.nodes, graphData.edges),
    [graphData],
  );

  const graphTypes = useMemo(
    () => Array.from(new Set(graphData.nodes.map((node) => node.entity_type.trim().toLowerCase() || 'unknown'))).sort(),
    [graphData.nodes],
  );

  const setGraphMode = useCallback((nextMode: GraphMode) => {
    setLocalMode(nextMode);
    onModeChange?.(nextMode);
    settleFramesRef.current = 8;
    requestRenderRef.current();
  }, [onModeChange]);

  const updateSelection = useCallback((node: RenderNode | null) => {
    selectedNodeRef.current = node;
    setSelectedNode(node);
    onSelectNode?.(node?.name || null);
    requestRenderRef.current();
  }, [onSelectNode]);

  useEffect(() => {
    const next = activeSelectionName ? layoutRef.current.get(activeSelectionName) || null : null;
    selectedNodeRef.current = next;
    setSelectedNode(next);
    requestRenderRef.current();
  }, [activeSelectionName]);

  const viewportFor = useCallback((): Viewport => {
    const canvas = canvasRef.current;
    const transform = transformRef.current;
    const width = canvas ? canvas.clientWidth : 800;
    const height = canvas ? canvas.clientHeight : 560;
    return {
      left: -transform.x / transform.k - 80,
      top: -transform.y / transform.k - 80,
      right: (width - transform.x) / transform.k + 80,
      bottom: (height - transform.y) / transform.k + 80,
    };
  }, []);

  const reportVisibleStats = useCallback(() => {
    if (!onVisibleStatsChange) return;
    const visible = selectVisibleGraph(
      Array.from(layoutRef.current.values()),
      graphData.edges,
      clustersRef.current,
      activeMode,
      transformRef.current.k,
      viewportFor(),
      activeSelectionName,
      activeSearch,
    );
    const signature = [
      activeMode,
      activeSearch,
      visible.visibleNodeCount,
      visible.visibleEdgeCount,
      visible.loadedNodeCount,
    ].join(':');
    if (signature === lastReportedStatsRef.current) return;
    lastReportedStatsRef.current = signature;
    setVisibleStatus({ visible: visible.visibleNodeCount, edges: visible.visibleEdgeCount });
    onVisibleStatsChange({
      loaded: visible.loadedNodeCount,
      visible: visible.visibleNodeCount,
      edges: visible.visibleEdgeCount,
    });
  }, [activeMode, activeSearch, activeSelectionName, graphData.edges, onVisibleStatsChange, viewportFor]);

  const fitToView = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const width = canvas.clientWidth || 800;
    const height = canvas.clientHeight || 560;
    const source = activeMode === 'overview'
      ? clustersRef.current.map((cluster) => ({ x: cluster.x, y: cluster.y, radius: cluster.radius }))
      : Array.from(layoutRef.current.values()).map((node) => ({ x: node.x, y: node.y, radius: node.radius }));
    if (source.length === 0) {
      transformRef.current = { x: 0, y: 0, k: 1 };
      requestRenderRef.current();
      return;
    }
    const minX = Math.min(...source.map((item) => item.x - item.radius));
    const maxX = Math.max(...source.map((item) => item.x + item.radius));
    const minY = Math.min(...source.map((item) => item.y - item.radius));
    const maxY = Math.max(...source.map((item) => item.y + item.radius));
    const graphWidth = Math.max(180, maxX - minX + 120);
    const graphHeight = Math.max(180, maxY - minY + 120);
    const scale = Math.min(1.55, Math.max(0.35, Math.min(width / graphWidth, height / graphHeight) * 0.9));
    transformRef.current = {
      x: width / 2 - ((minX + maxX) / 2) * scale,
      y: height / 2 - ((minY + maxY) / 2) * scale,
      k: scale,
    };
    reportVisibleStats();
    requestRenderRef.current();
  }, [activeMode, reportVisibleStats]);

  const focusNode = useCallback((node: RenderNode) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const scale = Math.min(2.2, Math.max(transformRef.current.k, 1.18));
    focusTargetRef.current = {
      x: (canvas.clientWidth || 800) / 2 - node.x * scale,
      y: (canvas.clientHeight || 560) / 2 - node.y * scale,
      k: scale,
    };
    requestRenderRef.current();
  }, []);

  const zoomAroundCenter = useCallback((factor: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const current = transformRef.current;
    const nextK = Math.min(5, Math.max(0.18, current.k * factor));
    const centerX = canvas.clientWidth / 2;
    const centerY = canvas.clientHeight / 2;
    transformRef.current = {
      x: centerX - (centerX - current.x) * (nextK / current.k),
      y: centerY - (centerY - current.y) * (nextK / current.k),
      k: nextK,
    };
    reportVisibleStats();
    requestRenderRef.current();
  }, [reportVisibleStats]);

  const resetLayout = useCallback(() => {
    layoutRef.current.forEach((node) => {
      node.isPinned = false;
      node.vx = 0;
      node.vy = 0;
    });
    settleFramesRef.current = 12;
    fitToView();
  }, [fitToView]);

  const getGraphCoords = useCallback((screenX: number, screenY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    const transform = transformRef.current;
    return {
      x: (screenX - rect.left - transform.x) / transform.k,
      y: (screenY - rect.top - transform.y) / transform.k,
    };
  }, []);

  const findNodeAt = useCallback((screenX: number, screenY: number): RenderNode | null => {
    const point = getGraphCoords(screenX, screenY);
    const padding = 10 / transformRef.current.k;
    for (let index = visibleNodesRef.current.length - 1; index >= 0; index -= 1) {
      const node = visibleNodesRef.current[index];
      const dx = node.x - point.x;
      const dy = node.y - point.y;
      const radius = node.radius + padding;
      if (dx * dx + dy * dy <= radius * radius) return node;
    }
    return null;
  }, [getGraphCoords]);

  const findEdgeAt = useCallback((screenX: number, screenY: number): GraphEdge | null => {
    const point = getGraphCoords(screenX, screenY);
    const threshold = Math.max(6, 13 / transformRef.current.k);
    let closest: GraphEdge | null = null;
    let closestDistance = threshold;
    visibleEdgesRef.current.forEach((edge) => {
      const distance = distanceToSegment(point.x, point.y, edge.source.x, edge.source.y, edge.target.x, edge.target.y);
      if (distance < closestDistance) {
        closest = edge;
        closestDistance = distance;
      }
    });
    return closest;
  }, [getGraphCoords]);

  const findClusterAt = useCallback((screenX: number, screenY: number): GraphCluster | null => {
    if (activeMode !== 'overview') return null;
    const point = getGraphCoords(screenX, screenY);
    let closest: GraphCluster | null = null;
    let closestDistance = Infinity;
    clustersRef.current.forEach((cluster) => {
      const dx = cluster.x - point.x;
      const dy = cluster.y - point.y;
      const distance = Math.sqrt(dx * dx + dy * dy);
      if (distance <= cluster.radius && distance < closestDistance) {
        closest = cluster;
        closestDistance = distance;
      }
    });
    return closest;
  }, [activeMode, getGraphCoords]);

  const focusCluster = useCallback((cluster: GraphCluster) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const scale = Math.min(2.25, Math.max(1.15, Math.min(canvas.clientWidth, canvas.clientHeight) / (cluster.radius * 3)));
    transformRef.current = {
      x: canvas.clientWidth / 2 - cluster.x * scale,
      y: canvas.clientHeight / 2 - cluster.y * scale,
      k: scale,
    };
    setGraphMode('all');
    reportVisibleStats();
    requestRenderRef.current();
  }, [reportVisibleStats, setGraphMode]);

  const handleMouseDown = (event: React.MouseEvent<HTMLCanvasElement>) => {
    const node = findNodeAt(event.clientX, event.clientY);
    if (node) {
      isDraggingRef.current = true;
      draggedNodeRef.current = node;
      node.isPinned = true;
      updateSelection(node);
      if (!event.shiftKey) focusNode(node);
      settleFramesRef.current = 8;
      return;
    }
    const cluster = findClusterAt(event.clientX, event.clientY);
    if (cluster) {
      focusCluster(cluster);
      return;
    }
    isPanningRef.current = true;
    panStartRef.current = {
      x: event.clientX - transformRef.current.x,
      y: event.clientY - transformRef.current.y,
    };
    if (event.button === 0 && !event.shiftKey) updateSelection(null);
  };

  const handleMouseMove = (event: React.MouseEvent<HTMLCanvasElement>) => {
    if (isDraggingRef.current && draggedNodeRef.current) {
      const point = getGraphCoords(event.clientX, event.clientY);
      draggedNodeRef.current.x = point.x;
      draggedNodeRef.current.y = point.y;
      draggedNodeRef.current.vx = 0;
      draggedNodeRef.current.vy = 0;
      settleFramesRef.current = 6;
      requestRenderRef.current();
      return;
    }
    if (isPanningRef.current) {
      transformRef.current.x = event.clientX - panStartRef.current.x;
      transformRef.current.y = event.clientY - panStartRef.current.y;
      reportVisibleStats();
      requestRenderRef.current();
      return;
    }
    const nextHovered = findNodeAt(event.clientX, event.clientY);
    const nextHoveredEdge = nextHovered ? null : findEdgeAt(event.clientX, event.clientY);
    setHoveredNode((previous) => (previous?.id === nextHovered?.id ? previous : nextHovered));
    setHoveredEdge((previous) => (
      previous?.source.id === nextHoveredEdge?.source.id && previous?.target.id === nextHoveredEdge?.target.id && previous?.type === nextHoveredEdge?.type
        ? previous
        : nextHoveredEdge
    ));
    hoveredNodeRef.current = nextHovered;
    hoveredEdgeRef.current = nextHoveredEdge;
    hoveredClusterRef.current = nextHovered || nextHoveredEdge ? null : findClusterAt(event.clientX, event.clientY);
    requestRenderRef.current();
  };

  const handleMouseUp = () => {
    isDraggingRef.current = false;
    draggedNodeRef.current = null;
    isPanningRef.current = false;
  };

  const handleWheel = (event: React.WheelEvent<HTMLCanvasElement>) => {
    event.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mouseX = event.clientX - rect.left;
    const mouseY = event.clientY - rect.top;
    const current = transformRef.current;
    const nextK = Math.min(5, Math.max(0.18, current.k * (event.deltaY < 0 ? 1.12 : 0.89)));
    transformRef.current = {
      x: mouseX - (mouseX - current.x) * (nextK / current.k),
      y: mouseY - (mouseY - current.y) * (nextK / current.k),
      k: nextK,
    };
    reportVisibleStats();
    requestRenderRef.current();
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return undefined;
    const context = canvas.getContext('2d');
    if (!context) return undefined;

    const resizeCanvas = () => {
      const rect = container.getBoundingClientRect();
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = Math.max(1, Math.floor(rect.width * dpr));
      canvas.height = Math.max(1, Math.floor(rect.height * dpr));
      canvas.style.width = rect.width + 'px';
      canvas.style.height = rect.height + 'px';
      needsRenderRef.current = true;
      requestRenderRef.current();
    };

    resizeCanvas();
    const width = canvas.clientWidth || 800;
    const height = canvas.clientHeight || 560;
    const baseLayout = layoutNodes(graphData.nodes, graphData.edges, width, height);
    const previous = layoutRef.current;
    const next = new Map<string, RenderNode>();
    baseLayout.forEach((node) => {
      const oldNode = previous.get(node.id);
      const metrics = graphSemantics.nodeMetrics.get(node.id);
      next.set(node.id, {
        ...node,
        x: oldNode?.x ?? node.x,
        y: oldNode?.y ?? node.y,
        vx: oldNode?.vx ?? 0,
        vy: oldNode?.vy ?? 0,
        color: getNodeColor(node.type).main,
        isPinned: oldNode?.isPinned ?? false,
        importance: metrics?.importance ?? 0,
        clusterId: metrics?.clusterId ?? '',
      });
    });
    layoutRef.current = next;
    clustersRef.current = buildClusters(graphData.nodes, graphData.edges, width, height);
    const nextEdges = graphData.edges.flatMap((edge) => {
      const source = next.get(edge.from_name);
      const target = next.get(edge.to_name);
      if (!source || !target) return [];
      const detail = graphSemantics.edgeMetrics.get(edgeKey(edge));
      return [{
        source,
        target,
        type: edge.relation_type,
        confidence: detail?.confidence ?? null,
        provenance: detail?.provenance ?? 'not_recorded' as EdgeProvenance,
        sourceContext: detail?.source ?? null,
      }];
    });
    edgesRef.current = nextEdges;
    edgeDetailsRef.current = new Map(nextEdges.map((edge) => [
      edgeKey({ from_name: edge.source.id, to_name: edge.target.id, relation_type: edge.type }),
      edge,
    ]));
    visibleEdgesRef.current = [];
    hoveredEdgeRef.current = null;
    setHoveredEdge(null);
    selectedNodeRef.current = activeSelectionName ? next.get(activeSelectionName) || null : null;
    setSelectedNode(selectedNodeRef.current);
    settleFramesRef.current = 12;
    lastReportedStatsRef.current = '';

    if (activeSearch.trim()) {
      const query = activeSearch.trim().toLowerCase();
      const match = Array.from(next.values()).find((node) =>
        (node.name + ' ' + node.type + ' ' + node.observations).toLowerCase().includes(query),
      );
      if (match) {
        const scale = Math.max(transformRef.current.k, 1.05);
        transformRef.current.x = width / 2 - match.x * scale;
        transformRef.current.y = height / 2 - match.y * scale;
        transformRef.current.k = scale;
      }
    } else if (previous.size === 0) {
      transformRef.current = { x: 0, y: 0, k: 1 };
    }

    const draw = () => {
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      const canvasWidth = canvas.clientWidth || 800;
      const canvasHeight = canvas.clientHeight || 560;
      const transform = transformRef.current;
      const layout = Array.from(layoutRef.current.values());
      const visible = selectVisibleGraph(
        layout,
        graphData.edges,
        clustersRef.current,
        activeMode,
        transform.k,
        viewportFor(),
        selectedNodeRef.current?.id,
        activeSearch,
      );
      visibleNodesRef.current = visible.nodes
        .map((node) => layoutRef.current.get(node.id))
        .filter((node): node is RenderNode => Boolean(node));
      visibleEdgesRef.current = visible.edges
        .slice(0, 1800)
        .map((edge) => edgeDetailsRef.current.get(edgeKey(edge)))
        .filter((edge): edge is GraphEdge => Boolean(edge));

      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      context.clearRect(0, 0, canvasWidth, canvasHeight);
      const gradient = context.createRadialGradient(canvasWidth * 0.48, canvasHeight * 0.42, 20, canvasWidth * 0.48, canvasHeight * 0.42, Math.max(canvasWidth, canvasHeight) * 0.9);
      gradient.addColorStop(0, '#111827');
      gradient.addColorStop(0.52, '#090d17');
      gradient.addColorStop(1, '#04060b');
      context.fillStyle = gradient;
      context.fillRect(0, 0, canvasWidth, canvasHeight);

      const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
      if (display.showGrid) {
        context.strokeStyle = 'rgba(71, 85, 105, 0.08)';
        context.lineWidth = 1;
        const gridStep = 56;
        for (let x = 0; x <= canvasWidth; x += gridStep) {
          context.beginPath();
          context.moveTo(x, 0);
          context.lineTo(x, canvasHeight);
          context.stroke();
        }
        for (let y = 0; y <= canvasHeight; y += gridStep) {
          context.beginPath();
          context.moveTo(0, y);
          context.lineTo(canvasWidth, y);
          context.stroke();
        }
      }
      if (display.showSpaceField && !reducedMotion) {
        SPACE_STARS.forEach((star) => {
          context.beginPath();
          context.arc(star.x * canvasWidth, star.y * canvasHeight, star.radius, 0, Math.PI * 2);
          context.fillStyle = `rgba(226, 232, 240, ${star.alpha})`;
          context.fill();
        });
      }

      context.save();
      context.translate(transform.x, transform.y);
      context.scale(transform.k, transform.k);

      if (activeMode === 'overview') {
        clustersRef.current.forEach((cluster) => {
          const isHovered = hoveredClusterRef.current?.id === cluster.id;
          const palette = getNodeColor(cluster.type);
          const left = -transform.x / transform.k - 100;
          const top = -transform.y / transform.k - 100;
          const right = (canvasWidth - transform.x) / transform.k + 100;
          const bottom = (canvasHeight - transform.y) / transform.k + 100;
          if (cluster.x + cluster.radius < left || cluster.x - cluster.radius > right || cluster.y + cluster.radius < top || cluster.y - cluster.radius > bottom) return;
          const halo = context.createRadialGradient(cluster.x, cluster.y, cluster.radius * 0.08, cluster.x, cluster.y, cluster.radius);
          halo.addColorStop(0, isHovered ? palette.glow : 'rgba(30, 41, 59, 0.54)');
          halo.addColorStop(0.5, isHovered ? 'rgba(30, 41, 59, 0.32)' : 'rgba(15, 23, 42, 0.3)');
          halo.addColorStop(1, 'rgba(2, 6, 23, 0)');
          context.beginPath();
          context.arc(cluster.x, cluster.y, cluster.radius, 0, Math.PI * 2);
          context.fillStyle = halo;
          context.fill();
          context.beginPath();
          context.arc(cluster.x, cluster.y, cluster.radius * 0.72, 0, Math.PI * 2);
          context.fillStyle = isHovered ? palette.glow.replace(/0\.4[25]/, '0.08') : 'rgba(15, 23, 42, 0.2)';
          context.fill();
          context.strokeStyle = isHovered ? palette.main : 'rgba(100, 116, 139, 0.24)';
          context.lineWidth = isHovered ? 1.4 : 0.8;
          context.setLineDash(isHovered ? [5, 4] : [2, 7]);
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

      const activeNode = hoveredNodeRef.current || selectedNodeRef.current;
      const connectedIds = new Set<string>();
      if (activeNode) {
        connectedIds.add(activeNode.id);
        edgesRef.current.forEach((edge) => {
          if (edge.source.id === activeNode.id) connectedIds.add(edge.target.id);
          if (edge.target.id === activeNode.id) connectedIds.add(edge.source.id);
        });
      }

      if (activeNode && display.showOrbits) {
        const neighbors = visibleNodesRef.current.filter((node) => node.id !== activeNode.id && connectedIds.has(node.id)).slice(0, 160);
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

      const activeEdge = hoveredEdgeRef.current;
      visibleEdgesRef.current.forEach((edge) => {
        const source = edge.source;
        const target = edge.target;
        const connected = Boolean(activeNode && (source.id === activeNode.id || target.id === activeNode.id));
        const edgeIsHovered = Boolean(activeEdge && edge.source.id === activeEdge.source.id && edge.target.id === activeEdge.target.id && edge.type === activeEdge.type);
        const dimmed = Boolean(activeNode && !connected && !edgeIsHovered);
        const confidenceAlpha = edge.confidence === null ? 0.22 : 0.25 + edge.confidence * 0.5;
        const edgeColor = edgeIsHovered ? '#f8fafc' : connected ? (activeNode?.color || '#f59e0b') : `rgba(148, 163, 184, ${dimmed ? 0.035 : confidenceAlpha})`;
        context.beginPath();
        if (display.curvedLinks) {
          const midX = (source.x + target.x) / 2 + (target.y - source.y) * 0.12;
          const midY = (source.y + target.y) / 2 - (target.x - source.x) * 0.12;
          context.moveTo(source.x, source.y);
          context.quadraticCurveTo(midX, midY, target.x, target.y);
        } else {
          context.moveTo(source.x, source.y);
          context.lineTo(target.x, target.y);
        }
        context.strokeStyle = edgeColor;
        context.lineWidth = edgeIsHovered ? 2.2 : connected ? 1.7 : 0.62;
        context.setLineDash(edge.provenance === 'not_recorded' ? [4, 5] : []);
        context.stroke();
        context.setLineDash([]);
        if (transform.k > 0.7 && (connected || edgeIsHovered)) {
          drawArrowhead(context, source, target, edgeColor, transform.k);
        }
        if (display.showParticles && !reducedMotion && (connected || (!activeNode && visibleEdgesRef.current.length < 900))) {
          const pulse = (performance.now() / 2200 + colorHash(source.id + ':' + target.id) / 1000) % 1;
          context.beginPath();
          context.arc(source.x + (target.x - source.x) * pulse, source.y + (target.y - source.y) * pulse, connected ? 1.8 : 1.05, 0, Math.PI * 2);
          context.fillStyle = connected ? (activeNode?.color || '#f59e0b') : 'rgba(226, 232, 240, 0.5)';
          context.globalAlpha = connected ? 0.9 : 0.35;
          context.fill();
          context.globalAlpha = 1;
        }
      });

      const maxDegree = Math.max(...visible.nodes.map((node) => node.degree), 0);
      const labelThreshold = visible.nodes.length > 80 ? Math.max(3, Math.ceil(maxDegree * 0.35)) : 1;
      visibleNodesRef.current.forEach((node) => {
        const connected = activeNode ? connectedIds.has(node.id) : true;
        const isHovered = hoveredNodeRef.current?.id === node.id;
        const isSelected = selectedNodeRef.current?.id === node.id;
        const query = activeSearch.trim().toLowerCase();
        const isSearchMatch = Boolean(query) && (node.name + ' ' + node.type + ' ' + node.observations).toLowerCase().includes(query);
        const palette = getNodeColor(node.type);
        const alpha = activeNode && !connected ? 0.12 : 0.72 + node.importance * 0.28;
        const isImportant = node.importance >= 0.62;
        context.save();
        context.globalAlpha = alpha;
        if (display.showGlow && (isHovered || isSelected || isSearchMatch || connected)) {
          const glowRadius = node.radius * (isHovered || isSelected ? 3.5 : 2.1);
          const glow = context.createRadialGradient(node.x, node.y, node.radius * 0.3, node.x, node.y, glowRadius);
          glow.addColorStop(0, palette.glow);
          glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
          context.fillStyle = glow;
          context.beginPath();
          context.arc(node.x, node.y, glowRadius, 0, Math.PI * 2);
          context.fill();
        }
        if (isSearchMatch) {
          context.beginPath();
          context.arc(node.x, node.y, node.radius + 5, 0, Math.PI * 2);
          context.strokeStyle = '#7dd3fc';
          context.lineWidth = 1.3;
          context.setLineDash([3, 3]);
          context.stroke();
          context.setLineDash([]);
        }
        context.beginPath();
        context.arc(node.x, node.y, node.radius * (0.88 + node.importance * 0.36) + (isSelected ? 1.5 : 0), 0, Math.PI * 2);
        context.fillStyle = palette.main;
        context.fill();
        context.lineWidth = isHovered || isSelected || isImportant ? 1.8 : 0.7;
        context.strokeStyle = isHovered || isSelected ? '#f8fafc' : isImportant ? palette.main : '#020617';
        context.stroke();
        if (isImportant && !isSelected) {
          context.beginPath();
          context.arc(node.x, node.y, node.radius + 3.5, 0, Math.PI * 2);
          context.strokeStyle = palette.glow;
          context.lineWidth = 0.8;
          context.globalAlpha = 0.55;
          context.stroke();
          context.globalAlpha = 1;
        }
        if (isSelected) {
          context.beginPath();
          context.arc(node.x, node.y, node.radius + 5, 0, Math.PI * 2);
          context.strokeStyle = palette.main;
          context.lineWidth = 1.2;
          context.stroke();
        }
        context.restore();

        const showLabel = display.showLabels && (
          isHovered || isSelected || isSearchMatch ||
          Boolean(activeNode && connected) ||
          isImportant ||
          (activeMode !== 'overview' && transform.k > 1.15 && node.degree >= labelThreshold) ||
          (activeMode === 'overview' && node.degree >= labelThreshold * 2)
        );
        if (showLabel) {
          context.save();
          context.globalAlpha = alpha;
          context.font = (isSelected ? '600 ' : '500 ') + (isSelected ? '11px' : '10px') + ' Inter, system-ui, sans-serif';
          context.fillStyle = isHovered || isSelected ? '#f8fafc' : '#cbd5e1';
          context.shadowColor = 'rgba(0, 0, 0, 0.95)';
          context.shadowBlur = 4;
          const label = node.name.length > 36 ? node.name.slice(0, 33) + '…' : node.name;
          context.fillText(label, node.x + node.radius + 5, node.y + 3.5);
          context.restore();
        }
      });
      context.restore();

      needsRenderRef.current = false;
      if (settleFramesRef.current > 0) {
        const clusterByNode = new Map<string, GraphCluster>();
        clustersRef.current.forEach((cluster) => cluster.nodeIds.forEach((id) => clusterByNode.set(id, cluster)));
        layoutRef.current.forEach((node) => {
          if (node.isPinned) return;
          const cluster = clusterByNode.get(node.id);
          if (cluster) {
            node.vx += (cluster.x - node.x) * 0.002;
            node.vy += (cluster.y - node.y) * 0.002;
          }
          node.x += node.vx;
          node.y += node.vy;
          node.vx *= 0.82;
          node.vy *= 0.82;
        });
        settleFramesRef.current -= 1;
        needsRenderRef.current = true;
      }
      const focusTarget = focusTargetRef.current;
      if (focusTarget) {
        const current = transformRef.current;
        const next = {
          x: current.x + (focusTarget.x - current.x) * 0.22,
          y: current.y + (focusTarget.y - current.y) * 0.22,
          k: current.k + (focusTarget.k - current.k) * 0.22,
        };
        transformRef.current = next;
        const settled = Math.abs(focusTarget.x - next.x) < 0.5 && Math.abs(focusTarget.y - next.y) < 0.5 && Math.abs(focusTarget.k - next.k) < 0.005;
        if (settled) {
          transformRef.current = focusTarget;
          focusTargetRef.current = null;
        } else {
          needsRenderRef.current = true;
        }
      }
      const animateParticles = display.showParticles && !reducedMotion && visible.edges.length < 900;
      if (animateParticles || settleFramesRef.current > 0 || needsRenderRef.current) {
        animationFrameRef.current = requestAnimationFrame(draw);
      } else {
        animationFrameRef.current = null;
      }
    };

    const requestRender = () => {
      needsRenderRef.current = true;
      if (animationFrameRef.current === null) animationFrameRef.current = requestAnimationFrame(draw);
    };
    requestRenderRef.current = requestRender;
    requestRender();
    reportVisibleStats();
    window.addEventListener('resize', resizeCanvas);
    return () => {
      window.removeEventListener('resize', resizeCanvas);
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
      requestRenderRef.current = () => undefined;
    };
  }, [activeMode, activeSearch, activeSelectionName, display, graphData, graphSemantics, reportVisibleStats, viewportFor]);

  const hasData = graphData.nodes.length > 0;
  const selectedInfo = selectedNode || hoveredNode;
  const modeLabel = activeMode === 'overview' ? 'Cluster overview' : activeMode === 'all' ? 'All loaded nodes' : 'Selected neighborhood';

  return (
    <div
      ref={containerRef}
      className={cn('relative w-full overflow-hidden rounded-2xl border border-border/80 bg-slate-950 shadow-2xl', className)}
      style={{ height }}
    >
      {!hasData && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-slate-950 px-6 text-center">
          <BrainCircuit className="mb-4 h-12 w-12 text-slate-700" />
          <div className="text-sm font-semibold text-slate-400">No persisted graph records</div>
          <p className="mt-2 max-w-sm text-xs leading-relaxed text-slate-600">
            OpenZ will show entities and relationships here as they are written to cognitive memory.
          </p>
        </div>
      )}

      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={() => {
          handleMouseUp();
          hoveredNodeRef.current = null;
          hoveredEdgeRef.current = null;
          hoveredClusterRef.current = null;
          setHoveredNode(null);
          setHoveredEdge(null);
          requestRenderRef.current();
        }}
        onWheel={handleWheel}
        className="block h-full w-full cursor-grab touch-none active:cursor-grabbing"
        aria-label={`Interactive cognitive memory graph. ${graphData.nodes.length} records loaded. ${modeLabel}.`}
      />
      <span className="sr-only" aria-live="polite">
        {graphData.nodes.length} records loaded; {visibleStatus.visible} visible; {visibleStatus.edges} visible relations; {modeLabel}.
      </span>

      {hasData && (
        <>
          <div className="absolute left-3 top-3 z-10 flex max-w-[calc(100%-1.5rem)] flex-wrap items-center gap-2">
            {searchQuery === undefined && (
              <div className="flex items-center rounded-xl border border-white/10 bg-slate-950/88 px-2.5 py-1.5 shadow-lg backdrop-blur-md focus-within:border-amber-500/50">
                <Search className="mr-1.5 h-3.5 w-3.5 shrink-0 text-slate-500" />
                <input
                  value={localSearch}
                  onChange={(event) => setLocalSearch(event.target.value)}
                  placeholder="Find an entity or relation"
                  className="w-40 bg-transparent text-xs text-slate-200 outline-none placeholder:text-slate-600 sm:w-52"
                />
                {localSearch && (
                  <button type="button" onClick={() => setLocalSearch('')} className="ml-1 rounded p-0.5 text-slate-500 hover:text-slate-200" aria-label="Clear graph search">
                    <X className="h-3 w-3" />
                  </button>
                )}
              </div>
            )}
            <div className="flex items-center gap-1 rounded-xl border border-white/10 bg-slate-950/88 p-1 shadow-lg backdrop-blur-md">
              {([
                ['overview', 'Overview', Layers3],
                ['all', 'All nodes', MousePointer2],
                ['neighborhood', 'Neighborhood', Focus],
              ] as const).map(([value, label, Icon]) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => setGraphMode(value)}
                  disabled={value === 'neighborhood' && !activeSelectionName}
                  aria-pressed={activeMode === value}
                  className={cn(
                    'flex min-h-8 items-center gap-1.5 rounded-lg px-2.5 text-[10px] font-semibold transition',
                    activeMode === value ? 'bg-amber-500/15 text-amber-300' : 'text-slate-500 hover:bg-white/5 hover:text-slate-200',
                    value === 'neighborhood' && !activeSelectionName && 'cursor-not-allowed opacity-40',
                  )}
                  title={value === 'neighborhood' && !activeSelectionName ? 'Select an entity first' : label}
                >
                  <Icon className="h-3 w-3" />
                  <span className="hidden sm:inline">{label}</span>
                </button>
              ))}
            </div>
            <button
              type="button"
              onClick={() => setShowSettings((visible) => !visible)}
              className={cn('flex h-9 w-9 items-center justify-center rounded-xl border border-white/10 bg-slate-950/88 text-slate-500 shadow-lg backdrop-blur-md transition hover:text-slate-200', showSettings && 'border-amber-500/40 bg-amber-500/10 text-amber-300')}
              aria-label="Graph display settings"
              aria-pressed={showSettings}
            >
              <Sliders className="h-3.5 w-3.5" />
            </button>
          </div>

          <div className="absolute right-3 top-3 z-10 flex items-start gap-2">
            <div className="hidden rounded-xl border border-white/10 bg-slate-950/88 px-3 py-2 text-[10px] shadow-lg backdrop-blur-md sm:block">
              <div className="flex items-center gap-2 font-semibold text-slate-300">
                <span className="h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.8)]" />
                {graphData.nodes.length} records loaded
              </div>
              <div className="mt-1 text-right font-mono text-[9px] text-slate-600">{modeLabel} · {graphData.edges.length} relations</div>
            </div>
            <div className="flex flex-col gap-1 rounded-xl border border-white/10 bg-slate-950/88 p-1 shadow-lg backdrop-blur-md">
              <button type="button" onClick={() => zoomAroundCenter(1.2)} className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200" title="Zoom in" aria-label="Zoom in"><ZoomIn className="h-3.5 w-3.5" /></button>
              <button type="button" onClick={() => zoomAroundCenter(0.83)} className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200" title="Zoom out" aria-label="Zoom out"><ZoomOut className="h-3.5 w-3.5" /></button>
              <button type="button" onClick={fitToView} className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200" title="Fit graph" aria-label="Fit graph"><Maximize2 className="h-3.5 w-3.5" /></button>
              <button type="button" onClick={resetLayout} className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-white/5 hover:text-slate-200" title="Reset layout" aria-label="Reset layout"><RotateCcw className="h-3.5 w-3.5" /></button>
            </div>
          </div>

          {showSettings && (
            <div className="absolute left-3 top-16 z-20 w-72 max-w-[calc(100%-1.5rem)] rounded-2xl border border-white/10 bg-slate-950/96 p-4 text-xs shadow-2xl backdrop-blur-md">
              <div className="mb-3 flex items-center justify-between border-b border-white/10 pb-2 font-semibold text-slate-200">
                <span>Graph display</span>
                <button type="button" onClick={() => setShowSettings(false)} className="rounded p-1 text-slate-500 hover:text-slate-200" aria-label="Close graph settings"><X className="h-3.5 w-3.5" /></button>
              </div>
              <div className="space-y-2 text-[11px] text-slate-400">
                {([
                  ['showLabels', 'Entity labels'],
                  ['showGlow', 'Node glow'],
                  ['showParticles', 'Relation pulses'],
                  ['curvedLinks', 'Curved relations'],
                  ['showSpaceField', 'Space field'],
                  ['showGrid', 'Constellation grid'],
                  ['showOrbits', 'Neighbor orbits'],
                ] as const).map(([key, label]) => (
                  <label key={key} className="flex min-h-9 cursor-pointer items-center justify-between rounded-lg px-2 hover:bg-white/5">
                    <span>{label}</span>
                    <input
                      type="checkbox"
                      checked={display[key]}
                      onChange={(event) => setDisplay((current) => ({ ...current, [key]: event.target.checked }))}
                      className="h-4 w-4 accent-amber-500"
                    />
                  </label>
                ))}
              </div>
              <div className="mt-3 border-t border-white/10 pt-3 text-[10px] leading-relaxed text-slate-600">
                Overview groups connected records. Choose All nodes or zoom in to inspect individual entities.
              </div>
            </div>
          )}

          {graphTypes.length > 0 && (
            <div className="absolute bottom-3 right-3 z-10 hidden max-h-32 max-w-64 overflow-y-auto rounded-xl border border-white/10 bg-slate-950/88 p-2.5 text-[10px] shadow-lg backdrop-blur-md sm:block">
              <div className="mb-1.5 border-b border-white/10 pb-1 font-semibold text-slate-300">Entity types</div>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1">
                {graphTypes.map((type) => (
                  <div key={type} className="flex min-w-0 items-center gap-1.5 text-slate-500">
                    <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: getNodeColor(type).main }} />
                    <span className="truncate">{type}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {selectedInfo && (
            <div className="absolute bottom-3 left-3 z-10 max-w-[calc(100%-1.5rem)] rounded-xl border border-white/10 bg-slate-950/94 p-3 shadow-2xl backdrop-blur-md sm:max-w-sm">
              <div className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: getNodeColor(selectedInfo.type).main }} />
                <span className="truncate text-xs font-semibold text-slate-100">{selectedInfo.name}</span>
                <span className="rounded bg-white/5 px-1.5 py-0.5 font-mono text-[9px] uppercase text-slate-500">{selectedInfo.type}</span>
                <button type="button" onClick={() => updateSelection(null)} className="ml-auto rounded p-1 text-slate-500 hover:text-slate-200" aria-label="Clear selected entity"><X className="h-3 w-3" /></button>
              </div>
              <div className="mt-2 line-clamp-2 text-[10px] leading-relaxed text-slate-500">{observationsPreview(selectedInfo.observations)}</div>
              <div className="mt-2 flex items-center gap-2 text-[9px] font-mono text-slate-600">
                <span>{selectedInfo.degree} relations</span>
                <button type="button" onClick={() => setGraphMode('neighborhood')} className="rounded bg-amber-500/10 px-2 py-1 font-semibold text-amber-300 hover:bg-amber-500/20">Focus neighborhood</button>
              </div>
            </div>
          )}

          {hoveredEdge && !selectedInfo && (
            <div className="pointer-events-none absolute bottom-3 left-3 z-10 max-w-[calc(100%-1.5rem)] rounded-xl border border-cyan-400/20 bg-slate-950/94 px-3 py-2 text-[10px] shadow-2xl backdrop-blur sm:max-w-sm">
              <div className="flex items-center gap-2 text-slate-100">
                <span className="h-2 w-2 shrink-0 rounded-full bg-cyan-300 shadow-[0_0_10px_rgba(103,232,249,0.8)]" />
                <span className="truncate font-semibold">{hoveredEdge.source.name} → {hoveredEdge.target.name}</span>
              </div>
              <div className="mt-1.5 flex flex-wrap items-center gap-1.5 font-mono text-[9px] text-slate-500">
                <span className="rounded bg-white/5 px-1.5 py-0.5 text-cyan-300">{hoveredEdge.type}</span>
                <span>Confidence: {formatConfidence(hoveredEdge.confidence)}</span>
                <span>Provenance: {formatProvenance(hoveredEdge.provenance)}</span>
              </div>
            </div>
          )}

          <div className="pointer-events-none absolute bottom-3 left-1/2 hidden -translate-x-1/2 rounded-full border border-white/10 bg-slate-950/70 px-3 py-1 text-[9px] text-slate-600 backdrop-blur sm:block">
            <span className="mr-1.5">Drag to pan</span>·<span className="mx-1.5">scroll to zoom</span>·<span>click a cluster to explore</span>
          </div>
        </>
      )}
    </div>
  );
};
