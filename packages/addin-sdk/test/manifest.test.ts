/** Manifest schema (CORE-ADDIN-001): valid manifests pass, each broken rule is reported with its path. */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  REQUIRED_FIELDS,
  isAddinOrigin,
  localize,
  parseManifest,
  ribbonCommands,
  validateManifest,
  type AddinManifest,
} from '../src';

export const SAMPLE: AddinManifest = {
  manifestVersion: 1,
  id: 'vn.example.hello',
  name: { vi: 'Xin chào', en: 'Hello' },
  description: { vi: 'Add-in mẫu', en: 'Sample add-in' },
  version: '1.0.0',
  publisher: 'Example Co.',
  icon: 'https://addins.example.vn/hello/icon.png',
  startUrl: 'https://addins.example.vn/hello/index.html?x=1',
  allowedOrigins: ['https://addins.example.vn'],
  permissions: [
    'read-document',
    'selection',
    'write-document',
    'ui:ribbon',
    'storage',
    'network:https://api.example.vn',
  ],
  apps: ['doc', 'sheet'],
  ribbon: {
    tab: 'home',
    group: { vi: 'Mẫu', en: 'Sample' },
    commands: [
      { id: 'open', label: { vi: 'Mở', en: 'Open' }, action: 'openPane' },
      {
        id: 'greet',
        label: { vi: 'Chào', en: 'Greet' },
        tooltip: { vi: 'Chèn lời chào', en: 'Insert a greeting' },
        action: 'event',
        apps: ['doc'],
      },
    ],
  },
};

const clone = (): Record<string, unknown> =>
  JSON.parse(JSON.stringify(SAMPLE)) as Record<string, unknown>;

const errorsOf = (m: unknown) => {
  const r = validateManifest(m);
  return r.ok ? [] : r.errors;
};
const paths = (m: unknown) => errorsOf(m).map((e) => e.path);

describe('validateManifest', () => {
  it('accepts a full manifest and a minimal one', () => {
    expect(validateManifest(SAMPLE)).toEqual({ ok: true, manifest: SAMPLE });
    const minimal = {
      manifestVersion: 1,
      id: 'vn.example.min',
      name: { vi: 'Tối thiểu', en: 'Minimal' },
      version: '0.1.0-beta.1',
      publisher: 'X',
      startUrl: 'http://localhost:5555/',
      allowedOrigins: ['http://localhost:5555'],
      permissions: [],
      apps: ['slide'],
    };
    expect(validateManifest(minimal).ok).toBe(true);
  });

  it('reports every missing required field', () => {
    expect(paths({})).toEqual([...REQUIRED_FIELDS]);
    expect(paths(null)).toEqual(['']);
    expect(paths([])).toEqual(['']);
  });

  it('rejects unknown properties (typos are not silently ignored)', () => {
    const m = clone();
    m.permision = ['storage'];
    expect(paths(m)).toEqual(['permision']);
    const r = clone();
    (r.ribbon as Record<string, unknown>).tabs = 'home';
    expect(paths(r)).toEqual(['ribbon.tabs']);
  });

  it.each([
    ['id', 'Hello'],
    ['id', 'hello'],
    ['id', 'vn..x'],
    ['version', '1.0'],
    ['version', '01.0.0'],
    ['manifestVersion', 2],
    ['publisher', ''],
    ['publisher', 'a\u0007b'],
    ['name', 'Hello'],
    ['name', { vi: 'Xin chào' }],
    ['apps', []],
    ['apps', ['doc', 'doc']],
    ['apps', ['word']],
    ['permissions', 'storage'],
  ])('rejects %s = %j', (key, value) => {
    const m = clone();
    m[key] = value;
    expect(
      paths(m).some((p) => p === key || p.startsWith(`${key}.`) || p.startsWith(`${key}[`)),
    ).toBe(true);
  });

  it('checks origins and that URLs stay on them', () => {
    const m = clone();
    m.allowedOrigins = ['https://addins.example.vn/path', 'http://addins.example.vn', 'ftp://x'];
    expect(paths(m)).toEqual(
      expect.arrayContaining([
        'allowedOrigins[0]',
        'allowedOrigins[1]',
        'allowedOrigins[2]',
        'startUrl',
        'icon',
      ]),
    );
    const other = clone();
    other.startUrl = 'https://evil.example/hello.html';
    other.icon = 'https://user:pw@addins.example.vn/icon.png';
    expect(paths(other)).toEqual(['startUrl', 'icon']);
    expect(isAddinOrigin('https://a.example:8443')).toBe(true);
    expect(isAddinOrigin('http://127.0.0.1:9000')).toBe(true);
    expect(isAddinOrigin('http://intranet.example')).toBe(false);
    expect(isAddinOrigin('https://A.example')).toBe(false);
  });

  it('checks permissions', () => {
    const m = clone();
    m.permissions = ['storage', 'storage', 'admin', 'network:https://x.example/api', 7];
    expect(paths(m)).toEqual(
      expect.arrayContaining([
        'permissions[1]',
        'permissions[2]',
        'permissions[3]',
        'permissions[4]',
      ]),
    );
  });

  it('ribbon commands need ui:ribbon, unique ids, both labels and apps of the manifest', () => {
    const m = clone();
    m.permissions = ['read-document'];
    expect(paths(m)).toContain('ribbon');
    const r = clone();
    const ribbon = r.ribbon as { commands: Record<string, unknown>[] };
    ribbon.commands[1]!.id = 'open';
    ribbon.commands[0]!.label = { vi: 'Mở' };
    ribbon.commands[0]!.action = 'run';
    ribbon.commands[1]!.apps = ['slide'];
    expect(paths(r)).toEqual(
      expect.arrayContaining([
        'ribbon.commands[1].id',
        'ribbon.commands[0].label.en',
        'ribbon.commands[0].action',
        'ribbon.commands[1].apps',
      ]),
    );
    const many = clone();
    (many.ribbon as { commands: unknown[] }).commands = Array.from({ length: 21 }, (_, i) => ({
      id: `c${i}`,
      label: { vi: 'a', en: 'a' },
      action: 'openPane',
    }));
    expect(paths(many)).toEqual(['ribbon.commands']);
  });

  it('parses JSON text', () => {
    expect(parseManifest(JSON.stringify(SAMPLE)).ok).toBe(true);
    const bad = parseManifest('{');
    expect(bad.ok).toBe(false);
  });

  it('keeps the published JSON schema in step with the validator', () => {
    const schema = JSON.parse(
      readFileSync(join(__dirname, '..', 'manifest.schema.json'), 'utf8'),
    ) as {
      required: string[];
      properties: Record<string, unknown>;
      additionalProperties: boolean;
    };
    expect(schema.required).toEqual([...REQUIRED_FIELDS]);
    expect(schema.additionalProperties).toBe(false);
    // Every top-level key the validator knows is described by the schema.
    for (const k of Object.keys(SAMPLE)) expect(schema.properties).toHaveProperty(k);
  });

  it('localizes and filters ribbon commands per app', () => {
    expect(localize(SAMPLE.name, 'en-US')).toBe('Hello');
    expect(localize(SAMPLE.name, 'vi-VN')).toBe('Xin chào');
    expect(ribbonCommands(SAMPLE, 'doc').map((c) => c.id)).toEqual(['open', 'greet']);
    expect(ribbonCommands(SAMPLE, 'sheet').map((c) => c.id)).toEqual(['open']);
    expect(ribbonCommands({ ...SAMPLE, permissions: [] }, 'doc')).toEqual([]);
  });
});
