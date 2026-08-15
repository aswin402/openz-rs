import React, { useEffect, useRef, useState, useMemo, useCallback } from 'react';
import {
  ZoomIn,
  ZoomOut,
  Maximize2,
  RotateCcw,
  Sliders,
  Search,
  X,
  BrainCircuit,
  Zap,
  CheckCircle2,
} from 'lucide-react';
import type { CognitiveNode, CognitiveEdge, CognitiveFact } from '../types/openz';
import { cn } from '../lib/utils';

export interface ObsidianGraphProps {
  nodes: CognitiveNode[];
  edges: CognitiveEdge[];
  facts?: CognitiveFact[];
  onSelectNode?: (nodeName: string | null) => void;
  className?: string;
  height?: number | string;
}

interface GraphNode {
  id: string;
  name: string;
  type: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  glowColor: string;
  observations: string;
  degree: number;
  isPinned: boolean;
  alpha: number;
  spawnRipple?: number; // 1.0 -> 0.0 expanding entrance ring
}

interface Particle {
  t: number;
  speed: number;
  size: number;
}

interface GraphEdge {
  source: GraphNode;
  target: GraphNode;
  type: string;
  particles: Particle[];
}

const TYPE_COLORS: Record<string, { main: string; glow: string }> = {
  agent: { main: '#f59e0b', glow: 'rgba(245, 158, 11, 0.45)' },
  user: { main: '#3b82f6', glow: 'rgba(59, 130, 246, 0.45)' },
  workspace: { main: '#10b981', glow: 'rgba(16, 185, 129, 0.45)' },
  file: { main: '#ec4899', glow: 'rgba(236, 72, 153, 0.45)' },
  code: { main: '#f43f5e', glow: 'rgba(244, 63, 94, 0.45)' },
  concept: { main: '#a855f7', glow: 'rgba(168, 85, 247, 0.45)' },
  fact: { main: '#06b6d4', glow: 'rgba(6, 182, 212, 0.45)' },
  skill: { main: '#f97316', glow: 'rgba(249, 115, 22, 0.45)' },
  link: { main: '#38bdf8', glow: 'rgba(56, 189, 248, 0.45)' },
  device: { main: '#14b8a6', glow: 'rgba(20, 184, 166, 0.45)' },
  session: { main: '#eab308', glow: 'rgba(234, 179, 8, 0.45)' },
  default: { main: '#94a3b8', glow: 'rgba(148, 163, 184, 0.35)' },
};

function getNodeColor(type: string) {
  const key = type.toLowerCase();
  return TYPE_COLORS[key] || TYPE_COLORS.default;
}

