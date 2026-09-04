import { describe, expect, test } from 'bun:test';
import { buildGraphSemantics, edgeKey, formatConfidence } from './graphSemantics';

describe('graph semantics', () => {
  test('computes degree and bounded importance from real edges', () => {
    const nodes = [
      { name: 'hub', entity_type: 'concept', observations: '[]' },
      { name: 'leaf', entity_type: 'concept', observations: '[]' },
    ];
    const edges = [{ from_name: 'hub', to_name: 'leaf', relation_type: 'uses' }];
    const semantics = buildGraphSemantics(nodes, edges);
    expect(semantics.nodeMetrics.get('hub')?.degree).toBe(1);
    expect(semantics.nodeMetrics.get('hub')?.importance).toBeGreaterThanOrEqual(0);
    expect(semantics.nodeMetrics.get('hub')?.importance).toBeLessThanOrEqual(1);
  });

  test('keeps missing provenance explicit and preserves confidence', () => {
    const edge = { from_name: 'a', to_name: 'b', relation_type: 'links', confidence: 0.7 };
    const semantics = buildGraphSemantics(
      [
        { name: 'a', entity_type: 'concept', observations: '[]' },
        { name: 'b', entity_type: 'concept', observations: '[]' },
      ],
      [edge],
    );
    const detail = semantics.edgeMetrics.get(edgeKey(edge));
    expect(detail?.confidence).toBe(0.7);
    expect(detail?.provenance).toBe('not_recorded');
    expect(formatConfidence(null)).toBe('Not recorded');
  });

  test('rejects malformed endpoints without losing valid records', () => {
    const semantics = buildGraphSemantics(
      [{ name: 'a', entity_type: 'concept', observations: '[]' }],
      [{ from_name: 'a', to_name: '', relation_type: 'broken' }],
    );
    expect(semantics.malformedRecords).toBe(1);
    expect(semantics.edgeMetrics.size).toBe(0);
  });

  test('uses stable cluster IDs and collapses isolated entities by type', () => {
    const nodes = [
      { name: 'connected-a', entity_type: 'concept', observations: '[]' },
      { name: 'connected-b', entity_type: 'concept', observations: '[]' },
      { name: 'isolated-a', entity_type: 'skill', observations: '[]' },
      { name: 'isolated-b', entity_type: 'skill', observations: '[]' },
    ];
    const edges = [{ from_name: 'connected-a', to_name: 'connected-b', relation_type: 'uses' }];
    const first = buildGraphSemantics(nodes, edges);
    const second = buildGraphSemantics([...nodes].reverse(), edges);
    expect(first.clusterIds).toEqual(second.clusterIds);
    expect(first.nodeMetrics.get('isolated-a')?.clusterId).toBe(first.nodeMetrics.get('isolated-b')?.clusterId);
  });
});
