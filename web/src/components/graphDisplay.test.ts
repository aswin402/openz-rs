import { expect, test } from 'bun:test';
import { DEFAULT_GRAPH_DISPLAY } from './ObsidianGraph';

test('constellation display defaults keep overlays quiet and the atlas visible', () => {
  expect(DEFAULT_GRAPH_DISPLAY.showGrid).toBe(false);
  expect(DEFAULT_GRAPH_DISPLAY.showOrbits).toBe(false);
  expect(DEFAULT_GRAPH_DISPLAY.showLabels).toBe(true);
  expect(DEFAULT_GRAPH_DISPLAY.showGlow).toBe(true);
  expect(DEFAULT_GRAPH_DISPLAY.showSpaceField).toBe(true);
});
