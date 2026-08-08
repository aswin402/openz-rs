import React, { useEffect, useRef, useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import {
  ArrowLeft,
  BrainCircuit,
  Database,
  ExternalLink,
  KeyRound,
  Share2,
  FileCode,
  AlertCircle,
  Copy,
  Check,
  RefreshCw,
  Download,
  Filter,
  Search
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { CognitiveNode, CognitiveEdge } from '../types/openz';

interface SimNode {
  name: string;
  type: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  observations: string;
}

interface SimEdge {
  source: SimNode;
  target: SimNode;
  type: string;
}

interface GraphVisualizerProps {
  nodes: CognitiveNode[];
  edges: CognitiveEdge[];
}

function observationText(value: string): string {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.join(' ') : String(parsed);
  } catch {
    return value || '';
  }
}

function downloadTextFile(filename: string, content: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

export const GraphVisualizer: React.FC<GraphVisualizerProps> = ({ nodes, edges }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [hoveredNode, setHoveredNode] = useState<SimNode | null>(null);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);

  const simNodesRef = useRef<SimNode[]>([]);
  const simEdgesRef = useRef<SimEdge[]>([]);
  const hoveredNodeRef = useRef<SimNode | null>(null);
  const selectedNodeRef = useRef<SimNode | null>(null);

  useEffect(() => {
    hoveredNodeRef.current = hoveredNode;
  }, [hoveredNode]);

  useEffect(() => {
    selectedNodeRef.current = selectedNode;
  }, [selectedNode]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const resizeCanvas = () => {
      if (containerRef.current && canvas) {
        canvas.width = containerRef.current.clientWidth;
        canvas.height = 420;
      }
    };
    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    const getColor = (type: string) => {
      switch (type.toLowerCase()) {
        case 'user': return '#60a5fa'; // soft blue
        case 'agent': return '#fbbf24'; // soft amber
        case 'workspace': return '#34d399'; // soft emerald
        case 'file': return '#f472b6'; // soft pink
        case 'concept': return '#a78bfa'; // soft purple
        default: return '#a1a1aa'; // zinc-400
      }
    };

    const w = canvas.width || 800;
    const h = canvas.height || 420;

    // Sync simulation nodes from prop updates (keeping positions of existing nodes)
    const existingNodesMap = new Map<string, SimNode>();
    simNodesRef.current.forEach((n) => existingNodesMap.set(n.name, n));

    const updatedSimNodes: SimNode[] = nodes.map((n) => {
      const existing = existingNodesMap.get(n.name);
      if (existing) {
        existing.color = getColor(n.entity_type);
        existing.observations = n.observations;
        return existing;
      }
      
      const angle = Math.random() * Math.PI * 2;
      const distance = 30 + Math.random() * 90;
      return {
        name: n.name,
        type: n.entity_type,
        x: w / 2 + Math.cos(angle) * distance,
        y: h / 2 + Math.sin(angle) * distance,
        vx: 0,
        vy: 0,
        radius: n.entity_type.toLowerCase() === 'agent' ? 3.5 : 2.2,
        color: getColor(n.entity_type),
        observations: n.observations,
      };
    });

    simNodesRef.current = updatedSimNodes;

    const nodeMap = new Map<string, SimNode>();
    simNodesRef.current.forEach((sn) => nodeMap.set(sn.name, sn));

    // Sync edges
    simEdgesRef.current = edges
      .map((e) => {
        const source = nodeMap.get(e.from_name);
        const target = nodeMap.get(e.to_name);
        if (source && target) {
          return { source, target, type: e.relation_type };
        }
        return null;
      })
      .filter((e): e is SimEdge => e !== null);

    const simNodes = simNodesRef.current;
    const simEdges = simEdgesRef.current;

    let animationId: number;
    let draggedNode: SimNode | null = null;

    const gravity = 0.04;
    const repulsion = 80;
    const linkForce = 0.08;
    const damping = 0.85;

    const tick = () => {
      if (!canvas || !ctx) return;

      const width = canvas.width;
      const height = canvas.height;
      const centerX = width / 2;
      const centerY = height / 2;

      // N^2 Repulsion
      for (let i = 0; i < simNodes.length; i++) {
        const nodeA = simNodes[i];
        for (let j = i + 1; j < simNodes.length; j++) {
          const nodeB = simNodes[j];
          const dx = nodeB.x - nodeA.x;
          const dy = nodeB.y - nodeA.y;
          const distSq = dx * dx + dy * dy + 0.1;
          const dist = Math.sqrt(distSq);

          if (dist < 120) {
            const force = (repulsion / distSq) * 1.2;
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;

            if (nodeA !== draggedNode) {
              nodeA.vx -= fx;
              nodeA.vy -= fy;
            }
            if (nodeB !== draggedNode) {
              nodeB.vx += fx;
              nodeB.vy += fy;
            }
          }
        }
      }

      // Link Attraction
      simEdges.forEach((link) => {
        const dx = link.target.x - link.source.x;
        const dy = link.target.y - link.source.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 0.1;
        const targetDist = 45;
        const k = (dist - targetDist) * linkForce;
        const fx = (dx / dist) * k;
        const fy = (dy / dist) * k;

        if (link.source !== draggedNode) {
          link.source.vx += fx;
          link.source.vy += fy;
        }
        if (link.target !== draggedNode) {
          link.target.vx -= fx;
          link.target.vy -= fy;
        }
      });

      // Gravity and constraints
      simNodes.forEach((node) => {
        if (node === draggedNode) return;

        const dx = centerX - node.x;
        const dy = centerY - node.y;
        node.vx += dx * gravity * 0.03;
        node.vy += dy * gravity * 0.03;

        node.x += node.vx;
        node.y += node.vy;
        node.vx *= damping;
        node.vy *= damping;

        const pad = 15;
        if (node.x < pad) { node.x = pad; node.vx = 0; }
        if (node.x > width - pad) { node.x = width - pad; node.vx = 0; }
        if (node.y < pad) { node.y = pad; node.vy = 0; }
        if (node.y > height - pad) { node.y = height - pad; node.vy = 0; }
      });

      // Render clean dark background
      ctx.clearRect(0, 0, width, height);

      // Determine active hovered/selected scope for dimmed render highlights
      const hovered = hoveredNodeRef.current;
      const selected = selectedNodeRef.current;
      const activeNode = hovered || selected;

      const connectedNodeNames = new Set<string>();
      if (activeNode) {
        connectedNodeNames.add(activeNode.name);
        simEdges.forEach((link) => {
          if (link.source.name === activeNode.name) {
            connectedNodeNames.add(link.target.name);
          }
          if (link.target.name === activeNode.name) {
            connectedNodeNames.add(link.source.name);
          }
        });
      }

      // Draw links
      simEdges.forEach((link) => {
        ctx.beginPath();
        ctx.moveTo(link.source.x, link.source.y);
        ctx.lineTo(link.target.x, link.target.y);
        
        let alpha = 0.08;
        let lineWidth = 0.4;
        if (activeNode) {
          const isConnected = (link.source.name === activeNode.name || link.target.name === activeNode.name);
          alpha = isConnected ? 0.35 : 0.02;
          lineWidth = isConnected ? 0.75 : 0.2;
        }

        ctx.strokeStyle = `rgba(228, 228, 231, ${alpha})`;
        ctx.lineWidth = lineWidth;
        ctx.stroke();
      });

      // Draw nodes
      simNodes.forEach((node) => {
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
        
        let alpha = 1;
        if (activeNode) {
          alpha = connectedNodeNames.has(node.name) ? 1 : 0.15;
        }

        ctx.fillStyle = node.color;
        ctx.globalAlpha = alpha;
        ctx.fill();

        ctx.lineWidth = 0.6;
        ctx.strokeStyle = '#09090b';
        ctx.stroke();

        ctx.globalAlpha = 1; // reset alpha

        if (node === hovered || node === selected) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius + 3.5, 0, Math.PI * 2);
          ctx.strokeStyle = node.color;
          ctx.lineWidth = 0.8;
          ctx.stroke();
        }

        if (node === hovered || node === selected || (activeNode && connectedNodeNames.has(node.name))) {
          ctx.font = '9px monospace';
          ctx.fillStyle = '#e4e4e7';
          ctx.fillText(node.name, node.x + node.radius + 4, node.y + 3);
        }
      });

      animationId = requestAnimationFrame(tick);
    };

    const getMousePos = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      return {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
      };
    };

    const handleMouseMove = (e: MouseEvent) => {
      const pos = getMousePos(e);
      if (draggedNode) {
        draggedNode.x = pos.x;
        draggedNode.y = pos.y;
        draggedNode.vx = 0;
        draggedNode.vy = 0;
        return;
      }


      let foundHover: SimNode | null = null;
      for (const node of simNodes) {
        const dx = node.x - pos.x;
        const dy = node.y - pos.y;
        if (dx * dx + dy * dy < (node.radius + 6) * (node.radius + 6)) {
          foundHover = node;
          break;
        }
      }
      setHoveredNode(foundHover);
    };

    const handleMouseDown = (e: MouseEvent) => {
      const pos = getMousePos(e);
      let foundNode: SimNode | null = null;
      for (const node of simNodes) {
        const dx = node.x - pos.x;
        const dy = node.y - pos.y;
        if (dx * dx + dy * dy < (node.radius + 6) * (node.radius + 6)) {
          foundNode = node;
          break;
        }
      }

      if (foundNode) {
        draggedNode = foundNode;
        setSelectedNode(foundNode);
      } else {
        setSelectedNode(null);
      }
    };

    const handleMouseUp = () => {
      draggedNode = null;
    };

    canvas.addEventListener('mousemove', handleMouseMove);
    canvas.addEventListener('mousedown', handleMouseDown);
    window.addEventListener('mouseup', handleMouseUp);

    animationId = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(animationId);
      window.removeEventListener('resize', resizeCanvas);
      if (canvas) {
        canvas.removeEventListener('mousemove', handleMouseMove);
        canvas.removeEventListener('mousedown', handleMouseDown);
      }
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [nodes, edges]);

  return (
    <div ref={containerRef} className="relative w-full rounded-xl border border-border bg-black/45 overflow-hidden shadow-inner">
      <canvas ref={canvasRef} className="block cursor-grab active:cursor-grabbing w-full h-[420px]" />
      
      {/* Legend overlay */}
      <div className="absolute top-3 right-3 rounded-md bg-zinc-950/90 border border-border/60 p-2 text-[9px] text-muted-foreground select-none pointer-events-none space-y-1 z-10">
        <div className="font-semibold text-foreground border-b border-border/40 pb-0.5 mb-1">Node Legend</div>
        <div className="flex items-center gap-1.5"><span className="h-2 w-2 rounded-full bg-amber-500" /> Agent</div>
        <div className="flex items-center gap-1.5"><span className="h-2 w-2 rounded-full bg-blue-500" /> User</div>
        <div className="flex items-center gap-1.5"><span className="h-2 w-2 rounded-full bg-emerald-500" /> Workspace</div>
        <div className="flex items-center gap-1.5"><span className="h-2 w-2 rounded-full bg-pink-500" /> File</div>
        <div className="flex items-center gap-1.5"><span className="h-2 w-2 rounded-full bg-purple-500" /> Concept</div>
      </div>

      {/* Detail overlay card */}
      {(hoveredNode || selectedNode) && (
        <div className="absolute bottom-3 left-3 right-3 max-w-sm rounded-lg border border-amber-500/20 bg-zinc-950/95 p-3 text-[11px] shadow-lg animate-in fade-in slide-in-from-bottom-1 duration-100 z-10">
          <div className="flex items-center gap-2 border-b border-border/40 pb-1 mb-1.5">
            <span
              className="h-2 w-2 rounded-full"
              style={{ backgroundColor: (hoveredNode || selectedNode)!.color }}
            />
            <span className="font-semibold text-foreground truncate">{(hoveredNode || selectedNode)!.name}</span>
            <span className="ml-auto rounded bg-muted/60 px-1 py-0.5 font-mono text-[9px] uppercase">
              {(hoveredNode || selectedNode)!.type}
            </span>
          </div>
          <div className="text-muted-foreground leading-relaxed break-words font-mono text-[10px] max-h-16 overflow-y-auto">
            {(() => {
              try {
                const obs = JSON.parse((hoveredNode || selectedNode)!.observations);
                return Array.isArray(obs) ? obs.join(' • ') : String(obs);
              } catch {
                return (hoveredNode || selectedNode)!.observations || 'No observations recorded.';
              }
            })()}
          </div>
        </div>
      )}
    </div>
  );
};

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  
  const [activeTab, setActiveTab] = useState<'graph' | 'markdown' | 'facts'>('graph');
  const [copied, setCopied] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [nodeTypeFilter, setNodeTypeFilter] = useState('all');
  const [factTagFilter, setFactTagFilter] = useState('all');
  const [sortMode, setSortMode] = useState<'importance' | 'newest'>('importance');

  useEffect(() => {
    wsService.requestCognitiveMemory();
  }, []);

  const handleRefresh = () => {
    setIsRefreshing(true);
    wsService.requestCognitiveMemory();
    setTimeout(() => setIsRefreshing(false), 800);
  };

  const nodeTypes = Array.from(new Set((cognitiveStats.nodes || []).map((node) => node.entity_type).filter(Boolean))).sort();
  const factTags = Array.from(new Set((cognitiveStats.facts || []).flatMap((fact) =>
    fact.tags.split(',').map((tag) => tag.trim()).filter(Boolean)
  ))).sort();

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const filteredNodes = (cognitiveStats.nodes || []).filter((node) => {
    const matchesType = nodeTypeFilter === 'all' || node.entity_type === nodeTypeFilter;
    const haystack = [node.name, node.entity_type, observationText(node.observations)].join(' ').toLowerCase();
    return matchesType && (!normalizedSearch || haystack.includes(normalizedSearch));
  });
  const filteredNodeNames = new Set(filteredNodes.map((node) => node.name));
  const filteredEdges = (cognitiveStats.edges || []).filter((edge) => {
    const endpointMatch = filteredNodeNames.has(edge.from_name) || filteredNodeNames.has(edge.to_name);
    const haystack = [edge.from_name, edge.to_name, edge.relation_type].join(' ').toLowerCase();
    return endpointMatch && (!normalizedSearch || haystack.includes(normalizedSearch) || endpointMatch);
  });
  const filteredFacts = (cognitiveStats.facts || [])
    .filter((fact) => {
      const matchesTag = factTagFilter === 'all' || fact.tags.split(',').map((tag) => tag.trim()).includes(factTagFilter);
      const haystack = [fact.text, fact.tags, fact.timestamp, String(fact.importance)].join(' ').toLowerCase();
      return matchesTag && (!normalizedSearch || haystack.includes(normalizedSearch));
    })
    .sort((a, b) => sortMode === 'importance'
      ? b.importance - a.importance
      : new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  const snapshotStats = {
    entitiesCount: filteredNodes.length,
    relationsCount: filteredEdges.length,
    factsCount: filteredFacts.length,
  };

  const generateMarkdownString = () => {
    let md = `# Knowledge Graph Memory Snapshot\n\n`;
    md += `Generated at: \`${new Date().toLocaleString()}\`\n`;
    md += `Path: \`~/.openz/memory.db\` and \`~/.openz/graph_memory.db\`\n\n`;
    
    md += `## 1. Node Entities (${snapshotStats.entitiesCount})\n\n`;
    if (filteredNodes.length > 0) {
      filteredNodes.forEach((n) => {
        md += `### [[${n.name}]] (${n.entity_type})\n`;
        try {
          const obs = JSON.parse(n.observations);
          if (Array.isArray(obs) && obs.length > 0) {
            obs.forEach(o => {
              md += `- ${o}\n`;
            });
          } else {
            md += `- ${n.observations}\n`;
          }
        } catch {
          md += `- ${n.observations || 'No observations'}\n`;
        }
        md += `\n`;
      });
    } else {
      md += `*No nodes present in database.*\n\n`;
    }

    md += `## 2. Relationships (${snapshotStats.relationsCount})\n\n`;
    if (filteredEdges.length > 0) {
      md += `| Source Entity | Target Entity | Relation Type |\n`;
      md += `| :--- | :--- | :--- |\n`;
      filteredEdges.forEach((e) => {
        md += `| [[${e.from_name}]] | [[${e.to_name}]] | \`${e.relation_type}\` |\n`;
      });
      md += `\n`;
    } else {
      md += `*No edges present in database.*\n\n`;
    }

    md += `## 3. Stored Cognitive Facts (${snapshotStats.factsCount})\n\n`;
    if (filteredFacts.length > 0) {
      filteredFacts.forEach((f) => {
        md += `- **Fact**: ${f.text}\n`;
        md += `  - *Importance*: ${f.importance} | *Timestamp*: ${f.timestamp}\n`;
        if (f.tags) md += `  - *Tags*: \`${f.tags}\`\n`;
        md += `\n`;
      });
    } else {
      md += `*No long-term facts compiled yet by background curators. Learnings are saved automatically as conversation progresses or when manual memory tools are triggered.*\n\n`;
    }
    
    return md;
  };

  const handleCopyMarkdown = () => {
    navigator.clipboard.writeText(generateMarkdownString());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadMarkdown = () => {
    downloadTextFile('openz-knowledge-snapshot.md', generateMarkdownString(), 'text/markdown;charset=utf-8');
  };

  const handleDownloadJson = () => {
    const payload = {
      generatedAt: new Date().toISOString(),
      filters: { searchQuery, nodeTypeFilter, factTagFilter, sortMode },
      stats: snapshotStats,
      nodes: filteredNodes,
      edges: filteredEdges,
      facts: filteredFacts,
      workingMemoryKeys: cognitiveStats.workingMemoryKeys,
    };
    downloadTextFile('openz-knowledge-snapshot.json', JSON.stringify(payload, null, 2), 'application/json;charset=utf-8');
  };

  const cards = [
    { label: 'Entities', value: snapshotStats.entitiesCount, icon: BrainCircuit, color: 'text-amber-500' },
    { label: 'Relations', value: snapshotStats.relationsCount, icon: Share2, color: 'text-purple-400' },
    { label: 'Stored Facts', value: snapshotStats.factsCount, icon: Database, color: 'text-emerald-400' },
  ];

  return (
    <div className="mx-auto max-w-4xl px-4 py-6 space-y-6">
      {/* Header section */}
      <div className="flex items-center justify-between pb-4 border-b border-border/50">
        <div className="space-y-1">
          <button
            onClick={() => setActiveView('dashboard')}
            className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-muted/20 px-3 py-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors"
          >
            <ArrowLeft className="h-3.5 w-3.5" /> Go Back
          </button>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground pt-2">
            <BrainCircuit className="h-6 w-6 text-amber-500" /> Cognitive Memory Graph
          </h1>
          <p className="text-xs text-muted-foreground">
            Semantic entity-relation graph nodes, indexed facts, and compactor working memory.
          </p>
        </div>

        <button
          onClick={handleRefresh}
          className="flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-2 text-xs font-semibold text-foreground hover:bg-muted transition duration-150"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? 'animate-spin text-amber-500' : ''}`} />
          Sync Graph
        </button>
      </div>

      <div className="rounded-xl border border-border/70 bg-card/40 p-3 shadow-sm">
        <div className="grid gap-3 lg:grid-cols-[1fr_160px_160px_140px]">
          <div className="relative">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search nodes, relations, facts, tags..."
              className="w-full rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            />
          </div>
          <label className="relative">
            <Filter className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <select
              value={nodeTypeFilter}
              onChange={(e) => setNodeTypeFilter(e.target.value)}
              className="w-full appearance-none rounded-lg border border-border/60 bg-background py-2 pl-9 pr-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All node types</option>
              {nodeTypes.map((type) => <option key={type} value={type}>{type}</option>)}
            </select>
          </label>
          <label>
            <select
              value={factTagFilter}
              onChange={(e) => setFactTagFilter(e.target.value)}
              className="w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="all">All fact tags</option>
              {factTags.map((tag) => <option key={tag} value={tag}>{tag}</option>)}
            </select>
          </label>
          <label>
            <select
              value={sortMode}
              onChange={(e) => setSortMode(e.target.value as 'importance' | 'newest')}
              className="w-full rounded-lg border border-border/60 bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/50"
            >
              <option value="importance">Importance</option>
              <option value="newest">Newest</option>
            </select>
          </label>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] text-muted-foreground">
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">visible: {filteredNodes.length} nodes</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{filteredEdges.length} relations</span>
          <span className="rounded bg-muted/50 px-2 py-0.5 font-mono">{filteredFacts.length} facts</span>
          {(searchQuery || nodeTypeFilter !== 'all' || factTagFilter !== 'all') && (
            <button
              type="button"
              onClick={() => { setSearchQuery(''); setNodeTypeFilter('all'); setFactTagFilter('all'); }}
              className="ml-auto rounded border border-border/60 px-2 py-0.5 font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
            >
              Clear filters
            </button>
          )}
        </div>
      </div>

      {/* Metrics Row */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div key={card.label} className="rounded-xl border border-border bg-card p-4 shadow-sm">
              <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {card.label}
                <Icon className={`h-4 w-4 ${card.color}`} />
              </div>
              <div className="mt-1.5 text-2xl font-extrabold tracking-tight text-foreground">{card.value}</div>
            </div>
          );
        })}
      </div>

      {/* Mode Selector Tab Row */}
      <div className="flex items-center justify-between border-b border-border/40 pb-2">
        <div className="flex gap-1.5">
          <button
            onClick={() => setActiveTab('graph')}
            className={`rounded-lg px-3.5 py-2 text-xs font-semibold transition-colors ${
              activeTab === 'graph'
                ? 'bg-amber-500/10 text-amber-400 border border-amber-500/25'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Obsidian Graph View
          </button>
          <button
            onClick={() => setActiveTab('markdown')}
            className={`rounded-lg px-3.5 py-2 text-xs font-semibold transition-colors ${
              activeTab === 'markdown'
                ? 'bg-amber-500/10 text-amber-400 border border-amber-500/25'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Markdown File (.md)
          </button>
          <button
            onClick={() => setActiveTab('facts')}
            className={`rounded-lg px-3.5 py-2 text-xs font-semibold transition-colors ${
              activeTab === 'facts'
                ? 'bg-amber-500/10 text-amber-400 border border-amber-500/25'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            }`}
          >
            Stored Facts List
          </button>
        </div>

        <div className="flex items-center gap-2">
          {activeTab === 'markdown' && (
            <button
              onClick={handleCopyMarkdown}
              className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              {copied ? (
                <>
                  <Check className="h-3.5 w-3.5 text-emerald-500" /> Copied!
                </>
              ) : (
                <>
                  <Copy className="h-3.5 w-3.5" /> Copy Markdown
                </>
              )}
            </button>
          )}
          <button
            onClick={handleDownloadMarkdown}
            className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <Download className="h-3.5 w-3.5" /> MD
          </button>
          <button
            onClick={handleDownloadJson}
            className="flex items-center gap-1.5 rounded-lg border border-border/80 bg-muted/20 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <Download className="h-3.5 w-3.5" /> JSON
          </button>
        </div>
      </div>

      {/* Main Tab Panels */}
      <div className="min-h-[400px]">
        {activeTab === 'graph' && (
          <div className="space-y-3">
            <GraphVisualizer
              nodes={filteredNodes}
              edges={filteredEdges}
            />
            <p className="text-[10px] text-muted-foreground text-center select-none">
              💡 Drag nodes to pin them and inspect entity properties by hovering/selecting.
            </p>
          </div>
        )}

        {activeTab === 'markdown' && (
          <div className="rounded-xl border border-border bg-black/40 shadow-inner overflow-hidden flex flex-col max-h-[500px]">
            {/* Editor Top Bar mock */}
            <div className="flex items-center gap-1.5 border-b border-border/60 bg-muted/20 px-4 py-2 select-none">
              <FileCode className="h-3.5 w-3.5 text-amber-500" />
              <span className="font-mono text-[10px] text-muted-foreground">cognitive_graph_snapshot.md</span>
              <span className="ml-auto rounded bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 text-[9px] text-emerald-400 font-semibold font-mono">
                READ-ONLY
              </span>
            </div>
            {/* Editor Body */}
            <div className="flex-1 overflow-y-auto p-5 text-xs font-mono prose prose-invert max-w-none text-foreground prose-xs scrollbar-thin">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {generateMarkdownString()}
              </ReactMarkdown>
            </div>
          </div>
        )}

        {activeTab === 'facts' && (
          <div className="space-y-4">
            {/* Explanatory banner if facts are 0 */}
            {filteredFacts.length === 0 && (
              <div className="rounded-xl border border-amber-500/10 bg-amber-500/5 p-4 flex gap-3 text-xs leading-relaxed">
                <AlertCircle className="h-5 w-5 text-amber-500 shrink-0 mt-0.5" />
                <div className="space-y-2">
                  <div className="font-semibold text-amber-400">Why are my Stored Facts showing 0?</div>
                  <p className="text-muted-foreground text-[11px]">
                    OpenZ utilizes an asynchronous **Self-Improvement Memory Curator** that spawns in the background after chat loops. If your conversation history is still brief or has not reached compaction thresholds (configured via <code>max_messages</code>), the curator might not have generated long-term facts/memories yet.
                  </p>
                  <p className="text-muted-foreground text-[11px]">
                    Additionally, you can manually trigger memory generation or scope entities directly by executing LLM prompts calling memory tools (such as <code>set_working_memory</code>, <code>extract_and_store_facts</code>, or <code>smart_store</code>).
                  </p>
                </div>
              </div>
            )}

            {/* List of facts */}
            <div className="space-y-2 max-h-[420px] overflow-y-auto pr-1">
              {filteredFacts.length === 0 ? (
                <div className="rounded-xl border border-border/50 bg-card/40 p-8 text-center text-xs text-muted-foreground select-none">
                  No facts in cognitive database. Start chatting or execute memory tools to store memories!
                </div>
              ) : (
                filteredFacts.map((fact, idx) => (
                  <div key={idx} className="rounded-xl border border-border bg-card p-4 shadow-sm hover:border-amber-500/30 transition-colors">
                    <div className="flex items-center gap-2 mb-1.5">
                      <span className="rounded bg-amber-500/10 border border-amber-500/20 px-1.5 py-0.5 text-[9px] text-amber-400 font-semibold font-mono">
                        Importance: {fact.importance.toFixed(2)}
                      </span>
                      {fact.tags && (
                        <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] text-muted-foreground font-mono truncate max-w-[200px]">
                          Tags: {fact.tags}
                        </span>
                      )}
                      <span className="ml-auto text-[9px] text-muted-foreground font-mono">
                        {fact.timestamp}
                      </span>
                    </div>
                    <p className="text-xs text-foreground leading-relaxed break-words font-sans">{fact.text}</p>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {/* Active Working Memory scope */}
      <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
        <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          <KeyRound className="h-4 w-4 text-amber-500" /> Working Memory Scope
          <span className="ml-auto rounded-full bg-muted/70 px-2 py-0.5 font-mono text-[10px]">
            {cognitiveStats.workingMemoryKeys.length}
          </span>
        </div>
        {cognitiveStats.workingMemoryKeys.length === 0 ? (
          <div className="mt-3 text-xs text-muted-foreground/70 select-none">
            No working memory keys scope set. The agent will populate them dynamically.
          </div>
        ) : (
          <div className="mt-3 flex flex-wrap gap-2">
            {cognitiveStats.workingMemoryKeys.map((key) => (
              <span
                key={key}
                className="rounded-md border border-border bg-muted/30 px-2.5 py-1 font-mono text-[10px] text-foreground"
              >
                {key}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Inspector control */}
      <div className="flex justify-end pt-2">
        <button
          onClick={() => {
            setIsMemoryOpen(true);
            wsService.requestCognitiveMemory();
          }}
          className="flex items-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2.5 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20"
        >
          <ExternalLink className="h-4 w-4" /> Open full memory inspector
        </button>
      </div>
    </div>
  );
};