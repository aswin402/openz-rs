import type { GraphCluster } from '../graphLayout';
import { distanceToSegment, type GraphEdge, type RenderNode } from './graphCanvas';

export interface GraphPoint {
  x: number;
  y: number;
}

export function findNodeAtPoint(
  nodes: readonly RenderNode[],
  point: GraphPoint,
  zoom: number,
  hitPadding = 10,
): RenderNode | null {
  const padding = hitPadding / zoom;
  for (let index = nodes.length - 1; index >= 0; index -= 1) {
    const node = nodes[index];
    const dx = node.x - point.x;
    const dy = node.y - point.y;
    const radius = node.radius + padding;
    if (dx * dx + dy * dy <= radius * radius) return node;
  }
  return null;
}

export function findEdgeAtPoint(edges: readonly GraphEdge[], point: GraphPoint, zoom: number): GraphEdge | null {
  const threshold = Math.max(6, 13 / zoom);
  let closest: GraphEdge | null = null;
  let closestDistance = threshold;
  edges.forEach((edge) => {
    const distance = distanceToSegment(point.x, point.y, edge.source.x, edge.source.y, edge.target.x, edge.target.y);
    if (distance < closestDistance) {
      closest = edge;
      closestDistance = distance;
    }
  });
  return closest;
}

export function findClusterAtPoint(
  clusters: readonly GraphCluster[],
  point: GraphPoint,
): GraphCluster | null {
  let closest: GraphCluster | null = null;
  let closestDistance = Infinity;
  clusters.forEach((cluster) => {
    const dx = cluster.x - point.x;
    const dy = cluster.y - point.y;
    const distance = Math.sqrt(dx * dx + dy * dy);
    if (distance <= cluster.radius && distance < closestDistance) {
      closest = cluster;
      closestDistance = distance;
    }
  });
  return closest;
}
