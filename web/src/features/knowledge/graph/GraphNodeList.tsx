import React from 'react';
import { List } from 'lucide-react';
import type { RenderNode } from './graphCanvas';

interface GraphNodeListProps {
  nodes: readonly RenderNode[];
  selectedNodeId: string | null;
  onSelectNode: (node: RenderNode) => void;
}

export const GraphNodeList: React.FC<GraphNodeListProps> = ({ nodes, selectedNodeId, onSelectNode }) => {
  const sortedNodes = [...nodes].sort((left, right) => left.name.localeCompare(right.name));

  return (
    <details className="absolute right-3 top-16 z-10 w-64 max-w-[calc(100%-1.5rem)] rounded-xl border border-border/75 bg-card/90 text-[10px] shadow-lg backdrop-blur-md">
      <summary className="flex min-h-10 cursor-pointer list-none items-center gap-2 px-3 py-2 font-semibold text-foreground outline-none transition hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 [&::-webkit-details-marker]:hidden">
        <List className="h-3.5 w-3.5 text-muted-foreground" />
        <span>All entities</span>
        <span className="ml-auto font-mono text-muted-foreground">{nodes.length}</span>
      </summary>
      <div className="max-h-64 overflow-y-auto border-t border-border/60 p-1.5" aria-label="Loaded graph entities">
        {sortedNodes.length === 0 ? (
          <div className="px-2 py-3 text-center text-muted-foreground">No entities loaded.</div>
        ) : (
          sortedNodes.map((node) => (
            <button
              key={node.id}
              type="button"
              onClick={() => onSelectNode(node)}
              aria-current={selectedNodeId === node.id ? 'true' : undefined}
              className="flex w-full min-w-0 items-center gap-2 rounded-lg px-2 py-2 text-left transition hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60"
            >
              <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: node.color }} />
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium text-foreground">{node.name}</span>
                <span className="block truncate text-[9px] text-muted-foreground">{node.type} · {node.degree} relations</span>
              </span>
              {selectedNodeId === node.id && <span className="shrink-0 text-primary">selected</span>}
            </button>
          ))
        )}
      </div>
    </details>
  );
};
