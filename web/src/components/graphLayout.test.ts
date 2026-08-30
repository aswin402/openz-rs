import { expect, test } from 'bun:test';
import type { CognitiveEdge, CognitiveNode } from '../types/openz';
import { buildClusters, layoutNodes, prioritizeGraphEdges, selectVisibleGraph } from './graphLayout';

const node = (name: string, type = 'concept'): CognitiveNode => ({ name, entity_type: type, observations: '[]' });
const edge = (from_name: string, to_name: string, relation_type = 'links', confidence?: number): CognitiveEdge => ({
  from_name,
  to_name,
  relation_type,
  ...(confidence === undefined ? {} : { confidence }),
});

test('overview represents every loaded node instead of only cluster representatives', () => {
  const nodes = Array.from({ length: 24 }, (_, index) => node(`entity-${index}`));
  const edges = nodes.slice(1).map((item) => edge('entity-0', item.name, 'relates'));
  const layout = layoutNodes(nodes, edges, 900, 560);
  const result = selectVisibleGraph(layout, edges, buildClusters(nodes, edges, 900, 560), 'overview', 0.45, {
    left: -1000, top: -1000, right: 2000, bottom: 2000,
  });
  expect(result.loadedNodeCount).toBe(24);
  expect(result.nodes).toHaveLength(24);
});

test('edge priority preserves selected neighborhood before low-value edges', () => {
  const nodes = [node('hub'), ...Array.from({ length: 8 }, (_, index) => node(`leaf-${index}`))];
  const edges = [
    ...nodes.slice(1).map((item) => edge('hub', item.name, 'primary', 0.95)),
    edge('leaf-0', 'leaf-1', 'incidental', 0.1),
  ];
  const layout = layoutNodes(nodes, edges, 900, 560);
  const selected = prioritizeGraphEdges(edges, layout, 'hub', 0.4, 3);
  expect(selected).toHaveLength(3);
  expect(selected.every((item) => item.from_name === 'hub' || item.to_name === 'hub')).toBe(true);
});

test('places a connected hub nearer the community center than leaves', () => {
  const nodes = [node('hub'), node('leaf-a'), node('leaf-b'), node('leaf-c')];
  const edges = [edge('hub', 'leaf-a'), edge('hub', 'leaf-b'), edge('hub', 'leaf-c')];
  const clusters = buildClusters(nodes, edges, 900, 560);
  const layout = layoutNodes(nodes, edges, 900, 560);
  const cluster = clusters[0];
  const hub = layout.find((item) => item.id === 'hub')!;
  const leaf = layout.find((item) => item.id === 'leaf-a')!;
  expect(Math.hypot(hub.x - cluster.x, hub.y - cluster.y)).toBeLessThan(Math.hypot(leaf.x - cluster.x, leaf.y - cluster.y));
  const leafDistances = layout
    .filter((item) => item.id !== 'hub')
    .map((item) => Math.hypot(item.x - cluster.x, item.y - cluster.y));
  expect(Math.hypot(hub.x - cluster.x, hub.y - cluster.y)).toBeLessThan(Math.min(...leafDistances));
});

test('constellation placement remains stable when input order changes', () => {
  const nodes = [node('a'), node('b'), node('c')];
  const edges = [edge('a', 'b'), edge('b', 'c')];
  const first = layoutNodes(nodes, edges, 900, 560).map(({ id, x, y }) => ({ id, x, y }));
  const second = layoutNodes([...nodes].reverse(), edges, 900, 560).map(({ id, x, y }) => ({ id, x, y }));
  expect([...first].sort((a, b) => a.id.localeCompare(b.id))).toEqual([...second].sort((a, b) => a.id.localeCompare(b.id)));
});

test('constellation placement stays exact for reordered larger communities', () => {
  const nodes = Array.from({ length: 128 }, (_, index) => node(`node-${index}`));
  const edges = nodes.slice(1).map((item) => edge('node-0', item.name));
  const byName = (left: { id: string }, right: { id: string }) => left.id.localeCompare(right.id);
  const first = layoutNodes(nodes, edges, 913, 577).map(({ id, x, y }) => ({ id, x, y })).sort(byName);
  const second = layoutNodes([...nodes].reverse(), edges, 913, 577).map(({ id, x, y }) => ({ id, x, y })).sort(byName);
  expect(first).toEqual(second);
});
