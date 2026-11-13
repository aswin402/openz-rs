import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useOpenZStore } from '../../store/useOpenZStore';
import { wsService } from '../../services/websocket';
import {
  AlertCircle,
  ArrowLeft,
  BrainCircuit,
  Check,
  CheckSquare,
  ChevronDown,
  ChevronUp,
  Compass,
  Copy,
  Database,
  Download,
  ExternalLink,
  FileCode,
  Filter,
  Focus,
  KeyRound,
  Layers,
  Maximize2,
  RefreshCw,
  RotateCcw,
  Search,
  SlidersHorizontal,
  Sparkles,
  Square,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  DEFAULT_GRAPH_DISPLAY,
  ObsidianGraph,
  type ObsidianGraphHandle,
} from './ObsidianGraph';
import type { CognitiveEdge, CognitiveNode } from '../../types/openz';
import { cn } from '../../lib/utils';
import type { GraphMode } from './graphLayout';
import {
  buildGraphSemantics,
  edgeKey,
  formatConfidence,
  formatProvenance,
  type EdgeSemantics,
  type NodeMetrics,
} from './graphSemantics';
import { formatGraphStats, type GraphStats } from './graphStats';
import { getNodeColor, type GraphDisplayState } from './graph/graphCanvas';
import { formatObservations } from '../../shared/lib/format';
import { TIMING_MS } from '../../config/runtime';
import { Switch } from '../../components/ui/switch';

export const GraphVisualizer: React.FC<{ nodes: CognitiveNode[]; edges: CognitiveEdge[] }> = ({
  nodes,
  edges,
}) => {
  const facts = useOpenZStore((s) => s.cognitiveStats.facts || []);
  return <ObsidianGraph nodes={nodes} edges={edges} facts={facts} height={500} />;
};

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

type ControlSection = 'filters' | 'display' | 'vault';
type NodeSortOrder = 'degree' | 'name' | 'type';
type VaultDrawerMode = 'none' | 'facts' | 'markdown';

const DISPLAY_OPTIONS: ReadonlyArray<{
  key: keyof GraphDisplayState;
  label: string;
  hint: string;
}> = [
  { key: 'showLabels', label: 'Node labels', hint: 'Entity titles on zoom & hubs' },
  { key: 'showGlow', label: 'Ambient glow', hint: 'Theme-derived node aura' },
  { key: 'showParticles', label: 'Link particles', hint: 'Directional relation flow' },
  { key: 'curvedLinks', label: 'Curved links', hint: 'Arc relation trajectories' },
  { key: 'showSpaceField', label: 'Depth field', hint: 'Subtle ambient backdrop' },
  { key: 'showGrid', label: 'Coordinate grid', hint: 'Spatial alignment lines' },
  { key: 'showOrbits', label: 'Neighbor orbits', hint: 'Radial ring on active hub' },
];

interface FloatingInspectorProps {
  node: CognitiveNode | null;
  nodeMetric: NodeMetrics | undefined;
  edges: CognitiveEdge[];
  neighbors: CognitiveNode[];
  edgeSemantics: Map<string, EdgeSemantics>;
  selectedEdge: CognitiveEdge | null;
  selectedEdgeKey: string | null;
  isChecked: boolean;
  graphMode: GraphMode;
  topHubNode: CognitiveNode | null;
  collapsed: boolean;
  onToggleCollapse: () => void;
  onToggleChecked: (nodeName: string) => void;
  onSelectNode: (nodeName: string) => void;
  onSelectEdge: (key: string) => void;
  onToggleNeighborhood: () => void;
  onCenterNode: (nodeName: string) => void;
  onClear: () => void;
}

