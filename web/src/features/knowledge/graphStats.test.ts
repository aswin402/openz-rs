import { expect, test } from 'bun:test';
import { formatGraphStats } from './graphStats';

test('formats authoritative and rendered counts separately', () => {
  expect(formatGraphStats({ loaded: 1000, visible: 986, edges: 7200, renderedNodes: 986, renderedEdges: 1800 }))
    .toBe('1,000 loaded · 986 visible · 7,200 relations · 1,800 rendered');
});
