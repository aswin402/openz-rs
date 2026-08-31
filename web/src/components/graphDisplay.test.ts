import { expect, test } from 'bun:test';
import { DEFAULT_GRAPH_DISPLAY, findGraphFocusNodeId, resolveSettleFrames } from './ObsidianGraph';

test('constellation display defaults keep overlays quiet and the atlas visible', () => {
  expect(DEFAULT_GRAPH_DISPLAY.showGrid).toBe(false);
  expect(DEFAULT_GRAPH_DISPLAY.showOrbits).toBe(false);
  expect(DEFAULT_GRAPH_DISPLAY.showLabels).toBe(true);
  expect(DEFAULT_GRAPH_DISPLAY.showGlow).toBe(true);
  expect(DEFAULT_GRAPH_DISPLAY.showSpaceField).toBe(true);
});

test('graph focus resolves relation-only searches to the first relation endpoint', () => {
  const nodes = [
    { name: 'Ada', entity_type: 'person', observations: 'researcher' },
    { name: 'OpenZ', entity_type: 'project', observations: 'agent framework' },
  ];
  const edges = [{ from_name: 'Ada', to_name: 'OpenZ', relation_type: 'maintains' }];

  expect(findGraphFocusNodeId(nodes, edges, 'maintains')).toBe('Ada');
});

test('reduced motion disables bounded graph settling', () => {
  expect(resolveSettleFrames(12, true)).toBe(0);
  expect(resolveSettleFrames(12, false)).toBe(12);
});