export const ObsidianGraph: React.FC<ObsidianGraphProps> = ({
  nodes,
  edges,
  facts = [],
  onSelectNode,
  className,
  height = 520,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Graph state refs (for 60fps canvas loop — never trigger React re-renders)
  const nodesMapRef = useRef<Map<string, GraphNode>>(new Map());
  const edgesRef = useRef<GraphEdge[]>([]);
  const transformRef = useRef<{ x: number; y: number; k: number }>({ x: 0, y: 0, k: 1 });
  const hoveredNodeRef = useRef<GraphNode | null>(null);
  const selectedNodeRef = useRef<GraphNode | null>(null);
  const isDraggingNodeRef = useRef<boolean>(false);
  const draggedNodeRef = useRef<GraphNode | null>(null);
  const isPanningRef = useRef<boolean>(false);
  const panStartRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const animFrameIdRef = useRef<number | null>(null);
  const simAlphaRef = useRef<number>(1.0);
  const hasFittedRef = useRef<boolean>(false);
  const hoveredEdgeRef = useRef<GraphEdge | null>(null);
  const progressiveStreamTimerRef = useRef<number | null>(null);

  // UI State
  const [hoveredNode, setHoveredNode] = useState<GraphNode | null>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [streamProgress, setStreamProgress] = useState<{ current: number; total: number; isStreaming: boolean }>({
    current: 0,
    total: 0,
    isStreaming: false,
  });

  // Graph force & visual settings
  const [settings, setSettings] = useState({
    repulsion: 180,
    linkDistance: 65,
    centerGravity: 0.05,
    collisionRadius: 10,
    showLabels: true,
    showGlow: true,
    showParticles: true,
    curvedLinks: false,
    fontSize: 10,
    progressiveStream: true,
  });

  // Build unified graph data from REAL props only — zero hardcoded nodes
  const graphData = useMemo(() => {
    const rawNodesMap = new Map<string, CognitiveNode>();
    nodes.forEach((n) => rawNodesMap.set(n.name, n));

    const rawEdges: CognitiveEdge[] = [...edges];

    // Synthesize fact nodes and tag relationships from actual cognitive data
    facts.forEach((f) => {
      if (!f.text || !f.tags) return;

      const factNodeName = `Fact: ${f.text.slice(0, 40)}${f.text.length > 40 ? '…' : ''}`;
      if (!rawNodesMap.has(factNodeName)) {
        rawNodesMap.set(factNodeName, {
          name: factNodeName,
          entity_type: 'fact',
          observations: JSON.stringify([f.text, `Tags: ${f.tags}`, `Importance: ${f.importance}`]),
        });
      }

      // Link facts to tag nodes
      f.tags
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
        .forEach((tag) => {
          const tagNodeName = `#${tag}`;
          if (!rawNodesMap.has(tagNodeName)) {
            rawNodesMap.set(tagNodeName, {
              name: tagNodeName,
              entity_type: 'concept',
              observations: JSON.stringify([`Memory tag: ${tag}`]),
            });
          }
          rawEdges.push({
            from_name: factNodeName,
            to_name: tagNodeName,
            relation_type: 'tagged_with',
          });
        });
    });

    return {
      nodes: Array.from(rawNodesMap.values()),
      edges: rawEdges,
    };
  }, [nodes, edges, facts]);

  // Check if graph has real data
  const hasData = graphData.nodes.length > 0;

  // Function to re-sync all edges for current nodes in nodesMapRef
  const refreshEdges = useCallback(() => {
    const currentNodes = nodesMapRef.current;
    const nextEdges: GraphEdge[] = [];

    graphData.edges.forEach((e) => {
      const source = currentNodes.get(e.from_name);
      const target = currentNodes.get(e.to_name);
      if (source && target) {
        const particles: Particle[] = [
          { t: Math.random(), speed: 0.003 + Math.random() * 0.004, size: 1.5 },
          { t: Math.random(), speed: 0.002 + Math.random() * 0.003, size: 1.2 },
        ];
        nextEdges.push({ source, target, type: e.relation_type, particles });
      }
    });

    edgesRef.current = nextEdges;
  }, [graphData.edges]);

  // Function to instantly load all remaining queued nodes
  const handleLoadAllInstantly = useCallback(() => {
    if (progressiveStreamTimerRef.current) {
      clearInterval(progressiveStreamTimerRef.current);
      progressiveStreamTimerRef.current = null;
    }

    const degreeMap = new Map<string, number>();
    graphData.edges.forEach((e) => {
      degreeMap.set(e.from_name, (degreeMap.get(e.from_name) || 0) + 1);
      degreeMap.set(e.to_name, (degreeMap.get(e.to_name) || 0) + 1);
    });

    const canvas = canvasRef.current;
    const width = canvas ? canvas.width / (window.devicePixelRatio || 1) : 800;
    const height = canvas ? canvas.height / (window.devicePixelRatio || 1) : 520;
    const cx = width / 2;
    const cy = height / 2;

    const currentMap = nodesMapRef.current;

    graphData.nodes.forEach((n, idx) => {
      if (currentMap.has(n.name)) return;

      const degree = degreeMap.get(n.name) || 0;
      const typeColors = getNodeColor(n.entity_type);
      const baseRadius = n.entity_type.toLowerCase() === 'agent' ? 8 : 4;
      const radius = Math.min(15, Math.max(3.5, baseRadius + Math.sqrt(degree) * 2.2));

      const angle = (idx / Math.max(1, graphData.nodes.length)) * Math.PI * 2 + (Math.random() * 0.4 - 0.2);
      const distance = 40 + Math.random() * 120 + (n.entity_type.toLowerCase() === 'agent' ? 0 : 30);

      currentMap.set(n.name, {
        id: n.name,
        name: n.name,
        type: n.entity_type,
        x: cx + Math.cos(angle) * distance,
        y: cy + Math.sin(angle) * distance,
        vx: (Math.random() - 0.5) * 2,
        vy: (Math.random() - 0.5) * 2,
        radius,
        color: typeColors.main,
        glowColor: typeColors.glow,
        observations: n.observations,
        degree,
        isPinned: false,
        alpha: 1.0,
      });
    });

    refreshEdges();
    simAlphaRef.current = Math.max(simAlphaRef.current, 0.6);
    setStreamProgress({
      current: graphData.nodes.length,
      total: graphData.nodes.length,
      isStreaming: false,
    });
  }, [graphData, refreshEdges]);

  // Progressive One-by-One stream ingestion
  useEffect(() => {
    if (!hasData) {
      if (progressiveStreamTimerRef.current) {
        clearInterval(progressiveStreamTimerRef.current);
        progressiveStreamTimerRef.current = null;
      }
      nodesMapRef.current = new Map();
      edgesRef.current = [];
      setStreamProgress({ current: 0, total: 0, isStreaming: false });
      return;
    }

    // Calculate node degree (number of connections)
    const degreeMap = new Map<string, number>();
    graphData.edges.forEach((e) => {
      degreeMap.set(e.from_name, (degreeMap.get(e.from_name) || 0) + 1);
      degreeMap.set(e.to_name, (degreeMap.get(e.to_name) || 0) + 1);
    });

    // Sort incoming nodes: Hubs (highest degree) first, then leaf nodes
    const sortedIncoming = [...graphData.nodes].sort((a, b) => {
      const degA = degreeMap.get(a.name) || 0;
      const degB = degreeMap.get(b.name) || 0;
      return degB - degA;
    });

    const canvas = canvasRef.current;
    const width = canvas ? canvas.width / (window.devicePixelRatio || 1) : 800;
    const height = canvas ? canvas.height / (window.devicePixelRatio || 1) : 520;
    const cx = width / 2;
    const cy = height / 2;

    const currentMap = nodesMapRef.current;

    // Update existing nodes in place
    sortedIncoming.forEach((n) => {
      const existing = currentMap.get(n.name);
      if (existing) {
        const degree = degreeMap.get(n.name) || 0;
        const typeColors = getNodeColor(n.entity_type);
        const baseRadius = n.entity_type.toLowerCase() === 'agent' ? 8 : 4;
        existing.type = n.entity_type;
        existing.observations = n.observations;
        existing.color = typeColors.main;
        existing.glowColor = typeColors.glow;
        existing.degree = degree;
        existing.radius = Math.min(15, Math.max(3.5, baseRadius + Math.sqrt(degree) * 2.2));
      }
    });

    // Find pending nodes to stream in one by one
    const pendingNodes = sortedIncoming.filter((n) => !currentMap.has(n.name));

    if (pendingNodes.length === 0) {
      refreshEdges();
      setStreamProgress({
        current: currentMap.size,
        total: sortedIncoming.length,
        isStreaming: false,
      });
      return;
    }

    if (!settings.progressiveStream) {
      // Instant loading mode
      handleLoadAllInstantly();
      return;
    }

    // Stream nodes one by one
    setStreamProgress({
      current: currentMap.size,
      total: sortedIncoming.length,
      isStreaming: true,
    });

    if (progressiveStreamTimerRef.current) {
      clearInterval(progressiveStreamTimerRef.current);
    }

    let queueIdx = 0;
    progressiveStreamTimerRef.current = window.setInterval(() => {
      if (queueIdx >= pendingNodes.length) {
        if (progressiveStreamTimerRef.current) {
          clearInterval(progressiveStreamTimerRef.current);
          progressiveStreamTimerRef.current = null;
        }
        setStreamProgress({
          current: sortedIncoming.length,
          total: sortedIncoming.length,
          isStreaming: false,
        });

        // Trigger smooth fit on initial stream finish
        if (!hasFittedRef.current) {
          hasFittedRef.current = true;
          setTimeout(() => handleFitToView(), 300);
        }
        return;
      }

      const n = pendingNodes[queueIdx];
      const degree = degreeMap.get(n.name) || 0;
      const typeColors = getNodeColor(n.entity_type);
      const baseRadius = n.entity_type.toLowerCase() === 'agent' ? 8 : 4;
      const radius = Math.min(15, Math.max(3.5, baseRadius + Math.sqrt(degree) * 2.2));

      // Organic radial spawn position
      const globalIdx = currentMap.size;
      const angle = (globalIdx / Math.max(1, sortedIncoming.length)) * Math.PI * 2 + (Math.random() * 0.4 - 0.2);
      const distance = 35 + Math.random() * 110 + (n.entity_type.toLowerCase() === 'agent' ? 0 : 25);

      currentMap.set(n.name, {
        id: n.name,
        name: n.name,
        type: n.entity_type,
        x: cx + Math.cos(angle) * distance,
        y: cy + Math.sin(angle) * distance,
        vx: (Math.random() - 0.5) * 1.5,
        vy: (Math.random() - 0.5) * 1.5,
        radius,
        color: typeColors.main,
        glowColor: typeColors.glow,
        observations: n.observations,
        degree,
        isPinned: false,
        alpha: 0.1,
        spawnRipple: 1.0, // Spawn effect
      });

      refreshEdges();
      simAlphaRef.current = Math.max(simAlphaRef.current, 0.45); // Gentle pulse

      queueIdx++;
      setStreamProgress({
        current: currentMap.size,
        total: sortedIncoming.length,
        isStreaming: true,
      });
    }, 28); // 1 node every 28ms for smooth organic unfolding

    return () => {
      if (progressiveStreamTimerRef.current) {
        clearInterval(progressiveStreamTimerRef.current);
        progressiveStreamTimerRef.current = null;
      }
    };
  }, [graphData, hasData, refreshEdges, handleLoadAllInstantly, settings.progressiveStream]);

  // Center / Fit Graph View
  const handleFitToView = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const width = canvas.width / (window.devicePixelRatio || 1);
    const height = canvas.height / (window.devicePixelRatio || 1);
    const nodesList = Array.from(nodesMapRef.current.values());

    if (nodesList.length === 0) {
      transformRef.current = { x: 0, y: 0, k: 1 };
      return;
    }

    let minX = Infinity,
      maxX = -Infinity,
      minY = Infinity,
      maxY = -Infinity;
    nodesList.forEach((n) => {
      minX = Math.min(minX, n.x);
      maxX = Math.max(maxX, n.x);
      minY = Math.min(minY, n.y);
      maxY = Math.max(maxY, n.y);
    });

    const graphWidth = Math.max(100, maxX - minX + 80);
    const graphHeight = Math.max(100, maxY - minY + 80);
    const scaleX = width / graphWidth;
    const scaleY = height / graphHeight;
    const k = Math.min(1.5, Math.max(0.4, Math.min(scaleX, scaleY) * 0.85));

    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;

    transformRef.current = {
      x: width / 2 - centerX * k,
      y: height / 2 - centerY * k,
      k,
    };
    simAlphaRef.current = 0.3;
  }, []);

  const handleZoom = (factor: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const width = canvas.width / (window.devicePixelRatio || 1);
    const height = canvas.height / (window.devicePixelRatio || 1);
    const cx = width / 2;
    const cy = height / 2;

    const current = transformRef.current;
    const newK = Math.min(4.5, Math.max(0.15, current.k * factor));
    const newX = cx - (cx - current.x) * (newK / current.k);
    const newY = cy - (cy - current.y) * (newK / current.k);

    transformRef.current = { x: newX, y: newY, k: newK };
    simAlphaRef.current = 0.2;
  };

  const handleReheat = () => {
    const nodesList = Array.from(nodesMapRef.current.values());
    nodesList.forEach((n) => {
      if (!n.isPinned) {
        n.vx += (Math.random() - 0.5) * 6;
        n.vy += (Math.random() - 0.5) * 6;
      }
    });
    simAlphaRef.current = 1.0;
  };

  // Main Canvas Rendering & Physics Engine
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let currentDpr = 1;

    const updateCanvasSize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = container.getBoundingClientRect();
      currentDpr = dpr;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
    };

    updateCanvasSize();
    window.addEventListener('resize', updateCanvasSize);

    // Run Force-Directed Simulation and Render Frame
    const renderLoop = () => {
      const dpr = currentDpr;
      const width = canvas.width / dpr;
      const height = canvas.height / dpr;

      const graphNodes = Array.from(nodesMapRef.current.values());
      const graphEdges = edgesRef.current;
      const transform = transformRef.current;

      // ─── 1. Force Simulation Step ───
      const alpha = simAlphaRef.current;
      if (alpha > 0.001) {
        const cx = width / 2;
        const cy = height / 2;
        const repulsionForce = settings.repulsion;
        const linkDist = settings.linkDistance;
        const gravity = settings.centerGravity;
        const collisionPad = settings.collisionRadius;

        // Gravity: Pull all unpinned nodes toward canvas center
        graphNodes.forEach((node) => {
          if (node === draggedNodeRef.current || node.isPinned) return;
          const dx = cx - node.x;
          const dy = cy - node.y;
          node.vx += dx * gravity * 0.03 * alpha;
          node.vy += dy * gravity * 0.03 * alpha;
        });

        // Many-body Repulsion with distance cutoff for performance
        const cutoffDist = 350;
        for (let i = 0; i < graphNodes.length; i++) {
          const a = graphNodes[i];
          for (let j = i + 1; j < graphNodes.length; j++) {
            const b = graphNodes[j];
            const dx = b.x - a.x;
            const dy = b.y - a.y;

            // Fast reject: skip pairs beyond cutoff
            if (Math.abs(dx) > cutoffDist || Math.abs(dy) > cutoffDist) continue;

            const distSq = dx * dx + dy * dy + 80;
            const dist = Math.sqrt(distSq);

            if (dist < cutoffDist) {
              const force = (repulsionForce / distSq) * alpha * 1.5;
              const fx = (dx / dist) * force;
              const fy = (dy / dist) * force;

              if (a !== draggedNodeRef.current && !a.isPinned) {
                a.vx -= fx;
                a.vy -= fy;
              }
              if (b !== draggedNodeRef.current && !b.isPinned) {
                b.vx += fx;
                b.vy += fy;
              }
            }

            // Elastic Collision detection
            const minAllowedDist = a.radius + b.radius + collisionPad;
            if (dist < minAllowedDist && dist > 0.1) {
              const overlap = (minAllowedDist - dist) * 0.5 * alpha;
              const ox = (dx / dist) * overlap;
              const oy = (dy / dist) * overlap;
              if (a !== draggedNodeRef.current && !a.isPinned) {
                a.vx -= ox;
                a.vy -= oy;
              }
              if (b !== draggedNodeRef.current && !b.isPinned) {
                b.vx += ox;
                b.vy += oy;
              }
            }
          }
        }

        // Link Spring Attraction (Hooke's Law)
        graphEdges.forEach((edge) => {
          const dx = edge.target.x - edge.source.x;
          const dy = edge.target.y - edge.source.y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 0.1;
          const delta = (dist - linkDist) * 0.05 * alpha;
          const fx = (dx / dist) * delta;
          const fy = (dy / dist) * delta;

          if (edge.source !== draggedNodeRef.current && !edge.source.isPinned) {
            edge.source.vx += fx;
            edge.source.vy += fy;
          }
          if (edge.target !== draggedNodeRef.current && !edge.target.isPinned) {
            edge.target.vx -= fx;
            edge.target.vy -= fy;
          }
        });

        // Integrate Velocities with Damping & Entrance Fade
        const damping = 0.88;
        graphNodes.forEach((node) => {
          if (node.alpha < 1) node.alpha = Math.min(1, node.alpha + 0.07);
          if (node.spawnRipple && node.spawnRipple > 0) {
            node.spawnRipple = Math.max(0, node.spawnRipple - 0.04);
          }

          if (node === draggedNodeRef.current || node.isPinned) return;
          node.x += node.vx;
          node.y += node.vy;
          node.vx *= damping;
          node.vy *= damping;
        });

        // Decay simulation heat
        simAlphaRef.current *= 0.988;
      }

      // Update Edge Pulse Particles
      graphEdges.forEach((edge) => {
        edge.particles.forEach((p) => {
          p.t += p.speed;
          if (p.t > 1) p.t = 0;
        });
      });

      // ─── 2. Draw Frame ───
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);

      // Deep obsidian background
      const bgGradient = ctx.createRadialGradient(
        width / 2,
        height / 2,
        40,
        width / 2,
        height / 2,
        Math.max(width, height) * 0.8
      );
      bgGradient.addColorStop(0, '#0c0d12');
      bgGradient.addColorStop(1, '#050508');
      ctx.fillStyle = bgGradient;
      ctx.fillRect(0, 0, width, height);

      // Empty state: nothing to render, skip graph drawing
      if (graphNodes.length === 0) {
        animFrameIdRef.current = requestAnimationFrame(renderLoop);
        return;
      }

      // Apply Graph Pan/Zoom Transform
      ctx.save();
      ctx.translate(transform.x, transform.y);
      ctx.scale(transform.k, transform.k);

      const hovered = hoveredNodeRef.current;
      const selected = selectedNodeRef.current;
      const activeNode = hovered || selected;

      // Identify connected neighbors for spotlight
      const connectedNeighbors = new Set<string>();
      if (activeNode) {
        connectedNeighbors.add(activeNode.id);
        graphEdges.forEach((edge) => {
          if (edge.source.id === activeNode.id) connectedNeighbors.add(edge.target.id);
          if (edge.target.id === activeNode.id) connectedNeighbors.add(edge.source.id);
        });
      }

      const normalizedSearch = searchQuery.trim().toLowerCase();

      // Viewport culling bounds (in graph coordinates)
      const vpLeft = -transform.x / transform.k - 60;
      const vpTop = -transform.y / transform.k - 60;
      const vpRight = (width - transform.x) / transform.k + 60;
      const vpBottom = (height - transform.y) / transform.k + 60;

      const isInViewport = (x: number, y: number) =>
        x >= vpLeft && x <= vpRight && y >= vpTop && y <= vpBottom;

      // ─── Draw Links ───
      graphEdges.forEach((edge) => {
        if (!isInViewport(edge.source.x, edge.source.y) && !isInViewport(edge.target.x, edge.target.y)) return;

        const isConnected = activeNode
          ? edge.source.id === activeNode.id || edge.target.id === activeNode.id
          : false;

        const strokeAlpha = activeNode ? (isConnected ? 0.65 : 0.04) : 0.22;
        const lineWidth = activeNode ? (isConnected ? 1.4 : 0.3) : 0.6;

        ctx.beginPath();
        if (settings.curvedLinks) {
          const midX =
            (edge.source.x + edge.target.x) / 2 + (edge.target.y - edge.source.y) * 0.15;
          const midY =
            (edge.source.y + edge.target.y) / 2 - (edge.target.x - edge.source.x) * 0.15;
          ctx.moveTo(edge.source.x, edge.source.y);
          ctx.quadraticCurveTo(midX, midY, edge.target.x, edge.target.y);
        } else {
          ctx.moveTo(edge.source.x, edge.source.y);
          ctx.lineTo(edge.target.x, edge.target.y);
        }

        if (isConnected && activeNode) {
          ctx.strokeStyle = activeNode.color;
        } else {
          ctx.strokeStyle = `rgba(148, 163, 184, ${strokeAlpha})`;
        }
        ctx.lineWidth = lineWidth;
        ctx.stroke();

        // Show relation type label on hovered edge
        if (edge === hoveredEdgeRef.current && isConnected && activeNode) {
          const midX = (edge.source.x + edge.target.x) / 2;
          const midY = (edge.source.y + edge.target.y) / 2;
          ctx.save();
          ctx.font = '9px Inter, system-ui, sans-serif';
          ctx.fillStyle = 'rgba(255, 255, 255, 0.75)';
          ctx.shadowColor = 'rgba(0, 0, 0, 0.9)';
          ctx.shadowBlur = 3;
          ctx.fillText(edge.type, midX + 4, midY - 4);
          ctx.restore();
        }

        // Flowing energy particles
        if (settings.showParticles && (!activeNode || isConnected)) {
          edge.particles.forEach((p) => {
            const px = edge.source.x + (edge.target.x - edge.source.x) * p.t;
            const py = edge.source.y + (edge.target.y - edge.source.y) * p.t;

            ctx.beginPath();
            ctx.arc(px, py, p.size, 0, Math.PI * 2);
            ctx.fillStyle = isConnected && activeNode ? activeNode.color : '#e2e8f0';
            ctx.globalAlpha = isConnected ? 0.9 : 0.35;
            ctx.fill();
            ctx.globalAlpha = 1.0;
          });
        }
      });

      // ─── Draw Nodes ───
      graphNodes.forEach((node) => {
        if (!isInViewport(node.x, node.y)) return;

        const isConnected = activeNode ? connectedNeighbors.has(node.id) : true;
        const isHovered = node === hovered;
        const isSelected = node === selected;
        const isSearchMatch = normalizedSearch
          ? node.name.toLowerCase().includes(normalizedSearch)
          : false;

        let nodeAlpha = activeNode ? (isConnected ? 1.0 : 0.12) : 1.0;
        nodeAlpha *= node.alpha;

        ctx.save();
        ctx.globalAlpha = nodeAlpha;

        // Spawn pulse wave effect on newly streamed nodes
        if (node.spawnRipple && node.spawnRipple > 0) {
          const rippleRadius = node.radius + (1.0 - node.spawnRipple) * 18;
          ctx.beginPath();
          ctx.arc(node.x, node.y, rippleRadius, 0, Math.PI * 2);
          ctx.strokeStyle = node.color;
          ctx.lineWidth = 1.2 * node.spawnRipple;
          ctx.globalAlpha = node.spawnRipple * 0.7;
          ctx.stroke();
          ctx.globalAlpha = nodeAlpha;
        }

        // Outer Radial Glow (Obsidian node halo)
        if (
          settings.showGlow &&
          (isHovered || isSelected || isConnected || isSearchMatch)
        ) {
          const glowRadius = node.radius * (isHovered || isSelected ? 3.5 : 2.2);
          const glowGrad = ctx.createRadialGradient(
            node.x,
            node.y,
            node.radius * 0.5,
            node.x,
            node.y,
            glowRadius
          );
          glowGrad.addColorStop(0, node.glowColor);
          glowGrad.addColorStop(1, 'rgba(0, 0, 0, 0)');

          ctx.fillStyle = glowGrad;
          ctx.beginPath();
          ctx.arc(node.x, node.y, glowRadius, 0, Math.PI * 2);
          ctx.fill();
        }

        // Search Match Halo Ring
        if (isSearchMatch) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius + 5, 0, Math.PI * 2);
          ctx.strokeStyle = '#38bdf8';
          ctx.lineWidth = 1.5;
          ctx.setLineDash([3, 3]);
          ctx.stroke();
          ctx.setLineDash([]);
        }

        // Node Body
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
        ctx.fillStyle = node.color;
        ctx.fill();

        // Node Border
        ctx.lineWidth = isHovered || isSelected ? 1.8 : 0.8;
        ctx.strokeStyle = isHovered || isSelected ? '#ffffff' : '#09090b';
        ctx.stroke();

        // Selection Target Ring
        if (isSelected) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius + 3.5, 0, Math.PI * 2);
          ctx.strokeStyle = node.color;
          ctx.lineWidth = 1.2;
          ctx.stroke();
        }

        ctx.restore();

        // ─── Draw Labels ───
        const shouldShowLabel =
          settings.showLabels &&
          (isHovered ||
            isSelected ||
            isSearchMatch ||
            (activeNode && isConnected) ||
            transform.k > 1.1 ||
            node.degree >= 2 ||
            node.type.toLowerCase() === 'agent');

        if (shouldShowLabel) {
          ctx.save();
          ctx.globalAlpha = nodeAlpha;
          ctx.font = `${settings.fontSize}px 'Inter', system-ui, sans-serif`;

          const text = node.name.length > 40 ? node.name.slice(0, 37) + '…' : node.name;
          const textX = node.x + node.radius + 5;
          const textY = node.y + 3.5;

          ctx.shadowColor = 'rgba(0, 0, 0, 0.9)';
          ctx.shadowBlur = 4;
          ctx.fillStyle = isHovered || isSelected ? '#ffffff' : '#e2e8f0';
          ctx.fillText(text, textX, textY);
          ctx.restore();
        }
      });

      ctx.restore();

      animFrameIdRef.current = requestAnimationFrame(renderLoop);
    };

    renderLoop();

    return () => {
      if (animFrameIdRef.current) cancelAnimationFrame(animFrameIdRef.current);
      window.removeEventListener('resize', updateCanvasSize);
    };
  }, [settings, searchQuery]);

  // Coordinate transforms
  const getGraphCoords = useCallback(
    (screenX: number, screenY: number) => {
      const canvas = canvasRef.current;
      if (!canvas) return { x: 0, y: 0 };
      const rect = canvas.getBoundingClientRect();
      const clientX = screenX - rect.left;
      const clientY = screenY - rect.top;
      const transform = transformRef.current;

      return {
        x: (clientX - transform.x) / transform.k,
        y: (clientY - transform.y) / transform.k,
      };
    },
    []
  );

  // Find node under cursor
  const getNodeAtScreenPos = useCallback(
    (screenX: number, screenY: number): GraphNode | null => {
      const coords = getGraphCoords(screenX, screenY);
      const nodesList = Array.from(nodesMapRef.current.values());

      for (let i = nodesList.length - 1; i >= 0; i--) {
        const node = nodesList[i];
        const dx = node.x - coords.x;
        const dy = node.y - coords.y;
        const hitRadius = (node.radius + 8) / Math.min(1, transformRef.current.k);
        if (dx * dx + dy * dy <= hitRadius * hitRadius) {
          return node;
        }
      }
      return null;
    },
    [getGraphCoords]
  );

  // Mouse / Touch Event Handlers
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const targetNode = getNodeAtScreenPos(e.clientX, e.clientY);

    if (targetNode) {
      isDraggingNodeRef.current = true;
      draggedNodeRef.current = targetNode;
      setSelectedNode(targetNode);
      selectedNodeRef.current = targetNode;
      onSelectNode?.(targetNode.name);
      simAlphaRef.current = 0.6;
    } else {
      isPanningRef.current = true;
      panStartRef.current = {
        x: e.clientX - transformRef.current.x,
        y: e.clientY - transformRef.current.y,
      };
      if (e.button === 0 && !e.shiftKey) {
        setSelectedNode(null);
        selectedNodeRef.current = null;
        onSelectNode?.(null);
      }
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (isDraggingNodeRef.current && draggedNodeRef.current) {
      const coords = getGraphCoords(e.clientX, e.clientY);
      draggedNodeRef.current.x = coords.x;
      draggedNodeRef.current.y = coords.y;
      draggedNodeRef.current.vx = 0;
      draggedNodeRef.current.vy = 0;
      simAlphaRef.current = Math.max(simAlphaRef.current, 0.4);
      return;
    }

    if (isPanningRef.current) {
      transformRef.current = {
        ...transformRef.current,
        x: e.clientX - panStartRef.current.x,
        y: e.clientY - panStartRef.current.y,
      };
      return;
    }

    const hoverNode = getNodeAtScreenPos(e.clientX, e.clientY);
    setHoveredNode(hoverNode);
    hoveredNodeRef.current = hoverNode;

    if (hoverNode) {
      const connected = edgesRef.current.find(
        (edge) => edge.source.id === hoverNode.id || edge.target.id === hoverNode.id
      );
      hoveredEdgeRef.current = connected || null;
    } else {
      hoveredEdgeRef.current = null;
    }
  };

  const handleMouseUp = () => {
    isDraggingNodeRef.current = false;
    draggedNodeRef.current = null;
    isPanningRef.current = false;
  };

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const zoomFactor = e.deltaY < 0 ? 1.12 : 0.89;
    const current = transformRef.current;
    const newK = Math.min(4.5, Math.max(0.15, current.k * zoomFactor));

    const newX = mouseX - (mouseX - current.x) * (newK / current.k);
    const newY = mouseY - (mouseY - current.y) * (newK / current.k);

    transformRef.current = { x: newX, y: newY, k: newK };
    simAlphaRef.current = Math.max(simAlphaRef.current, 0.15);
  };

  const activeNodeInfo = selectedNode || hoveredNode;

  // Compute dynamic legend from actual data
  const legendTypes = useMemo(() => {
    const types = new Set<string>();
    graphData.nodes.forEach((n) => types.add(n.entity_type.toLowerCase()));
    return Array.from(types)
      .filter((t) => TYPE_COLORS[t])
      .sort();
  }, [graphData.nodes]);

  return (
    <div
      ref={containerRef}
      className={cn(
        'relative w-full rounded-2xl border border-border/80 bg-zinc-950/90 overflow-hidden shadow-2xl select-none',
        className
      )}
      style={{ height }}
    >
      {/* Empty State — shown when there is genuinely no data */}
      {!hasData && (
        <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-zinc-950/95">
          <BrainCircuit className="h-12 w-12 text-muted-foreground/30 mb-4" />
          <div className="text-sm font-semibold text-muted-foreground/60">No memory data yet</div>
          <p className="mt-2 max-w-xs text-center text-xs text-muted-foreground/40 leading-relaxed">
            Start chatting with OpenZ to build the knowledge graph. Entities, relations, skills,
            and facts will appear here in real time as the agent learns.
          </p>
        </div>
      )}

      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
        className="block w-full h-full cursor-grab active:cursor-grabbing"
      />

      {/* ─── Top Left: Search & Settings ─── */}
      {hasData && (
        <div className="absolute top-3 left-3 z-10 flex items-center gap-2">
          <div className="relative flex items-center rounded-xl border border-border/60 bg-zinc-950/85 backdrop-blur-md shadow-lg shadow-black/30 px-2.5 py-1.5 transition-all focus-within:border-amber-500/50">
            <Search className="h-3.5 w-3.5 text-muted-foreground mr-1.5 shrink-0" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search nodes…"
              className="w-36 sm:w-48 bg-transparent text-xs text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="ml-1 text-muted-foreground hover:text-foreground"
              >
                <X className="h-3 w-3" />
              </button>
            )}
          </div>

          <button
            onClick={() => setShowSettings(!showSettings)}
            className={cn(
              'flex h-8 w-8 items-center justify-center rounded-xl border border-border/60 bg-zinc-950/85 text-muted-foreground backdrop-blur-md shadow-lg transition hover:text-foreground',
              showSettings && 'bg-amber-500/15 border-amber-500/40 text-amber-400'
            )}
            title="Graph Forces & Display Settings"
          >
            <Sliders className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      {/* ─── Top Center: Progressive One-by-One Stream Indicator ─── */}
      {hasData && (
        <div className="absolute top-3 left-1/2 -translate-x-1/2 z-10 pointer-events-auto">
          {streamProgress.isStreaming ? (
            <div className="flex items-center gap-2 rounded-full border border-amber-500/40 bg-zinc-950/90 px-3.5 py-1 shadow-xl shadow-black/40 backdrop-blur-md text-[11px] font-mono text-amber-400 animate-in fade-in zoom-in-95 duration-200">
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75" />
                <span className="relative inline-flex rounded-full h-2 w-2 bg-amber-500" />
              </span>
              <span>
                Streaming entity {streamProgress.current} of {streamProgress.total}
              </span>
              <button
                onClick={handleLoadAllInstantly}
                className="ml-1 flex items-center gap-1 rounded bg-amber-500/20 px-2 py-0.5 text-[10px] font-semibold text-amber-300 hover:bg-amber-500/30 transition-colors"
                title="Skip progressive stream and reveal all nodes immediately"
              >
                <Zap className="h-2.5 w-2.5" /> Load All
              </button>
            </div>
          ) : (
            <div className="hidden sm:flex items-center gap-1.5 rounded-full border border-border/60 bg-zinc-950/80 px-3 py-1 shadow-md shadow-black/20 backdrop-blur-md text-[10px] font-mono text-muted-foreground">
              <CheckCircle2 className="h-3 w-3 text-emerald-400" />
              <span>{streamProgress.total} entities loaded in real-time</span>
            </div>
          )}
        </div>
      )}

      {/* ─── Top Right: HUD & Dynamic Legend ─── */}
      {hasData && (
        <div className="absolute top-3 right-3 z-10 flex items-start gap-2">
          <div className="flex flex-col gap-1 rounded-xl border border-border/60 bg-zinc-950/85 p-1 shadow-lg shadow-black/30 backdrop-blur-md">
            <button
              onClick={() => handleZoom(1.2)}
              className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted/50 hover:text-foreground transition"
              title="Zoom In"
            >
              <ZoomIn className="h-3.5 w-3.5" />
            </button>
            <button
              onClick={() => handleZoom(0.83)}
              className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted/50 hover:text-foreground transition"
              title="Zoom Out"
            >
              <ZoomOut className="h-3.5 w-3.5" />
            </button>
            <button
              onClick={handleFitToView}
              className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted/50 hover:text-foreground transition"
              title="Fit to Center"
            >
              <Maximize2 className="h-3.5 w-3.5" />
            </button>
            <button
              onClick={handleReheat}
              className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted/50 hover:text-foreground transition"
              title="Shuffle & Re-simulate"
            >
              <RotateCcw className="h-3.5 w-3.5" />
            </button>
          </div>

          {/* Dynamic Legend — only shows types that actually exist in data */}
          {legendTypes.length > 0 && (
            <div className="hidden sm:flex flex-col rounded-xl border border-border/60 bg-zinc-950/85 p-2.5 shadow-lg shadow-black/30 backdrop-blur-md text-[10px] space-y-1.5">
              <div className="font-semibold text-foreground border-b border-border/40 pb-1 flex items-center justify-between gap-4">
                <span>Graph Nodes</span>
                <span className="font-mono text-muted-foreground text-[9px]">
                  {nodesMapRef.current.size}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-muted-foreground">
                {legendTypes.map((type) => (
                  <div key={type} className="flex items-center gap-1.5">
                    <span
                      className="h-2 w-2 rounded-full shrink-0"
                      style={{ backgroundColor: TYPE_COLORS[type]?.main || '#94a3b8' }}
                    />
                    {type.charAt(0).toUpperCase() + type.slice(1)}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* ─── Settings Drawer ─── */}
      {showSettings && (
        <div className="absolute top-14 left-3 z-20 w-72 rounded-2xl border border-border bg-zinc-950/95 p-4 shadow-2xl backdrop-blur-md text-xs space-y-4 animate-in fade-in slide-in-from-top-2 duration-150">
          <div className="flex items-center justify-between border-b border-border/40 pb-2">
            <span className="font-bold text-foreground flex items-center gap-1.5">
              <Sliders className="h-3.5 w-3.5 text-amber-500" /> Graph Forces & Display
            </span>
            <button
              onClick={() => setShowSettings(false)}
              className="rounded-md p-1 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>

          {/* Force Sliders */}
          <div className="space-y-3 text-[11px]">
            <div>
              <div className="flex justify-between text-muted-foreground mb-1">
                <span>Repulsion Force</span>
                <span className="font-mono">{settings.repulsion}</span>
              </div>
              <input
                type="range"
                min="50"
                max="400"
                value={settings.repulsion}
                onChange={(e) => {
                  setSettings({ ...settings, repulsion: Number(e.target.value) });
                  simAlphaRef.current = 0.5;
                }}
                className="w-full accent-amber-500 h-1 bg-muted rounded-lg appearance-none"
              />
            </div>

            <div>
              <div className="flex justify-between text-muted-foreground mb-1">
                <span>Link Distance</span>
                <span className="font-mono">{settings.linkDistance}px</span>
              </div>
              <input
                type="range"
                min="30"
                max="150"
                value={settings.linkDistance}
                onChange={(e) => {
                  setSettings({ ...settings, linkDistance: Number(e.target.value) });
                  simAlphaRef.current = 0.5;
                }}
                className="w-full accent-amber-500 h-1 bg-muted rounded-lg appearance-none"
              />
            </div>

            <div>
              <div className="flex justify-between text-muted-foreground mb-1">
                <span>Center Gravity</span>
                <span className="font-mono">{settings.centerGravity}</span>
              </div>
              <input
                type="range"
                min="0.01"
                max="0.15"
                step="0.01"
                value={settings.centerGravity}
                onChange={(e) => {
                  setSettings({ ...settings, centerGravity: Number(e.target.value) });
                  simAlphaRef.current = 0.5;
                }}
                className="w-full accent-amber-500 h-1 bg-muted rounded-lg appearance-none"
              />
            </div>
          </div>

          {/* Toggles */}
          <div className="border-t border-border/40 pt-3 space-y-2 text-[11px]">
            <label className="flex items-center justify-between cursor-pointer text-muted-foreground hover:text-foreground">
              <span>Progressive One-by-One Stream</span>
              <input
                type="checkbox"
                checked={settings.progressiveStream}
                onChange={(e) => setSettings({ ...settings, progressiveStream: e.target.checked })}
                className="rounded border-border accent-amber-500"
              />
            </label>
            <label className="flex items-center justify-between cursor-pointer text-muted-foreground hover:text-foreground">
              <span>Show Text Labels</span>
              <input
                type="checkbox"
                checked={settings.showLabels}
                onChange={(e) => setSettings({ ...settings, showLabels: e.target.checked })}
                className="rounded border-border accent-amber-500"
              />
            </label>
            <label className="flex items-center justify-between cursor-pointer text-muted-foreground hover:text-foreground">
              <span>Node Radial Glow</span>
              <input
                type="checkbox"
                checked={settings.showGlow}
                onChange={(e) => setSettings({ ...settings, showGlow: e.target.checked })}
                className="rounded border-border accent-amber-500"
              />
            </label>
            <label className="flex items-center justify-between cursor-pointer text-muted-foreground hover:text-foreground">
              <span>Edge Pulse Particles</span>
              <input
                type="checkbox"
                checked={settings.showParticles}
                onChange={(e) => setSettings({ ...settings, showParticles: e.target.checked })}
                className="rounded border-border accent-amber-500"
              />
            </label>
            <label className="flex items-center justify-between cursor-pointer text-muted-foreground hover:text-foreground">
              <span>Curved Relationship Lines</span>
              <input
                type="checkbox"
                checked={settings.curvedLinks}
                onChange={(e) => setSettings({ ...settings, curvedLinks: e.target.checked })}
                className="rounded border-border accent-amber-500"
              />
            </label>
          </div>
        </div>
      )}

      {/* ─── Bottom Floating Node Inspector ─── */}
      {activeNodeInfo && (
        <div className="absolute bottom-3 left-3 right-3 sm:right-auto sm:max-w-md z-20 rounded-xl border border-border/80 bg-zinc-950/95 p-3.5 shadow-2xl backdrop-blur-md animate-in fade-in slide-in-from-bottom-2 duration-150">
          <div className="flex items-center justify-between border-b border-border/40 pb-2 mb-2">
            <div className="flex items-center gap-2 min-w-0">
              <span
                className="h-2.5 w-2.5 rounded-full shrink-0 shadow-sm"
                style={{ backgroundColor: activeNodeInfo.color }}
              />
              <span className="font-bold text-foreground text-xs truncate">
                {activeNodeInfo.name}
              </span>
            </div>
            <div className="flex items-center gap-1.5 shrink-0">
              <span className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[9px] font-semibold uppercase text-muted-foreground">
                {activeNodeInfo.type}
              </span>
              <span className="rounded bg-amber-500/10 text-amber-400 px-1.5 py-0.5 font-mono text-[9px]">
                {activeNodeInfo.degree} links
              </span>
            </div>
          </div>

          <div className="text-muted-foreground font-mono text-[10px] leading-relaxed break-words max-h-24 overflow-y-auto pr-1">
            {(() => {
              try {
                const obs = JSON.parse(activeNodeInfo.observations);
                if (Array.isArray(obs) && obs.length > 0) {
                  return obs.map((o: unknown, idx: number) => (
                    <div key={idx} className="flex items-start gap-1.5 py-0.5">
                      <span className="text-amber-500">•</span>
                      <span>{String(o)}</span>
                    </div>
                  ));
                }
                return String(obs);
              } catch {
                return (
                  activeNodeInfo.observations || 'No additional observations recorded.'
                );
              }
            })()}
          </div>
        </div>
      )}
    </div>
  );
};
