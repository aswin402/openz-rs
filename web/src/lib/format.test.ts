import { expect, test } from 'bun:test';
import { collapseWhitespace } from './format';

test('collapseWhitespace removes repeated spaces and newlines', () => {
  expect(collapseWhitespace('  one\n\t two   three  ')).toBe('one two three');
});
