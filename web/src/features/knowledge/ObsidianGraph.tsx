import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BrainCircuit, X } from 'lucide-react';
import type { CognitiveEdge, CognitiveFact, CognitiveNode } from '../../types/openz';
import { cn } from '../../lib/utils';
import {
  buildClusters,
  layoutNodes,
  prioritizeGraphEdges,
  selectVisibleGraph,
  type GraphCluster,
  type GraphMode,
} from './graphLayout';
import {
  collectConnectedNodeIds,
  drawBackground,
  drawClusters,
  drawEdges,
  drawNodes,
  drawOrbits,
  getNodeColor,
  MAX_PARTICLE_EDGES,
  MAX_RENDERED_EDGES,
  prefersReducedMotion,
  type GraphDisplayState,
  type GraphEdge,
  type RenderNode,
} from './graph/graphCanvas';
import {
  fitTransform,
  focusClusterTransform,
  focusNodeTransform,
  graphPointFromScreen,
  viewportForCanvas,
  zoomTransformAtPoint,
  type GraphTransform,
} from './graph/useGraphViewport';
import { findClusterAtPoint, findEdgeAtPoint, findNodeAtPoint } from './graph/useGraphInteraction';
import {
  buildGraphSemantics,
  edgeKey,
  formatConfidence,
  formatProvenance,
  type EdgeProvenance,
} from './graphSemantics';
import { GraphControls } from './graph/GraphControls';
import { GraphLegend } from './graph/GraphLegend';
import { GraphNodeList } from './graph/GraphNodeList';
import { formatObservations } from '../../shared/lib/format';
import { useResolvedTheme } from '../../shared/lib/theme';
import { useThemeStore } from '../../store/useThemeStore';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';

export interface ObsidianGraphHandle {
  zoomIn: () => void;
  zoomOut: () => void;
  fitToView: () => void;
  resetLayout: () => void;
  focusNodeByName: (nodeName: string) => void;
}

export interface ObsidianGraphProps {
  nodes: CognitiveNode[];
  edges: CognitiveEdge[];
  facts?: CognitiveFact[];
  mode?: GraphMode;
  searchQuery?: string;
  selectedNodeName?: string | null;
  selectedEdgeKey?: string | null;
  checkedNodeNames?: ReadonlySet<string>;
  display?: GraphDisplayState;
  onDisplayChange?: (key: keyof GraphDisplayState, value: boolean) => void;
  onSelectNode?: (nodeName: string | null) => void;
  onSelectEdge?: (edgeKey: string | null) => void;
  onModeChange?: (mode: GraphMode) => void;
  onVisibleStatsChange?: (stats: {
    loaded: number;
    visible: number;
    edges: number;
    renderedNodes: number;
    renderedEdges: number;
  }) => void;
  controlsRef?: React.MutableRefObject<ObsidianGraphHandle | null>;
  fullBleed?: boolean;
  showBuiltInChrome?: boolean;
  className?: string;
  height?: number | string;
}

// Kept here so the data-only regression test verifies the component's real defaults.
// eslint-disable-next-line react-refresh/only-export-components
export const DEFAULT_GRAPH_DISPLAY = {
  showLabels: true,
  showGlow: true,
  showParticles: true,
  curvedLinks: false,
  showSpaceField: true,
  showGrid: false,
  showOrbits: false,
} as const;

// eslint-disable-next-line react-refresh/only-export-components
export function resolveSettleFrames(frames: number, reducedMotion: boolean): number {
  return reducedMotion ? 0 : frames;
}

// eslint-disable-next-line react-refresh/only-export-components
export function layoutNeedsSettling(previousIds: ReadonlySet<string>, nextIds: Iterable<string>): boolean {
  const next = Array.from(nextIds);
  return previousIds.size !== next.length || next.some((id) => !previousIds.has(id));
}