const FloatingEntityInspector: React.FC<FloatingInspectorProps> = ({
  node,
  nodeMetric,
  edges,
  neighbors,
  edgeSemantics,
  selectedEdge,
  selectedEdgeKey,
  isChecked,
  graphMode,
  topHubNode,
  collapsed,
  onToggleCollapse,
  onToggleChecked,
  onSelectNode,
  onSelectEdge,
  onToggleNeighborhood,
  onCenterNode,
  onClear,
}) => {
  if (!node && !selectedEdge) {
    return (
      <aside
        aria-label="Entity inspector standby"
        className="pointer-events-auto flex items-center gap-2 rounded-2xl border border-border/75 bg-card/88 px-3 py-2 text-xs shadow-lg backdrop-blur-md"
      >
        <Compass className="h-3.5 w-3.5 shrink-0 text-primary" />
        <span className="truncate text-[11px] text-muted-foreground">
          Select a node or relation to inspect
        </span>
        {topHubNode && (
          <button
            type="button"
            onClick={() => onSelectNode(topHubNode.name)}
            className="ml-1 shrink-0 rounded-lg border border-primary/25 bg-primary/10 px-2 py-1 text-[10px] font-semibold text-primary transition hover:bg-primary/20"
          >
            Focus hub
          </button>
        )}
      </aside>
    );
  }

  const entityColor = node ? getNodeColor(node.entity_type).main : undefined;
  const importancePct = Math.round((nodeMetric?.importance ?? 0) * 100);
  const selectedEdgeSemantics = selectedEdgeKey ? edgeSemantics.get(selectedEdgeKey) : undefined;

  return (
    <aside
      aria-label="Selected memory entity"
      className="pointer-events-auto flex max-h-[calc(100vh-6.75rem)] w-full flex-col overflow-hidden rounded-2xl border border-border/75 bg-card/90 shadow-2xl backdrop-blur-md"
    >
      <div className="flex items-start justify-between gap-2 border-b border-border/60 px-3.5 py-2.5">
        <div className="min-w-0 flex-1">
          {node ? (
            <>
              <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: entityColor }}
                />
                <span className="truncate">{node.entity_type || 'entity'}</span>
              </div>
              <h2 className="mt-0.5 break-words text-sm font-bold leading-snug text-foreground">
                {node.name}
              </h2>
            </>
          ) : (
            <>
              <div className="text-[10px] font-semibold uppercase tracking-wider text-info">
                Selected Relation
              </div>
              <h2 className="mt-0.5 break-words text-xs font-bold text-foreground">
                {selectedEdge?.from_name} → {selectedEdge?.to_name}
              </h2>
            </>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {node && (
            <button
              type="button"
              onClick={() => onToggleChecked(node.name)}
              className={cn(
                'rounded-lg p-1.5 transition',
                isChecked
                  ? 'bg-primary/15 text-primary'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground',
              )}
              title={isChecked ? 'Uncheck node' : 'Check & highlight node'}
              aria-label={isChecked ? 'Uncheck selected node' : 'Check selected node'}
            >
              {isChecked ? <CheckSquare className="h-3.5 w-3.5" /> : <Square className="h-3.5 w-3.5" />}
            </button>
          )}
          <button
            type="button"
            onClick={onToggleCollapse}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label={collapsed ? 'Expand entity inspector' : 'Collapse entity inspector'}
          >
            {collapsed ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronUp className="h-3.5 w-3.5" />}
          </button>
          <button
            type="button"
            onClick={onClear}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label="Clear selected entity"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {!collapsed && (
        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3.5 scrollbar-thin">
          {selectedEdge && (
            <div className="rounded-xl border border-info/30 bg-info/10 p-2.5 text-[10px]">
              <div className="flex items-center justify-between gap-2">
                <span className="font-semibold text-info">Active Relation</span>
                <span className="rounded bg-info/15 px-1.5 py-0.5 font-mono text-info">
                  {selectedEdge.relation_type}
                </span>
              </div>
              <div className="mt-1 font-mono text-[10px] text-foreground">
                {selectedEdge.from_name} → {selectedEdge.to_name}
              </div>
              <div className="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[9px] text-muted-foreground">
                <span>Confidence: {formatConfidence(selectedEdgeSemantics?.confidence ?? null)}</span>
                <span>
                  Provenance: {formatProvenance(selectedEdgeSemantics?.provenance ?? 'not_recorded')}
                </span>
              </div>
            </div>
          )}

          {node && (
            <>
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={onToggleNeighborhood}
                  className={cn(
                    'flex-1 rounded-xl border px-2.5 py-1.5 text-[11px] font-semibold transition',
                    graphMode === 'neighborhood'
                      ? 'border-primary/40 bg-primary/18 text-primary'
                      : 'border-border/70 bg-muted/30 text-foreground hover:border-primary/30 hover:bg-primary/10',
                  )}
                >
                  {graphMode === 'neighborhood' ? 'Exit neighborhood' : 'Focus neighborhood'}
                </button>
                <button
                  type="button"
                  onClick={() => onCenterNode(node.name)}
                  className="inline-flex items-center gap-1 rounded-xl border border-border/70 bg-muted/30 px-2.5 py-1.5 text-[11px] font-semibold text-foreground transition hover:bg-muted"
                  title="Center camera on entity"
                >
                  <Focus className="h-3 w-3 text-primary" />
                  Center
                </button>
              </div>

              <div className="grid grid-cols-3 gap-1.5 text-center">
                <div className="rounded-xl border border-border/50 bg-muted/30 px-2 py-1.5">
                  <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                    Relations
                  </div>
                  <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                    {edges.length}
                  </div>
                </div>
                <div className="rounded-xl border border-border/50 bg-muted/30 px-2 py-1.5">
                  <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                    Neighbors
                  </div>
                  <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                    {neighbors.length}
                  </div>
                </div>
                <div className="rounded-xl border border-border/50 bg-muted/30 px-2 py-1.5">
                  <div className="text-[9px] uppercase tracking-wider text-muted-foreground">
                    Centrality
                  </div>
                  <div className="mt-0.5 font-mono text-sm font-bold text-foreground">
                    {importancePct}%
                  </div>
                </div>
              </div>

              <div className="rounded-xl border border-border/60 bg-background/55 p-2.5">
                <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Observations
                </div>
                <p className="mt-1.5 max-h-28 overflow-y-auto whitespace-pre-wrap break-words text-[11px] leading-relaxed text-foreground/85 scrollbar-thin">
                  {formatObservations(node.observations)}
                </p>
              </div>

              {neighbors.length > 0 && (
                <div>
                  <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                    Linked Entities ({neighbors.length})
                  </div>
                  <div className="flex max-h-24 flex-wrap gap-1 overflow-y-auto scrollbar-thin">
                    {neighbors.map((neighbor) => (
                      <button
                        key={neighbor.name}
                        type="button"
                        onClick={() => onSelectNode(neighbor.name)}
                        className="inline-flex max-w-full items-center gap-1.5 rounded-lg border border-border/60 bg-muted/25 px-2 py-1 text-[10px] font-medium text-foreground transition hover:border-primary/35 hover:bg-primary/10"
                      >
                        <span
                          className="h-1.5 w-1.5 shrink-0 rounded-full"
                          style={{ backgroundColor: getNodeColor(neighbor.entity_type).main }}
                        />
                        <span className="truncate">{neighbor.name}</span>
                      </button>
                    ))}
                  </div>
                </div>
              )}

              <div>
                <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Persisted Relations ({edges.length})
                </div>
                {edges.length === 0 ? (
                  <div className="rounded-xl border border-dashed border-border/60 px-3 py-2 text-[11px] text-muted-foreground">
                    No persisted relationships for this entity.
                  </div>
                ) : (
                  <div className="space-y-1">
                    {edges.slice(0, 20).map((edge) => {
                      const relationKey = edgeKey(edge);
                      const detail = edgeSemantics.get(relationKey);
                      const isSelected = selectedEdgeKey === relationKey;
                      return (
                        <button
                          key={relationKey}
                          type="button"
                          onClick={() => onSelectEdge(relationKey)}
                          className={cn(
                            'w-full rounded-xl border px-2.5 py-2 text-left text-[10px] transition',
                            isSelected
                              ? 'border-info/45 bg-info/12'
                              : 'border-border/55 bg-background/40 hover:border-primary/30 hover:bg-muted/35',
                          )}
                          aria-pressed={isSelected}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="min-w-0 truncate font-medium text-foreground">
                              {edge.from_name} → {edge.to_name}
                            </span>
                            <span className="shrink-0 rounded bg-primary/12 px-1.5 py-0.5 font-mono text-[9px] text-primary">
                              {edge.relation_type}
                            </span>
                          </div>
                          <div className="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[9px] text-muted-foreground">
                            <span>Conf: {formatConfidence(detail?.confidence ?? null)}</span>
                            <span>
                              Prov: {formatProvenance(detail?.provenance ?? 'not_recorded')}
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </aside>
  );
};

export const KnowledgeView: React.FC = () => {
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);

  const graphControlsRef = useRef<ObsidianGraphHandle | null>(null);

  const [controlCollapsed, setControlCollapsed] = useState(false);
  const [nodesCardCollapsed, setNodesCardCollapsed] = useState(false);
  const [inspectorCollapsed, setInspectorCollapsed] = useState(false);
  const [showLegendPopover, setShowLegendPopover] = useState(false);
  const [controlSection, setControlSection] = useState<ControlSection>('filters');
  const [vaultDrawer, setVaultDrawer] = useState<VaultDrawerMode>('none');

  const [copied, setCopied] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [nodeListQuery, setNodeListQuery] = useState('');
  const [nodeSortOrder, setNodeSortOrder] = useState<NodeSortOrder>('degree');
  const [nodeTypeFilter, setNodeTypeFilter] = useState('all');
  const [factTagFilter, setFactTagFilter] = useState('all');
  const [sortMode, setSortMode] = useState<'importance' | 'newest'>('importance');
  const [graphMode, setGraphMode] = useState<GraphMode>('overview');
  const [selectedNodeName, setSelectedNodeName] = useState<string | null>(null);
  const [selectedEdgeKey, setSelectedEdgeKey] = useState<string | null>(null);
  const [communityFilter, setCommunityFilter] = useState('all');
  const [relationTypeFilter, setRelationTypeFilter] = useState('all');
  const [mostConnectedOnly, setMostConnectedOnly] = useState(false);

  const [checkedNodes, setCheckedNodes] = useState<Set<string>>(() => new Set());
  const [isolateChecked, setIsolateChecked] = useState(false);
  const [includeCheckedNeighbors, setIncludeCheckedNeighbors] = useState(true);
  const [display, setDisplay] = useState<GraphDisplayState>(() => ({ ...DEFAULT_GRAPH_DISPLAY }));

  const [visibleGraphStats, setVisibleGraphStats] = useState<GraphStats>({
    loaded: 0,
    visible: 0,
    edges: 0,
    renderedNodes: 0,
    renderedEdges: 0,
  });

  useEffect(() => {
    wsService.requestCognitiveMemory();

    const interval = setInterval(() => {
      wsService.requestCognitiveMemory();
    }, TIMING_MS.memoryPoll);

    return () => clearInterval(interval);
  }, []);

  const handleRefresh = useCallback(() => {
    setIsRefreshing(true);
    wsService.requestCognitiveMemory();
    setTimeout(() => setIsRefreshing(false), 700);
  }, []);

  const handleDisplayChange = useCallback((key: keyof GraphDisplayState, value: boolean) => {
    setDisplay((current) => ({ ...current, [key]: value }));
  }, []);

  const allNodes = useMemo(() => cognitiveStats.nodes || [], [cognitiveStats.nodes]);
  const allEdges = useMemo(() => cognitiveStats.edges || [], [cognitiveStats.edges]);
  const graphSemantics = useMemo(
    () => buildGraphSemantics(allNodes, allEdges),
    [allEdges, allNodes],
  );

  const nodeTypeCounts = useMemo(() => {
    const counts = new Map<string, number>();
    allNodes.forEach((node) => {
      const kind = node.entity_type.trim() || 'unknown';
      counts.set(kind, (counts.get(kind) || 0) + 1);
    });
    return counts;
  }, [allNodes]);

  const nodeTypes = useMemo(
    () => Array.from(nodeTypeCounts.keys()).sort(),
    [nodeTypeCounts],
  );

  const factTags = useMemo(
    () =>
      Array.from(
        new Set(
          (cognitiveStats.facts || []).flatMap((fact) =>
            fact.tags
              .split(',')
              .map((tag) => tag.trim())
              .filter(Boolean),
          ),
        ),
      ).sort(),
    [cognitiveStats.facts],
  );

  const communityOptions = useMemo(
    () =>
      graphSemantics.clusterIds.map((id, index) => ({
        id,
        label: `Constellation ${String(index + 1).padStart(2, '0')}`,
      })),
    [graphSemantics.clusterIds],
  );

  const relationTypes = useMemo(
    () => Array.from(new Set(allEdges.map((edge) => edge.relation_type).filter(Boolean))).sort(),
    [allEdges],
  );

  const degreeThreshold = useMemo(() => {
    const degrees = Array.from(graphSemantics.nodeMetrics.values())
      .map((metric) => metric.degree)
      .sort((left, right) => right - left);
    return degrees.length === 0
      ? 0
      : degrees[Math.min(degrees.length - 1, Math.floor(degrees.length * 0.25))];
  }, [graphSemantics]);

  const topHubNode = useMemo<CognitiveNode | null>(() => {
    return allNodes.reduce<CognitiveNode | null>((best, node) => {
      const deg = graphSemantics.nodeMetrics.get(node.name)?.degree ?? 0;
      const bestDeg = best ? (graphSemantics.nodeMetrics.get(best.name)?.degree ?? 0) : -1;
      return deg > bestDeg ? node : best;
    }, null);
  }, [allNodes, graphSemantics]);

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const matchingEdgeEndpoints = useMemo(() => {
    if (!normalizedSearch) return new Set<string>();
    return new Set(
      allEdges
        .filter((edge) =>
          [edge.from_name, edge.to_name, edge.relation_type]
            .join(' ')
            .toLowerCase()
            .includes(normalizedSearch),
        )
        .flatMap((edge) => [edge.from_name, edge.to_name]),
    );
  }, [allEdges, normalizedSearch]);

  const baseFilteredNodes = useMemo(
    () =>
      allNodes.filter((node) => {
        const metric = graphSemantics.nodeMetrics.get(node.name);
        const matchesType = nodeTypeFilter === 'all' || node.entity_type === nodeTypeFilter;
        const matchesCommunity =
          communityFilter === 'all' || metric?.clusterId === communityFilter;
        const matchesConnected = !mostConnectedOnly || (metric?.degree || 0) >= degreeThreshold;
        const haystack = [node.name, node.entity_type, formatObservations(node.observations)]
          .join(' ')
          .toLowerCase();
        return (
          matchesType &&
          matchesCommunity &&
          matchesConnected &&
          (!normalizedSearch ||
            haystack.includes(normalizedSearch) ||
            matchingEdgeEndpoints.has(node.name))
        );
      }),
    [
      allNodes,
      communityFilter,
      degreeThreshold,
      graphSemantics.nodeMetrics,
      matchingEdgeEndpoints,
      mostConnectedOnly,
      nodeTypeFilter,
      normalizedSearch,
    ],
  );

  const checkedScopeNames = useMemo(() => {
    if (!isolateChecked || checkedNodes.size === 0) return null;
    const allowed = new Set<string>(checkedNodes);
    if (includeCheckedNeighbors) {
      allEdges.forEach((edge) => {
        if (checkedNodes.has(edge.from_name)) allowed.add(edge.to_name);
        if (checkedNodes.has(edge.to_name)) allowed.add(edge.from_name);
      });
    }
    return allowed;
  }, [allEdges, checkedNodes, includeCheckedNeighbors, isolateChecked]);

  const filteredNodes = useMemo(() => {
    if (!checkedScopeNames) return baseFilteredNodes;
    return baseFilteredNodes.filter((node) => checkedScopeNames.has(node.name));
  }, [baseFilteredNodes, checkedScopeNames]);

  const filteredNodeNames = useMemo(
    () => new Set(filteredNodes.map((node) => node.name)),
    [filteredNodes],
  );

  const filteredEdges = useMemo(
    () =>
      allEdges.filter((edge) => {
        const endpointMatch = checkedScopeNames
          ? filteredNodeNames.has(edge.from_name) && filteredNodeNames.has(edge.to_name)
          : filteredNodeNames.has(edge.from_name) || filteredNodeNames.has(edge.to_name);
        const matchesRelationType =
          relationTypeFilter === 'all' || edge.relation_type === relationTypeFilter;
        const haystack = [edge.from_name, edge.to_name, edge.relation_type]
          .join(' ')
          .toLowerCase();
        const relationMatch = !normalizedSearch || haystack.includes(normalizedSearch);
        const nodeMatch =
          !normalizedSearch ||
          filteredNodeNames.has(edge.from_name) ||
          filteredNodeNames.has(edge.to_name);
        return endpointMatch && matchesRelationType && (relationMatch || nodeMatch);
      }),
    [allEdges, checkedScopeNames, filteredNodeNames, normalizedSearch, relationTypeFilter],
  );

  const sortedChecklistNodes = useMemo(() => {
    const query = nodeListQuery.trim().toLowerCase();
    const candidates = baseFilteredNodes.filter((node) => {
      if (!query) return true;
      return [node.name, node.entity_type].join(' ').toLowerCase().includes(query);
    });
    return [...candidates].sort((left, right) => {
      const leftChecked = checkedNodes.has(left.name) ? 1 : 0;
      const rightChecked = checkedNodes.has(right.name) ? 1 : 0;
      if (leftChecked !== rightChecked) return rightChecked - leftChecked;
      if (nodeSortOrder === 'name') return left.name.localeCompare(right.name);
      if (nodeSortOrder === 'type') {
        const typeCmp = left.entity_type.localeCompare(right.entity_type);
        return typeCmp !== 0 ? typeCmp : left.name.localeCompare(right.name);
      }
      const leftDeg = graphSemantics.nodeMetrics.get(left.name)?.degree ?? 0;
      const rightDeg = graphSemantics.nodeMetrics.get(right.name)?.degree ?? 0;
      return rightDeg !== leftDeg ? rightDeg - leftDeg : left.name.localeCompare(right.name);
    });
  }, [baseFilteredNodes, checkedNodes, graphSemantics.nodeMetrics, nodeListQuery, nodeSortOrder]);

  const filteredFacts = useMemo(
    () =>
      (cognitiveStats.facts || [])
        .filter((fact) => {
          const matchesTag =
            factTagFilter === 'all' ||
            fact.tags
              .split(',')
              .map((tag) => tag.trim())
              .includes(factTagFilter);
          const haystack = [fact.text, fact.tags, fact.timestamp, String(fact.importance)]
            .join(' ')
            .toLowerCase();
          return matchesTag && (!normalizedSearch || haystack.includes(normalizedSearch));
        })
        .sort((a, b) =>
          sortMode === 'importance'
            ? b.importance - a.importance
            : new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
        ),
    [cognitiveStats.facts, factTagFilter, normalizedSearch, sortMode],
  );

  const snapshotStats = {
    entitiesCount: filteredNodes.length,
    relationsCount: filteredEdges.length,
    factsCount: filteredFacts.length,
  };

  const totalStats = {
    entitiesCount: cognitiveStats.entitiesCount || allNodes.length,
    relationsCount: cognitiveStats.relationsCount || allEdges.length,
    factsCount: cognitiveStats.factsCount || (cognitiveStats.facts || []).length,
  };

  const graphCoverageLabel = formatGraphStats({
    loaded: totalStats.entitiesCount,
    visible: visibleGraphStats.visible,
    edges: totalStats.relationsCount,
    renderedNodes: visibleGraphStats.renderedNodes,
    renderedEdges: visibleGraphStats.renderedEdges,
  });

  const selectedNode = useMemo(
    () =>
      selectedNodeName ? allNodes.find((node) => node.name === selectedNodeName) || null : null,
    [allNodes, selectedNodeName],
  );

  const selectedNodeEdges = useMemo(
    () =>
      selectedNode
        ? allEdges.filter(
            (edge) => edge.from_name === selectedNode.name || edge.to_name === selectedNode.name,
          )
        : [],
    [allEdges, selectedNode],
  );

  const selectedNeighbors = useMemo(() => {
    if (!selectedNode) return [];
    const neighborNames = new Set(
      selectedNodeEdges
        .flatMap((edge) => [edge.from_name, edge.to_name])
        .filter((name) => name !== selectedNode.name),
    );
    return allNodes.filter((node) => neighborNames.has(node.name));
  }, [allNodes, selectedNode, selectedNodeEdges]);

  const graphNodes = useMemo(
    () =>
      selectedNode && !filteredNodes.some((node) => node.name === selectedNode.name)
        ? [...filteredNodes, selectedNode]
        : filteredNodes,
    [filteredNodes, selectedNode],
  );

  const graphEdges = useMemo(() => {
    const names = new Set(graphNodes.map((node) => node.name));
    return filteredEdges.filter((edge) => names.has(edge.from_name) && names.has(edge.to_name));
  }, [filteredEdges, graphNodes]);

  const selectedEdge = useMemo(
    () =>
      selectedEdgeKey ? allEdges.find((edge) => edgeKey(edge) === selectedEdgeKey) || null : null,
    [allEdges, selectedEdgeKey],
  );

  const handleSelectNode = useCallback((nodeName: string | null) => {
    setSelectedNodeName(nodeName);
    setSelectedEdgeKey(null);
    if (nodeName) {
      setInspectorCollapsed(false);
    }
  }, []);

  const focusNodeOnCanvas = useCallback(
    (nodeName: string) => {
      handleSelectNode(nodeName);
      if (graphMode === 'overview') {
        setGraphMode('all');
      }
      requestAnimationFrame(() => {
        graphControlsRef.current?.focusNodeByName(nodeName);
      });
    },
    [graphMode, handleSelectNode],
  );

  const handleSelectEdge = useCallback(
    (key: string | null) => {
      setSelectedEdgeKey(key);
      if (!key) return;
      const edge = allEdges.find((candidate) => edgeKey(candidate) === key);
      if (edge) {
        setSelectedNodeName(edge.from_name);
        setInspectorCollapsed(false);
      }
    },
    [allEdges],
  );

  const toggleNodeChecked = useCallback((nodeName: string) => {
    setCheckedNodes((previous) => {
      const next = new Set(previous);
      if (next.has(nodeName)) {
        next.delete(nodeName);
      } else {
        next.add(nodeName);
      }
      if (next.size === 0) {
        setIsolateChecked(false);
      }
      return next;
    });
  }, []);

  const checkVisibleNodes = useCallback(() => {
    setCheckedNodes((previous) => {
      const next = new Set(previous);
      sortedChecklistNodes.forEach((node) => next.add(node.name));
      return next;
    });
  }, [sortedChecklistNodes]);

  const clearCheckedNodes = useCallback(() => {
    setCheckedNodes(new Set());
    setIsolateChecked(false);
  }, []);

  const activeFilterCount =
    (nodeTypeFilter !== 'all' ? 1 : 0) +
    (communityFilter !== 'all' ? 1 : 0) +
    (relationTypeFilter !== 'all' ? 1 : 0) +
    (mostConnectedOnly ? 1 : 0) +
    (isolateChecked && checkedNodes.size > 0 ? 1 : 0);

  const hasAnyFilter = Boolean(searchQuery.trim()) || activeFilterCount > 0;

  const resetAllFilters = useCallback(() => {
    setSearchQuery('');
    setNodeTypeFilter('all');
    setCommunityFilter('all');
    setRelationTypeFilter('all');
    setMostConnectedOnly(false);
    setIsolateChecked(false);
    setFactTagFilter('all');
  }, []);

  const cycleNodeSortOrder = useCallback(() => {
    setNodeSortOrder((current) =>
      current === 'degree' ? 'name' : current === 'name' ? 'type' : 'degree',
    );
  }, []);

  const graphDbExists =
    runtimeInventory?.memory.graphDb.exists ?? Boolean(cognitiveStats.paths?.graphDb);
  const memoryDbExists =
    runtimeInventory?.memory.memoryDb.exists ?? Boolean(cognitiveStats.paths?.memoryDb);
  const syncLabel = allNodes.length > 0 ? 'Live sync' : 'Awaiting records';
  const memoryDbPath =
    cognitiveStats.paths?.memoryDb ||
    runtimeInventory?.paths.memoryDb ||
    'Runtime path unavailable';
  const graphDbPath =
    cognitiveStats.paths?.graphDb || runtimeInventory?.paths.graphDb || 'Runtime path unavailable';

  const generateMarkdownString = useCallback(() => {
    let md = `# Knowledge Graph Memory Snapshot\n\n`;
    md += `Generated at: \`${new Date().toLocaleString()}\`\n`;
    md += `Memory DB: \`${memoryDbPath}\`\n`;
    md += `Graph DB: \`${graphDbPath}\`\n\n`;

    md += `## 1. Node Entities (${snapshotStats.entitiesCount})\n\n`;
    if (filteredNodes.length > 0) {
      filteredNodes.forEach((n) => {
        md += `### [[${n.name}]] (${n.entity_type})\n`;
        try {
          const obs = JSON.parse(n.observations);
          if (Array.isArray(obs) && obs.length > 0) {
            obs.forEach((o) => {
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
      md += `*No long-term facts compiled yet by background curators.*\n\n`;
    }

    return md;
  }, [
    filteredEdges,
    filteredFacts,
    filteredNodes,
    graphDbPath,
    memoryDbPath,
    snapshotStats.entitiesCount,
    snapshotStats.factsCount,
    snapshotStats.relationsCount,
  ]);

  const handleCopyMarkdown = useCallback(() => {
    navigator.clipboard.writeText(generateMarkdownString());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [generateMarkdownString]);

  const handleDownloadMarkdown = useCallback(() => {
    downloadTextFile(
      'openz-knowledge-snapshot.md',
      generateMarkdownString(),
      'text/markdown;charset=utf-8',
    );
  }, [generateMarkdownString]);

  const handleDownloadJson = useCallback(() => {
    const payload = {
      generatedAt: new Date().toISOString(),
      filters: {
        searchQuery,
        nodeTypeFilter,
        communityFilter,
        relationTypeFilter,
        mostConnectedOnly,
        checkedNodes: Array.from(checkedNodes),
        isolateChecked,
        factTagFilter,
        sortMode,
      },
      stats: snapshotStats,
      nodes: filteredNodes,
      edges: filteredEdges,
      facts: filteredFacts,
      workingMemoryKeys: cognitiveStats.workingMemoryKeys,
    };
    downloadTextFile(
      'openz-knowledge-snapshot.json',
      JSON.stringify(payload, null, 2),
      'application/json;charset=utf-8',
    );
  }, [
    checkedNodes,
    cognitiveStats.workingMemoryKeys,
    communityFilter,
    factTagFilter,
    filteredEdges,
    filteredFacts,
    filteredNodes,
    isolateChecked,
    mostConnectedOnly,
    nodeTypeFilter,
    relationTypeFilter,
    searchQuery,
    snapshotStats,
    sortMode,
  ]);

  return (
    <div className="relative h-full w-full overflow-hidden bg-background select-none">
      {/* Full-Viewport Obsidian Interactive Graph Canvas */}
      <ObsidianGraph
        nodes={graphNodes}
        edges={graphEdges}
        facts={filteredFacts}
        mode={graphMode}
        searchQuery={searchQuery}
        selectedNodeName={selectedNodeName}
        selectedEdgeKey={selectedEdgeKey}
        checkedNodeNames={checkedNodes}
        display={display}
        onDisplayChange={handleDisplayChange}
        onSelectNode={handleSelectNode}
        onSelectEdge={handleSelectEdge}
        onModeChange={setGraphMode}
        onVisibleStatsChange={setVisibleGraphStats}
        controlsRef={graphControlsRef}
        fullBleed
        showBuiltInChrome={false}
        height="100%"
      />

      {/* Top-Left Floating Card: Search, Mode, Filters, Display & Vault Controls */}
      <section
        aria-label="Graph search and controls"
        className="pointer-events-auto absolute left-14 top-3 z-20 flex max-h-[calc(55vh-0.75rem)] w-[min(340px,calc(100vw-5.25rem))] flex-col overflow-hidden rounded-2xl border border-border/75 bg-card/88 shadow-2xl backdrop-blur-md md:left-3"
      >
        <div className="flex items-center gap-2 border-b border-border/60 px-3 py-2.5">
          <button
            type="button"
            onClick={() => setActiveView('chats')}
            className="rounded-lg border border-border/60 bg-muted/30 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Back to chat"
            aria-label="Back to chat"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
          </button>

          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <BrainCircuit className="h-4 w-4 shrink-0 text-primary" />
            <h1 className="truncate text-xs font-bold tracking-tight text-foreground">
              Cognitive Graph
            </h1>
            <span
              className={cn(
                'h-2 w-2 shrink-0 rounded-full',
                memoryDbExists && graphDbExists ? 'bg-success' : 'bg-warning',
              )}
              title={`${memoryDbExists && graphDbExists ? 'Stores ready' : 'Partial store'} · ${syncLabel}`}
            />
          </div>

          <span className="rounded-full bg-muted/70 px-2 py-0.5 font-mono text-[10px] text-muted-foreground">
            {graphNodes.length}/{totalStats.entitiesCount}
          </span>

          <button
            type="button"
            onClick={handleRefresh}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Sync cognitive memory"
            aria-label="Sync cognitive memory"
          >
            <RefreshCw className={cn('h-3.5 w-3.5', isRefreshing && 'animate-spin text-primary')} />
          </button>

          <button
            type="button"
            onClick={() => setControlCollapsed((prev) => !prev)}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label={controlCollapsed ? 'Expand graph controls' : 'Collapse graph controls'}
          >
            {controlCollapsed ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronUp className="h-3.5 w-3.5" />
            )}
          </button>
        </div>

        {!controlCollapsed && (
          <div className="min-h-0 flex-1 space-y-2.5 overflow-y-auto p-3 scrollbar-thin">
            {/* Search input */}
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search entities, relations, facts..."
                aria-label="Search cognitive graph"
                className="h-8 w-full rounded-xl border border-border/70 bg-background/75 pl-8 pr-7 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => setSearchQuery('')}
                  className="absolute right-2 top-1.5 rounded p-0.5 text-muted-foreground hover:text-foreground"
                  aria-label="Clear search query"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>

            {/* Graph Lens / View Mode */}
            <div className="grid grid-cols-3 gap-1 rounded-xl border border-border/60 bg-muted/25 p-1 text-[10px]">
              {(['overview', 'all', 'neighborhood'] as const).map((candidate) => {
                const active = graphMode === candidate;
                const label =
                  candidate === 'overview'
                    ? 'Clusters'
                    : candidate === 'all'
                      ? 'All Nodes'
                      : 'Local';
                return (
                  <button
                    key={candidate}
                    type="button"
                    onClick={() => {
                      if (candidate === 'neighborhood' && !selectedNodeName && topHubNode) {
                        setSelectedNodeName(topHubNode.name);
                      }
                      setGraphMode(candidate);
                    }}
                    className={cn(
                      'rounded-lg px-2 py-1 font-semibold transition',
                      active
                        ? 'bg-primary text-primary-foreground shadow-xs'
                        : 'text-muted-foreground hover:text-foreground',
                    )}
                    aria-pressed={active}
                  >
                    {label}
                  </button>
                );
              })}
            </div>

            {/* Control Card Sub-Tabs: Filters | Display | Vault */}
            <div className="flex items-center gap-1 border-b border-border/50 pb-2 text-[10px]">
              <button
                type="button"
                onClick={() => setControlSection('filters')}
                className={cn(
                  'inline-flex items-center gap-1 rounded-lg px-2.5 py-1 font-semibold transition',
                  controlSection === 'filters'
                    ? 'bg-primary/14 text-primary'
                    : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                )}
              >
                <Filter className="h-3 w-3" />
                Filters
                {activeFilterCount > 0 && (
                  <span className="rounded-full bg-primary px-1.5 text-[9px] text-primary-foreground">
                    {activeFilterCount}
                  </span>
                )}
              </button>
              <button
                type="button"
                onClick={() => setControlSection('display')}
                className={cn(
                  'inline-flex items-center gap-1 rounded-lg px-2.5 py-1 font-semibold transition',
                  controlSection === 'display'
                    ? 'bg-primary/14 text-primary'
                    : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                )}
              >
                <SlidersHorizontal className="h-3 w-3" />
                Display
              </button>
              <button
                type="button"
                onClick={() => setControlSection('vault')}
                className={cn(
                  'inline-flex items-center gap-1 rounded-lg px-2.5 py-1 font-semibold transition',
                  controlSection === 'vault'
                    ? 'bg-primary/14 text-primary'
                    : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                )}
              >
                <Database className="h-3 w-3" />
                Vault
              </button>

              {hasAnyFilter && (
                <button
                  type="button"
                  onClick={resetAllFilters}
                  className="ml-auto rounded-lg px-2 py-1 font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                >
                  Reset
                </button>
              )}
            </div>

            {controlSection === 'filters' && (
              <div className="space-y-2 text-xs">
                <div className="grid grid-cols-1 gap-1.5">
                  <select
                    value={nodeTypeFilter}
                    onChange={(e) => setNodeTypeFilter(e.target.value)}
                    aria-label="Filter by entity type"
                    className="h-8 w-full rounded-xl border border-border/65 bg-background/70 px-2.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                  >
                    <option value="all">All entity types ({nodeTypes.length})</option>
                    {nodeTypes.map((type) => (
                      <option key={type} value={type}>
                        {type} ({nodeTypeCounts.get(type) || 0})
                      </option>
                    ))}
                  </select>

                  <div className="grid grid-cols-2 gap-1.5">
                    <select
                      value={communityFilter}
                      onChange={(e) => setCommunityFilter(e.target.value)}
                      aria-label="Filter by constellation"
                      className="h-8 w-full rounded-xl border border-border/65 bg-background/70 px-2 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                    >
                      <option value="all">All clusters ({communityOptions.length})</option>
                      {communityOptions.map((community) => (
                        <option key={community.id} value={community.id}>
                          {community.label}
                        </option>
                      ))}
                    </select>

                    <select
                      value={relationTypeFilter}
                      onChange={(e) => setRelationTypeFilter(e.target.value)}
                      aria-label="Filter by relation type"
                      className="h-8 w-full rounded-xl border border-border/65 bg-background/70 px-2 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                    >
                      <option value="all">All relations ({relationTypes.length})</option>
                      {relationTypes.map((relation) => (
                        <option key={relation} value={relation}>
                          {relation}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-1.5 pt-0.5 text-[10px]">
                  <button
                    type="button"
                    onClick={() => setMostConnectedOnly((prev) => !prev)}
                    className={cn(
                      'inline-flex items-center gap-1 rounded-lg border px-2 py-1 font-semibold transition',
                      mostConnectedOnly
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/60 bg-muted/20 text-muted-foreground hover:text-foreground',
                    )}
                    aria-pressed={mostConnectedOnly}
                  >
                    <Sparkles className="h-3 w-3" />
                    Hubs only
                  </button>

                  <button
                    type="button"
                    disabled={checkedNodes.size === 0}
                    onClick={() => setIsolateChecked((prev) => !prev)}
                    className={cn(
                      'inline-flex items-center gap-1 rounded-lg border px-2 py-1 font-semibold transition disabled:cursor-not-allowed disabled:opacity-45',
                      isolateChecked && checkedNodes.size > 0
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/60 bg-muted/20 text-muted-foreground hover:text-foreground',
                    )}
                    aria-pressed={isolateChecked && checkedNodes.size > 0}
                  >
                    <CheckSquare className="h-3 w-3" />
                    Checked ({checkedNodes.size})
                  </button>

                  {isolateChecked && checkedNodes.size > 0 && (
                    <button
                      type="button"
                      onClick={() => setIncludeCheckedNeighbors((prev) => !prev)}
                      className={cn(
                        'rounded-lg border px-2 py-1 font-semibold transition',
                        includeCheckedNeighbors
                          ? 'border-info/40 bg-info/12 text-info'
                          : 'border-border/60 bg-muted/20 text-muted-foreground hover:text-foreground',
                      )}
                      title="Include direct 1-hop neighbors around checked nodes"
                    >
                      + 1-hop
                    </button>
                  )}
                </div>
              </div>
            )}

            {controlSection === 'display' && (
              <div className="space-y-1.5">
                {DISPLAY_OPTIONS.map((item) => (
                  <label
                    key={item.key}
                    className="flex cursor-pointer items-center justify-between gap-2 rounded-xl border border-border/45 bg-background/45 px-2.5 py-1.5"
                  >
                    <span className="min-w-0">
                      <span className="block text-[11px] font-medium text-foreground">
                        {item.label}
                      </span>
                      <span className="block truncate text-[9px] text-muted-foreground">
                        {item.hint}
                      </span>
                    </span>
                    <Switch
                      checked={display[item.key]}
                      onCheckedChange={(checked) => handleDisplayChange(item.key, checked)}
                      aria-label={item.label}
                    />
                  </label>
                ))}
              </div>
            )}

            {controlSection === 'vault' && (
              <div className="space-y-2 text-[11px]">
                <div className="grid grid-cols-3 gap-1.5 text-center">
                  <div className="rounded-xl border border-border/55 bg-background/50 p-2">
                    <div className="font-mono text-xs font-bold text-foreground">
                      {totalStats.entitiesCount}
                    </div>
                    <div className="text-[9px] text-muted-foreground">Entities</div>
                  </div>
                  <div className="rounded-xl border border-border/55 bg-background/50 p-2">
                    <div className="font-mono text-xs font-bold text-foreground">
                      {totalStats.relationsCount}
                    </div>
                    <div className="text-[9px] text-muted-foreground">Relations</div>
                  </div>
                  <div className="rounded-xl border border-border/55 bg-background/50 p-2">
                    <div className="font-mono text-xs font-bold text-foreground">
                      {totalStats.factsCount}
                    </div>
                    <div className="text-[9px] text-muted-foreground">Facts</div>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-1.5">
                  <button
                    type="button"
                    onClick={() =>
                      setVaultDrawer((curr) => (curr === 'facts' ? 'none' : 'facts'))
                    }
                    className={cn(
                      'flex items-center justify-center gap-1.5 rounded-xl border px-2.5 py-2 text-[11px] font-semibold transition',
                      vaultDrawer === 'facts'
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/65 bg-background/60 text-foreground hover:bg-muted/50',
                    )}
                  >
                    <Database className="h-3.5 w-3.5 text-primary" />
                    Facts ({filteredFacts.length})
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setVaultDrawer((curr) => (curr === 'markdown' ? 'none' : 'markdown'))
                    }
                    className={cn(
                      'flex items-center justify-center gap-1.5 rounded-xl border px-2.5 py-2 text-[11px] font-semibold transition',
                      vaultDrawer === 'markdown'
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/65 bg-background/60 text-foreground hover:bg-muted/50',
                    )}
                  >
                    <FileCode className="h-3.5 w-3.5 text-primary" />
                    Snapshot .md
                  </button>
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={handleDownloadMarkdown}
                    className="flex flex-1 items-center justify-center gap-1 rounded-xl border border-border/65 bg-muted/25 px-2.5 py-1.5 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                  >
                    <Download className="h-3 w-3" /> Export .md
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadJson}
                    className="flex flex-1 items-center justify-center gap-1 rounded-xl border border-border/65 bg-muted/25 px-2.5 py-1.5 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                  >
                    <Download className="h-3 w-3" /> Export .json
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setIsMemoryOpen(true);
                      wsService.requestCognitiveMemory();
                    }}
                    className="inline-flex items-center justify-center rounded-xl border border-border/65 bg-muted/25 p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                    title="Inspect database paths & working memory scope"
                    aria-label="Inspect database paths and working memory scope"
                  >
                    <ExternalLink className="h-3.5 w-3.5" />
                  </button>
                </div>

                {cognitiveStats.workingMemoryKeys.length > 0 && (
                  <div className="rounded-xl border border-border/55 bg-background/45 p-2">
                    <div className="flex items-center gap-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                      <KeyRound className="h-3 w-3 text-primary" />
                      Working Scope ({cognitiveStats.workingMemoryKeys.length})
                    </div>
                    <div className="mt-1.5 flex max-h-16 flex-wrap gap-1 overflow-y-auto scrollbar-thin">
                      {cognitiveStats.workingMemoryKeys.map((key) => (
                        <span
                          key={key}
                          className="rounded border border-border/60 bg-muted/35 px-1.5 py-0.5 font-mono text-[9px] text-foreground"
                        >
                          {key}
                        </span>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </section>

      {/* Bottom-Left Floating Card: Check, Highlight & Browse Nodes */}
      <section
        aria-label="Check and browse graph nodes"
        className="pointer-events-auto absolute bottom-3 left-3 z-20 flex max-h-[calc(42vh-0.75rem)] w-[min(340px,calc(100vw-4.5rem))] flex-col overflow-hidden rounded-2xl border border-border/75 bg-card/88 shadow-2xl backdrop-blur-md"
      >
        <div className="flex items-center gap-2 border-b border-border/60 px-3 py-2">
          <CheckSquare className="h-3.5 w-3.5 shrink-0 text-primary" />
          <h2 className="text-xs font-bold text-foreground">Nodes</h2>
          <span className="rounded-full bg-muted/70 px-2 py-0.5 font-mono text-[10px] text-muted-foreground">
            {checkedNodes.size > 0
              ? `${checkedNodes.size} checked · ${sortedChecklistNodes.length}`
              : sortedChecklistNodes.length}
          </span>

          <button
            type="button"
            onClick={cycleNodeSortOrder}
            className="ml-auto rounded-lg border border-border/60 bg-muted/25 px-2 py-0.5 font-mono text-[9px] font-semibold uppercase text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Cycle sort order"
          >
            {nodeSortOrder === 'degree' ? 'Hubs' : nodeSortOrder === 'name' ? 'A–Z' : 'Type'}
          </button>

          <button
            type="button"
            onClick={() => setNodesCardCollapsed((prev) => !prev)}
            className="rounded-lg p-1 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label={nodesCardCollapsed ? 'Expand node checklist' : 'Collapse node checklist'}
          >
            {nodesCardCollapsed ? (
              <ChevronUp className="h-3.5 w-3.5" />
            ) : (
              <ChevronDown className="h-3.5 w-3.5" />
            )}
          </button>
        </div>

        {!nodesCardCollapsed && (
          <div className="flex min-h-0 flex-1 flex-col p-2.5">
            <div className="mb-2 flex items-center gap-1.5">
              <div className="relative flex-1">
                <Search className="pointer-events-none absolute left-2 top-1.5 h-3 w-3 text-muted-foreground" />
                <input
                  value={nodeListQuery}
                  onChange={(e) => setNodeListQuery(e.target.value)}
                  placeholder="Filter node checklist..."
                  aria-label="Filter node checklist"
                  className="h-6 w-full rounded-lg border border-border/65 bg-background/70 pl-6 pr-5 text-[11px] text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                />
                {nodeListQuery && (
                  <button
                    type="button"
                    onClick={() => setNodeListQuery('')}
                    className="absolute right-1.5 top-1 text-muted-foreground hover:text-foreground"
                    aria-label="Clear checklist filter"
                  >
                    <X className="h-3 w-3" />
                  </button>
                )}
              </div>

              <button
                type="button"
                onClick={checkVisibleNodes}
                disabled={sortedChecklistNodes.length === 0}
                className="rounded-lg border border-border/60 bg-muted/25 px-2 py-1 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:opacity-40"
                title="Check all listed nodes"
              >
                All
              </button>

              {checkedNodes.size > 0 && (
                <>
                  <button
                    type="button"
                    onClick={() => setIsolateChecked((prev) => !prev)}
                    className={cn(
                      'rounded-lg border px-2 py-1 text-[10px] font-semibold transition',
                      isolateChecked
                        ? 'border-primary/40 bg-primary/15 text-primary'
                        : 'border-border/60 bg-muted/25 text-muted-foreground hover:text-foreground',
                    )}
                    title="Show only checked nodes on graph"
                  >
                    Isolate
                  </button>
                  <button
                    type="button"
                    onClick={clearCheckedNodes}
                    className="rounded-lg border border-border/60 bg-muted/25 px-2 py-1 text-[10px] font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                    title="Uncheck all nodes"
                  >
                    Clear
                  </button>
                </>
              )}
            </div>

            {sortedChecklistNodes.length === 0 ? (
              <div className="py-5 text-center text-[11px] text-muted-foreground">
                {allNodes.length === 0 ? 'No entities stored yet.' : 'No matching entities.'}
              </div>
            ) : (
              <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto pr-0.5 scrollbar-thin">
                {sortedChecklistNodes.slice(0, 150).map((node) => {
                  const isChecked = checkedNodes.has(node.name);
                  const isSelected = selectedNodeName === node.name;
                  const degree = graphSemantics.nodeMetrics.get(node.name)?.degree ?? 0;
                  const dotColor = getNodeColor(node.entity_type).main;

                  return (
                    <div
                      key={node.name}
                      className={cn(
                        'flex items-center gap-1.5 rounded-xl px-2 py-1 text-[11px] transition',
                        isSelected
                          ? 'bg-primary/14 text-foreground ring-1 ring-primary/35'
                          : isChecked
                            ? 'bg-muted/45 text-foreground'
                            : 'text-foreground/85 hover:bg-muted/35',
                      )}
                    >
                      <button
                        type="button"
                        role="checkbox"
                        aria-checked={isChecked}
                        aria-label={`Check ${node.name}`}
                        onClick={() => toggleNodeChecked(node.name)}
                        className={cn(
                          'flex h-4 w-4 shrink-0 items-center justify-center rounded border transition',
                          isChecked
                            ? 'border-primary bg-primary text-primary-foreground'
                            : 'border-border/80 bg-background/60 text-transparent hover:border-primary/60',
                        )}
                      >
                        <Check className="h-2.5 w-2.5" />
                      </button>

                      <button
                        type="button"
                        onClick={() => focusNodeOnCanvas(node.name)}
                        className="flex min-w-0 flex-1 items-center gap-2 text-left"
                      >
                        <span
                          className="h-2 w-2 shrink-0 rounded-full"
                          style={{ backgroundColor: dotColor }}
                        />
                        <span className="min-w-0 flex-1 truncate font-medium">{node.name}</span>
                        <span className="shrink-0 truncate rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground">
                          {node.entity_type || 'entity'} · {degree}
                        </span>
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </section>

      {/* Top-Right Floating Card: Entity & Relation Inspector (offset left of WorkspaceQuickActions rail) */}
      <div className="pointer-events-none absolute right-14 top-3 z-20 flex w-[min(350px,calc(100vw-4.75rem))] justify-end md:right-16">
        <FloatingEntityInspector
          node={selectedNode}
          nodeMetric={
            selectedNode ? graphSemantics.nodeMetrics.get(selectedNode.name) : undefined
          }
          edges={selectedNodeEdges}
          neighbors={selectedNeighbors}
          edgeSemantics={graphSemantics.edgeMetrics}
          selectedEdge={selectedEdge}
          selectedEdgeKey={selectedEdgeKey}
          isChecked={selectedNode ? checkedNodes.has(selectedNode.name) : false}
          graphMode={graphMode}
          topHubNode={topHubNode}
          collapsed={inspectorCollapsed}
          onToggleCollapse={() => setInspectorCollapsed((prev) => !prev)}
          onToggleChecked={toggleNodeChecked}
          onSelectNode={focusNodeOnCanvas}
          onSelectEdge={handleSelectEdge}
          onToggleNeighborhood={() =>
            setGraphMode((curr) => (curr === 'neighborhood' ? 'all' : 'neighborhood'))
          }
          onCenterNode={focusNodeOnCanvas}
          onClear={() => {
            setSelectedNodeName(null);
            setSelectedEdgeKey(null);
          }}
        />
      </div>

      {/* Bottom-Right Floating Dock: Dynamic Entity Legend + Camera Controls */}
      <div className="pointer-events-none absolute bottom-3 right-14 z-20 flex flex-col items-end gap-2 md:right-16">
        {showLegendPopover && nodeTypes.length > 0 && (
          <div className="pointer-events-auto max-h-56 w-60 overflow-y-auto rounded-2xl border border-border/75 bg-card/92 p-3 shadow-2xl backdrop-blur-md scrollbar-thin">
            <div className="mb-2 flex items-center justify-between text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              <span>Entity Legend ({nodeTypes.length})</span>
              {nodeTypeFilter !== 'all' && (
                <button
                  type="button"
                  onClick={() => setNodeTypeFilter('all')}
                  className="font-mono text-[9px] text-primary hover:underline"
                >
                  All
                </button>
              )}
            </div>
            <div className="space-y-1">
              {nodeTypes.map((type) => {
                const active = nodeTypeFilter === type;
                return (
                  <button
                    key={type}
                    type="button"
                    onClick={() => setNodeTypeFilter(active ? 'all' : type)}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-lg px-2 py-1 text-left text-[11px] transition',
                      active
                        ? 'bg-primary/15 font-semibold text-primary'
                        : 'text-foreground/85 hover:bg-muted/50',
                    )}
                  >
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: getNodeColor(type).main }}
                    />
                    <span className="min-w-0 flex-1 truncate">{type}</span>
                    <span className="font-mono text-[10px] text-muted-foreground">
                      {nodeTypeCounts.get(type) || 0}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <div className="pointer-events-auto flex items-center gap-1 rounded-2xl border border-border/75 bg-card/88 px-2 py-1.5 shadow-xl backdrop-blur-md">
          <span className="hidden max-w-[240px] truncate px-1.5 font-mono text-[10px] text-muted-foreground lg:inline">
            {graphCoverageLabel}
          </span>

          {nodeTypes.length > 0 && (
            <button
              type="button"
              onClick={() => setShowLegendPopover((prev) => !prev)}
              className={cn(
                'inline-flex items-center gap-1 rounded-xl px-2 py-1 text-[10px] font-semibold transition',
                showLegendPopover || nodeTypeFilter !== 'all'
                  ? 'bg-primary/15 text-primary'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground',
              )}
              title="Toggle dynamic entity legend"
            >
              <Layers className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">Types</span>
            </button>
          )}

          <div className="mx-0.5 h-4 w-px bg-border/60" />

          <button
            type="button"
            onClick={() => graphControlsRef.current?.zoomIn()}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Zoom in (+)"
            aria-label="Zoom in"
          >
            <ZoomIn className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.zoomOut()}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Zoom out (-)"
            aria-label="Zoom out"
          >
            <ZoomOut className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.fitToView()}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Fit graph to view (0)"
            aria-label="Fit graph to view"
          >
            <Maximize2 className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => graphControlsRef.current?.resetLayout()}
            className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
            title="Unpin & reset layout"
            aria-label="Reset graph layout"
          >
            <RotateCcw className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {/* Floating Slide-Over Vault Drawer (Stored Facts or Markdown Snapshot) */}
      {vaultDrawer !== 'none' && (
        <aside
          aria-label={vaultDrawer === 'facts' ? 'Stored cognitive facts' : 'Markdown graph snapshot'}
          className="pointer-events-auto absolute bottom-3 right-14 top-3 z-30 flex w-[min(480px,calc(100vw-4.5rem))] flex-col overflow-hidden rounded-2xl border border-border/80 bg-card/95 shadow-2xl backdrop-blur-lg md:right-16"
        >
          <div className="flex items-center gap-2 border-b border-border/60 px-4 py-3">
            {vaultDrawer === 'facts' ? (
              <Database className="h-4 w-4 text-primary" />
            ) : (
              <FileCode className="h-4 w-4 text-primary" />
            )}
            <h2 className="text-xs font-bold text-foreground">
              {vaultDrawer === 'facts'
                ? `Stored Cognitive Facts (${filteredFacts.length})`
                : 'Knowledge Graph Markdown Snapshot'}
            </h2>

            <div className="ml-auto flex items-center gap-1.5">
              {vaultDrawer === 'markdown' && (
                <>
                  <button
                    type="button"
                    onClick={handleCopyMarkdown}
                    className="inline-flex items-center gap-1 rounded-lg border border-border/70 bg-muted/30 px-2.5 py-1 text-[10px] font-semibold text-foreground transition hover:bg-muted"
                  >
                    {copied ? (
                      <>
                        <Check className="h-3 w-3 text-success" /> Copied
                      </>
                    ) : (
                      <>
                        <Copy className="h-3 w-3" /> Copy
                      </>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadMarkdown}
                    className="inline-flex items-center gap-1 rounded-lg border border-border/70 bg-muted/30 px-2.5 py-1 text-[10px] font-semibold text-foreground transition hover:bg-muted"
                  >
                    <Download className="h-3 w-3" /> .md
                  </button>
                </>
              )}
              <button
                type="button"
                onClick={() => setVaultDrawer('none')}
                className="rounded-lg p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground"
                aria-label="Close vault drawer"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>

          {vaultDrawer === 'facts' ? (
            <div className="flex min-h-0 flex-1 flex-col p-3.5">
              <div className="mb-3 grid grid-cols-2 gap-2">
                <select
                  value={factTagFilter}
                  onChange={(e) => setFactTagFilter(e.target.value)}
                  aria-label="Filter facts by tag"
                  className="h-8 rounded-xl border border-border/65 bg-background/75 px-2.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                >
                  <option value="all">All tags ({factTags.length})</option>
                  {factTags.map((tag) => (
                    <option key={tag} value={tag}>
                      {tag}
                    </option>
                  ))}
                </select>
                <select
                  value={sortMode}
                  onChange={(e) => setSortMode(e.target.value as 'importance' | 'newest')}
                  aria-label="Sort stored facts"
                  className="h-8 rounded-xl border border-border/65 bg-background/75 px-2.5 text-[11px] text-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                >
                  <option value="importance">Sort: Importance</option>
                  <option value="newest">Sort: Newest</option>
                </select>
              </div>

              <div className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-1 scrollbar-thin">
                {filteredFacts.length === 0 ? (
                  <div className="rounded-2xl border border-warning/25 bg-warning/8 p-4 text-xs leading-relaxed">
                    <div className="flex items-start gap-2.5">
                      <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-warning" />
                      <div className="space-y-1.5">
                        <div className="font-semibold text-warning">No stored facts matched</div>
                        <p className="text-[11px] text-muted-foreground">
                          OpenZ compiles conversation learnings asynchronously into{' '}
                          <code className="font-mono">{memoryDbPath}</code> or via native tools like{' '}
                          <code className="font-mono">extract_and_store_facts</code> and{' '}
                          <code className="font-mono">smart_store</code>.
                        </p>
                      </div>
                    </div>
                  </div>
                ) : (
                  filteredFacts.map((fact) => (
                    <div
                      key={`${fact.timestamp}-${fact.text}`}
                      className="rounded-xl border border-border/70 bg-background/60 p-3 transition hover:border-primary/35"
                    >
                      <div className="mb-1.5 flex flex-wrap items-center gap-1.5">
                        <span className="rounded border border-primary/25 bg-primary/12 px-1.5 py-0.5 font-mono text-[9px] font-semibold text-primary">
                          Importance {fact.importance.toFixed(2)}
                        </span>
                        {fact.tags && (
                          <span className="max-w-[200px] truncate rounded bg-muted px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground">
                            {fact.tags}
                          </span>
                        )}
                        <span className="ml-auto font-mono text-[9px] text-muted-foreground">
                          {fact.timestamp}
                        </span>
                      </div>
                      <p className="break-words font-sans text-xs leading-relaxed text-foreground">
                        {fact.text}
                      </p>
                    </div>
                  ))
                )}
              </div>
            </div>
          ) : (
            <div className="min-h-0 flex-1 overflow-y-auto p-4 font-mono text-xs prose prose-xs dark:prose-invert max-w-none text-foreground scrollbar-thin">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{generateMarkdownString()}</ReactMarkdown>
            </div>
          )}
        </aside>
      )}
    </div>
  );
};
