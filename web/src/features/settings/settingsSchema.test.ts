import { describe, expect, it } from 'bun:test';
import {
  buildDefaultsPatch,
  DEFAULT_SECURITY_MODE_OPTIONS,
  hasDefaultsChanged,
  normalizeSecurityMode,
  PERMISSION_TIER_DESCRIPTORS,
  settingsToForm,
} from './settingsSchema';
import type { AgentDefaultsConfig } from '../../types';

const SAMPLE_SETTINGS: AgentDefaultsConfig = {
  model: 'anthropic/claude-3-5-sonnet',
  provider: 'auto',
  theme: 'dark',
  temperature: 0.1,
  max_tokens: 4096,
  streaming: true,
  caveman_mode: true,
  security_mode: 'low',
  workspace: '~/.openz/workspace',
  bot_name: 'OpenZ',
  max_messages: 120,
  max_tool_iterations: 200,
  tool_timeout_secs: 300,
  enable_sandbox: false,
  context_limit: null,
  tool_output_limit: null,
  show_auto_capture_notices: true,
  tui_thought_display: 'off',
  firefox_webdriver_port: 4444,
  firefox_attach_port: 9222,
};

describe('settingsSchema Ask Permission tiers', () => {
  it('normalizes canonical and legacy security_mode strings with low default', () => {
    expect(normalizeSecurityMode('low')).toBe('low');
    expect(normalizeSecurityMode('loose')).toBe('low');
    expect(normalizeSecurityMode('medium')).toBe('medium');
    expect(normalizeSecurityMode('normal')).toBe('medium');
    expect(normalizeSecurityMode('high')).toBe('high');
    expect(normalizeSecurityMode('strict')).toBe('high');
    expect(normalizeSecurityMode('')).toBe('low');
    expect(normalizeSecurityMode(undefined)).toBe('low');
  });

  it('exposes low, medium, and high options and tier descriptors', () => {
    expect(DEFAULT_SECURITY_MODE_OPTIONS.map((opt) => opt.value)).toEqual(['low', 'medium', 'high']);
    expect(PERMISSION_TIER_DESCRIPTORS.map((tier) => tier.value)).toEqual(['low', 'medium', 'high']);
  });

  it('tracks security_mode changes accurately across form patches', () => {
    const form = settingsToForm({ ...SAMPLE_SETTINGS, security_mode: 'loose' });
    expect(form.security_mode).toBe('low');
    expect(hasDefaultsChanged(form, { ...SAMPLE_SETTINGS, security_mode: 'low' })).toBe(false);

    const mediumForm = { ...form, security_mode: 'medium' };
    expect(hasDefaultsChanged(mediumForm, SAMPLE_SETTINGS)).toBe(true);
    expect(buildDefaultsPatch(mediumForm, SAMPLE_SETTINGS)).toEqual({ security_mode: 'medium' });
  });
});