// eslint-disable-next-line react-refresh/only-export-components
export function findGraphFocusNodeId(
  nodes: Array<Pick<CognitiveNode, 'name' | 'entity_type' | 'observations'>>,
  edges: Array<Pick<CognitiveEdge, 'from_name' | 'to_name' | 'relation_type'>>,
  searchQuery: string,
): string | null {
  const query = searchQuery.trim().toLowerCase();
  if (!query) return null;
  const matchingNode = nodes.find((node) =>
    [node.name, node.entity_type, node.observations].join(' ').toLowerCase().includes(query),
  );
  if (matchingNode) return matchingNode.name;
  const matchingEdge = edges.find((edge) =>
    [edge.from_name, edge.to_name, edge.relation_type].join(' ').toLowerCase().includes(query),
  );
  return matchingEdge?.from_name ?? null;
}

export const ObsidianGraph: React.FC<ObsidianGraphProps> = ({
  nodes,
  edges,
  mode,
  searchQuery,
  selectedNodeName,
  selectedEdgeKey,
  checkedNodeNames,
  display: controlledDisplay,
  onDisplayChange: onControlledDisplayChange,
  onSelectNode,
  onSelectEdge,
  onModeChange,
  onVisibleStatsChange,
  controlsRef,
  fullBleed = false,
  showBuiltInChrome = true,
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
  const transformRef = useRef<GraphTransform>({ x: 0, y: 0, k: 1 });
  const focusTargetRef = useRef<GraphTransform | null>(null);
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
  const [localDisplay, setLocalDisplay] = useState<GraphDisplayState>({ ...DEFAULT_GRAPH_DISPLAY });
  const display = controlledDisplay ?? localDisplay;
  const handleDisplayChange = useCallback(
    (key: keyof GraphDisplayState, value: boolean) => {
      if (onControlledDisplayChange) {
        onControlledDisplayChange(key, value);
      } else {
        setLocalDisplay((current) => ({ ...current, [key]: value }));
      }
    },
    [onControlledDisplayChange],
  );
  const activeThemeId = useUiCustomizationStore((s) => s.activeThemeId);
  const themeMode = useThemeStore((s) => s.theme);
  const resolvedThemeMode = useResolvedTheme(themeMode);

  const activeMode = mode ?? localMode;
  const activeSearch = searchQuery ?? localSearch;
  const activeSelectionName = selectedNodeName !== undefined ? selectedNodeName : selectedNode?.id || null;

  const setSettleFrames = useCallback((frames: number) => {
    const reducedMotion = prefersReducedMotion();
    settleFramesRef.current = resolveSettleFrames(frames, reducedMotion);
    if (reducedMotion) focusTargetRef.current = null;
  }, []);

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
    setSettleFrames(8);
    requestRenderRef.current();
  }, [onModeChange, setSettleFrames]);

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

  const viewportFor = useCallback(() => viewportForCanvas(canvasRef.current, transformRef.current), []);

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
    const renderedEdges = prioritizeGraphEdges(
      visible.edges,
      visible.nodes,
      activeSelectionName,
      transformRef.current.k,
      MAX_RENDERED_EDGES,
    ).length;
    const signature = [
      activeMode,
      activeSearch,
      visible.visibleNodeCount,
      visible.visibleEdgeCount,
      visible.loadedNodeCount,
      graphData.edges.length,
      visible.nodes.length,
      renderedEdges,
    ].join(':');
    if (signature === lastReportedStatsRef.current) return;
    lastReportedStatsRef.current = signature;
    setVisibleStatus({ visible: visible.visibleNodeCount, edges: visible.visibleEdgeCount });
    onVisibleStatsChange({
      loaded: visible.loadedNodeCount,
      visible: visible.visibleNodeCount,
      edges: graphData.edges.length,
      renderedNodes: visible.nodes.length,
      renderedEdges,
    });
  }, [activeMode, activeSearch, activeSelectionName, graphData.edges, onVisibleStatsChange, viewportFor]);

  const fitToView = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const source = activeMode === 'overview'
      ? clustersRef.current.map((cluster) => ({ x: cluster.x, y: cluster.y, radius: cluster.radius }))
      : Array.from(layoutRef.current.values()).map((node) => ({ x: node.x, y: node.y, radius: node.radius }));
    const nextTransform = fitTransform(canvas, source);
    if (!nextTransform) {
      transformRef.current = { x: 0, y: 0, k: 1 };
      requestRenderRef.current();
      return;
    }
    transformRef.current = nextTransform;
    reportVisibleStats();
    requestRenderRef.current();
  }, [activeMode, reportVisibleStats]);

  const focusNode = useCallback((node: RenderNode) => {
    const canvas = canvasRef.current;
    const target = focusNodeTransform(canvas, node, transformRef.current.k);
    if (!target) return;
    if (prefersReducedMotion()) {
      settleFramesRef.current = 0;
      focusTargetRef.current = null;
      transformRef.current = target;
      reportVisibleStats();
    } else {
      focusTargetRef.current = target;
    }
    requestRenderRef.current();
  }, [reportVisibleStats]);

  useEffect(() => {
    if (selectedNodeName === undefined) return;
    const node = selectedNodeName ? layoutRef.current.get(selectedNodeName) : null;
    if (node) focusNode(node);
  }, [focusNode, selectedNodeName]);

  const zoomAroundCenter = useCallback((factor: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const centerX = canvas.clientWidth / 2;
    const centerY = canvas.clientHeight / 2;
    transformRef.current = zoomTransformAtPoint(transformRef.current, centerX, centerY, factor);
    reportVisibleStats();
    requestRenderRef.current();
  }, [reportVisibleStats]);

  const resetLayout = useCallback(() => {
    layoutRef.current.forEach((node) => {
      node.isPinned = false;
      node.vx = 0;
      node.vy = 0;
    });
    setSettleFrames(12);
    fitToView();
  }, [fitToView, setSettleFrames]);

  const focusNodeByName = useCallback(
    (nodeName: string) => {
      const targetNode = layoutRef.current.get(nodeName) || null;
      if (!targetNode) return;
      updateSelection(targetNode);
      focusNode(targetNode);
    },
    [focusNode, updateSelection],
  );

  useEffect(() => {
    if (!controlsRef) return undefined;
    controlsRef.current = {
      zoomIn: () => zoomAroundCenter(1.2),
      zoomOut: () => zoomAroundCenter(0.83),
      fitToView,
      resetLayout,
      focusNodeByName,
    };
    return () => {
      controlsRef.current = null;
    };
  }, [controlsRef, fitToView, focusNodeByName, resetLayout, zoomAroundCenter]);

  const getGraphCoords = useCallback((screenX: number, screenY: number) => {
    return graphPointFromScreen(canvasRef.current, transformRef.current, screenX, screenY);
  }, []);

  const findNodeAt = useCallback((screenX: number, screenY: number): RenderNode | null => {
    return findNodeAtPoint(
      visibleNodesRef.current,
      getGraphCoords(screenX, screenY),
      transformRef.current.k,
    );
  }, [getGraphCoords]);

  const findEdgeAt = useCallback((screenX: number, screenY: number): GraphEdge | null => {
    return findEdgeAtPoint(
      visibleEdgesRef.current,
      getGraphCoords(screenX, screenY),
      transformRef.current.k,
    );
  }, [getGraphCoords]);

  const findClusterAt = useCallback((screenX: number, screenY: number): GraphCluster | null => {
    if (activeMode !== 'overview') return null;
    return findClusterAtPoint(clustersRef.current, getGraphCoords(screenX, screenY));
  }, [activeMode, getGraphCoords]);

  const focusCluster = useCallback((cluster: GraphCluster) => {
    const canvas = canvasRef.current;
    const nextTransform = focusClusterTransform(canvas, cluster);
    if (!nextTransform) return;
    transformRef.current = nextTransform;
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
      setSettleFrames(8);
      return;
    }
    const cluster = findClusterAt(event.clientX, event.clientY);
    if (cluster) {
      focusCluster(cluster);
      return;
    }
    const edge = findEdgeAt(event.clientX, event.clientY);
    if (edge) {
      onSelectEdge?.(edgeKey({ from_name: edge.source.id, to_name: edge.target.id, relation_type: edge.type }));
      updateSelection(edge.source);
      focusNode(edge.source);
      return;
    }
    onSelectEdge?.(null);
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
      setSettleFrames(6);
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
    transformRef.current = zoomTransformAtPoint(
      transformRef.current,
      mouseX,
      mouseY,
      event.deltaY < 0 ? 1.12 : 0.89,
    );
    reportVisibleStats();
    requestRenderRef.current();
  };

  const handleCanvasKeyDown = (event: React.KeyboardEvent<HTMLCanvasElement>) => {
    const panStep = event.shiftKey ? 120 : 40;
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight' || event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      transformRef.current = {
        ...transformRef.current,
        x: transformRef.current.x + (event.key === 'ArrowLeft' ? panStep : event.key === 'ArrowRight' ? -panStep : 0),
        y: transformRef.current.y + (event.key === 'ArrowUp' ? panStep : event.key === 'ArrowDown' ? -panStep : 0),
      };
      reportVisibleStats();
      requestRenderRef.current();
      return;
    }
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      zoomAroundCenter(1.2);
      return;
    }
    if (event.key === '-' || event.key === '_') {
      event.preventDefault();
      zoomAroundCenter(0.83);
      return;
    }
    if (event.key === '0') {
      event.preventDefault();
      fitToView();
      return;
    }
    if (event.key === 'Escape') {
      onSelectEdge?.(null);
      updateSelection(null);
    }
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
    if (layoutNeedsSettling(new Set(previous.keys()), next.keys())) {
      setSettleFrames(12);
    } else {
      settleFramesRef.current = 0;
      focusTargetRef.current = null;
    }
    lastReportedStatsRef.current = '';

    if (activeSearch.trim()) {
      const match = findGraphFocusNodeId(graphData.nodes, graphData.edges, activeSearch);
      const node = match ? next.get(match) : null;
      if (node) {
        const scale = Math.max(transformRef.current.k, 1.05);
        transformRef.current.x = width / 2 - node.x * scale;
        transformRef.current.y = height / 2 - node.y * scale;
        transformRef.current.k = scale;
      }
    } else if (previous.size === 0 && next.size > 0) {
      const initialBounds = Array.from(next.values()).map((n) => ({
        x: n.x,
        y: n.y,
        radius: n.radius + 18,
      }));
      const fitted = fitTransform(canvas, initialBounds);
      transformRef.current = fitted ?? { x: 0, y: 0, k: 1 };
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
      visibleEdgesRef.current = prioritizeGraphEdges(
        visible.edges,
        visible.nodes,
        selectedNodeRef.current?.id || null,
        transform.k,
        MAX_RENDERED_EDGES,
      )
        .map((edge) => edgeDetailsRef.current.get(edgeKey(edge)))
        .filter((edge): edge is GraphEdge => Boolean(edge));

      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawBackground(
        context,
        canvasWidth,
        canvasHeight,
        display.showGrid,
        display.showSpaceField,
        transform,
      );

      const reducedMotion = prefersReducedMotion();
      if (reducedMotion) {
        settleFramesRef.current = 0;
        const focusTarget = focusTargetRef.current;
        if (focusTarget) transformRef.current = focusTarget;
        focusTargetRef.current = null;
      }
      const particlesEnabled = display.showParticles && !reducedMotion && visible.edges.length <= MAX_PARTICLE_EDGES;

      context.save();
      context.translate(transform.x, transform.y);
      context.scale(transform.k, transform.k);

      if (activeMode === 'overview' || display.showGlow) {
        drawClusters(context, clustersRef.current, hoveredClusterRef.current, transform, canvasWidth, canvasHeight);
      }

      const activeNode = hoveredNodeRef.current || selectedNodeRef.current;
      const connectedIds = collectConnectedNodeIds(activeNode, edgesRef.current);
      if (checkedNodeNames && checkedNodeNames.size > 0) {
        checkedNodeNames.forEach((checkedId) => connectedIds.add(checkedId));
      }
      drawOrbits(context, activeNode, visibleNodesRef.current, connectedIds, display.showOrbits);

      const selectedEdge = (selectedEdgeKey ? edgeDetailsRef.current.get(selectedEdgeKey) : null) ?? null;
      drawEdges(
        context,
        visibleEdgesRef.current,
        activeNode,
        hoveredEdgeRef.current,
        selectedEdge,
        transform,
        display.curvedLinks,
        particlesEnabled,
        performance.now(),
      );
      drawNodes(
        context,
        visibleNodesRef.current,
        activeNode,
        hoveredNodeRef.current,
        selectedNodeRef.current,
        connectedIds,
        activeSearch,
        transform,
        canvasWidth,
        display.showGlow,
        display.showLabels,
        checkedNodeNames,
      );
      context.restore();

      needsRenderRef.current = false;
      if (settleFramesRef.current > 0) {
        const nodesArr = Array.from(layoutRef.current.values());
        if (isDraggingRef.current && nodesArr.length <= 360) {
          // Interactive Obsidian spring physics while dragging a star
          for (const edge of edgesRef.current) {
            const dx = edge.target.x - edge.source.x;
            const dy = edge.target.y - edge.source.y;
            const dist = Math.hypot(dx, dy) || 1;
            const ideal = 72 + Math.sqrt(edge.source.degree + edge.target.degree) * 10;
            const pull = (dist - ideal) * 0.018;
            const ux = (dx / dist) * pull;
            const uy = (dy / dist) * pull;
            if (!edge.source.isPinned) {
              edge.source.vx += ux;
              edge.source.vy += uy;
            }
            if (!edge.target.isPinned) {
              edge.target.vx -= ux;
              edge.target.vy -= uy;
            }
          }
          for (let i = 0; i < nodesArr.length; i += 1) {
            const a = nodesArr[i];
            for (let j = i + 1; j < nodesArr.length; j += 1) {
              const b = nodesArr[j];
              const dx = b.x - a.x;
              const dy = b.y - a.y;
              const dSq = dx * dx + dy * dy + 1;
              if (dSq > 16_000) continue;
              const d = Math.sqrt(dSq);
              const rep = 160 / dSq;
              const rx = (dx / d) * rep;
              const ry = (dy / d) * rep;
              if (!a.isPinned) {
                a.vx -= rx;
                a.vy -= ry;
              }
              if (!b.isPinned) {
                b.vx += rx;
                b.vy += ry;
              }
            }
          }
        }
        nodesArr.forEach((node) => {
          if (node.isPinned) return;
          node.x += Math.max(-12, Math.min(12, node.vx));
          node.y += Math.max(-12, Math.min(12, node.vy));
          node.vx *= 0.78;
          node.vy *= 0.78;
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
      if (particlesEnabled || settleFramesRef.current > 0 || needsRenderRef.current) {
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
  }, [activeMode, activeSearch, activeSelectionName, activeThemeId, checkedNodeNames, display, focusNode, graphData, graphSemantics, onSelectEdge, reportVisibleStats, resolvedThemeMode, selectedEdgeKey, setSettleFrames, viewportFor]);

  const hasData = graphData.nodes.length > 0;
  const selectedInfo = selectedNode || hoveredNode;
  const modeLabel = activeMode === 'overview' ? 'Cluster overview' : activeMode === 'all' ? 'All loaded nodes' : 'Selected neighborhood';

  return (
    <div
      ref={containerRef}
      className={cn(
        'relative w-full overflow-hidden bg-background',
        fullBleed ? 'h-full rounded-none border-0 shadow-none' : 'rounded-2xl border border-border/80 shadow-xl',
        className,
      )}
      style={{ height }}
    >
      {!hasData && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-background px-6 text-center">
          <BrainCircuit className="mb-4 h-12 w-12 text-muted-foreground/45" />
          <div className="text-sm font-semibold text-foreground/85">No persisted graph records</div>
          <p className="mt-2 max-w-sm text-xs leading-relaxed text-muted-foreground">
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
        onKeyDown={handleCanvasKeyDown}
        tabIndex={0}
        className={cn('block h-full w-full touch-none active:cursor-grabbing', hoveredNode || hoveredEdge ? 'cursor-pointer' : 'cursor-grab')}
        aria-label={`Interactive cognitive memory graph. ${graphData.nodes.length} records loaded. ${modeLabel}. Use arrow keys to pan, plus or minus to zoom, zero to fit, and Escape to clear selection.`}
      />
      <span className="sr-only" aria-live="polite">
        {graphData.nodes.length} records loaded; {visibleStatus.visible} visible; {visibleStatus.edges} visible relations; {modeLabel}.
      </span>

      {hasData && showBuiltInChrome && (
        <>
          <GraphControls
            activeMode={activeMode}
            activeSelectionName={activeSelectionName}
            searchQuery={searchQuery}
            localSearch={localSearch}
            onLocalSearchChange={setLocalSearch}
            onModeChange={setGraphMode}
            showSettings={showSettings}
            onToggleSettings={() => setShowSettings((visible) => !visible)}
            onCloseSettings={() => setShowSettings(false)}
            display={display}
            onDisplayChange={handleDisplayChange}
            onZoom={zoomAroundCenter}
            onFit={fitToView}
            onReset={resetLayout}
            loadedNodes={graphData.nodes.length}
            loadedEdges={graphData.edges.length}
            modeLabel={modeLabel}
          />

          <GraphLegend types={graphTypes} />
          <GraphNodeList
            nodes={Array.from(layoutRef.current.values())}
            selectedNodeId={selectedNodeRef.current?.id || null}
            onSelectNode={(node) => {
              updateSelection(node);
              focusNode(node);
            }}
          />

          {selectedInfo && (
            <div className="absolute bottom-3 left-3 z-10 max-w-[calc(100%-1.5rem)] rounded-xl border border-border/75 bg-card/94 p-3 shadow-xl backdrop-blur-md sm:max-w-sm">
              <div className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: getNodeColor(selectedInfo.type).main }} />
                <span className="truncate text-xs font-semibold text-foreground">{selectedInfo.name}</span>
                <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-[9px] uppercase text-muted-foreground">{selectedInfo.type}</span>
                <button type="button" onClick={() => updateSelection(null)} className="ml-auto rounded p-1 text-muted-foreground hover:text-foreground" aria-label="Clear selected entity"><X className="h-3 w-3" /></button>
              </div>
              <div className="mt-2 line-clamp-2 text-[10px] leading-relaxed text-muted-foreground">{formatObservations(selectedInfo.observations)}</div>
              <div className="mt-2 flex items-center gap-2 text-[9px] font-mono text-muted-foreground">
                <span>{selectedInfo.degree} relations</span>
                <button type="button" onClick={() => setGraphMode('neighborhood')} className="rounded bg-primary/12 px-2 py-1 font-semibold text-primary hover:bg-primary/20">Focus neighborhood</button>
              </div>
            </div>
          )}

          {hoveredEdge && !selectedInfo && (
            <div className="pointer-events-none absolute bottom-3 left-3 z-10 max-w-[calc(100%-1.5rem)] rounded-xl border border-info/30 bg-card/94 px-3 py-2 text-[10px] shadow-xl backdrop-blur sm:max-w-sm">
              <div className="flex items-center gap-2 text-foreground">
                <span className="h-2 w-2 shrink-0 rounded-full bg-info" />
                <span className="truncate font-semibold">{hoveredEdge.source.name} → {hoveredEdge.target.name}</span>
              </div>
              <div className="mt-1.5 flex flex-wrap items-center gap-1.5 font-mono text-[9px] text-muted-foreground">
                <span className="rounded bg-info/12 px-1.5 py-0.5 text-info">{hoveredEdge.type}</span>
                <span>Confidence: {formatConfidence(hoveredEdge.confidence)}</span>
                <span>Provenance: {formatProvenance(hoveredEdge.provenance)}</span>
              </div>
            </div>
          )}

          <div className="pointer-events-none absolute bottom-3 left-1/2 hidden -translate-x-1/2 rounded-full border border-border/75 bg-card/80 px-3 py-1 text-[9px] text-muted-foreground backdrop-blur sm:block">
            <span className="mr-1.5">Drag to pan</span>·<span className="mx-1.5">scroll to zoom</span>·<span>click a cluster to explore</span>
          </div>
        </>
      )}
    </div>
  );
};
