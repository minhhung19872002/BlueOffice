// @vitest-environment happy-dom
/**
 * Editor side of add-ins (CORE-ADDIN-001): frame attributes, ribbon placement,
 * and the catalogue / consent / uninstall flow of `useAddins`.
 */
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { AddinManifest } from '@blueoffice/addin-sdk';
import type { AddinCatalog, AddinListing } from '@blueoffice/platform';
import type { RibbonTab } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { frameAttributes } from '../src/frame';
import { mergeAddinTabs } from '../src/ribbon';
import { addinStorage, memoryBacking } from '../src/storage';
import {
  CATALOGUE_PANE,
  MANAGE_COMMAND,
  addinCommandId,
  addinPaneId,
  useAddins,
  type Addins,
} from '../src/useAddins';

afterEach(cleanup);

const hello: AddinManifest = {
  manifestVersion: 1,
  id: 'vn.example.hello',
  name: { vi: 'Xin chào', en: 'Hello' },
  version: '1.0.0',
  publisher: 'Example Co.',
  startUrl: 'https://addins.example.vn/hello/index.html',
  allowedOrigins: ['https://addins.example.vn'],
  permissions: ['selection', 'write-document', 'ui:ribbon', 'network:https://api.example.vn'],
  apps: ['doc', 'sheet'],
  ribbon: {
    tab: 'insert',
    commands: [
      { id: 'open', label: { vi: 'Mở chào', en: 'Open hello' }, action: 'openPane' },
      { id: 'greet', label: { vi: 'Chào', en: 'Greet' }, action: 'event', apps: ['doc'] },
    ],
  },
};

describe('frameAttributes', () => {
  it('sandboxes the frame; only trusted add-ins keep their origin', () => {
    const a = frameAttributes(hello, false, 'https://office.example.vn');
    expect(a.src).toBe(hello.startUrl);
    expect(a.sandbox).toBe('allow-scripts allow-forms');
    expect(a.sandbox).not.toContain('allow-top-navigation');
    expect(a.sandbox).not.toContain('allow-popups');
    expect(frameAttributes(hello, true, '').sandbox).toBe(
      'allow-scripts allow-forms allow-same-origin',
    );
    expect(a.csp).toContain('default-src https://addins.example.vn');
    expect(a.csp).toContain('script-src https://addins.example.vn https://office.example.vn');
    expect(a.csp).toContain('connect-src https://api.example.vn');
    expect(a.csp).toContain("frame-src 'none'");
    expect(a.allow).toContain("camera 'none'");
    const offline = frameAttributes({ ...hello, permissions: [] }, false, 'https://o.example');
    expect(offline.csp).toContain("connect-src 'none'");
  });
});

describe('mergeAddinTabs', () => {
  const tab = (id: string, context?: boolean): RibbonTab => ({
    id,
    label: id,
    groups: [{ id: `${id}-g`, label: id, items: [] }],
    ...(context ? { context: 'table' as const } : {}),
  });
  it('puts groups in the named tab or the Add-in tab, before contextual tabs', () => {
    const out = mergeAddinTabs(
      [tab('home'), tab('insert'), tab('tableDesign', true)],
      [
        { tab: 'insert', group: { id: 'a', label: 'A', items: [] } },
        { tab: 'nosuch', group: { id: 'b', label: 'B', items: [] } },
        { tab: 'tableDesign', group: { id: 'c', label: 'C', items: [] } },
        { group: { id: 'd', label: 'D', items: [] } },
      ],
      tab('addins'),
    );
    expect(out.map((t) => t.id)).toEqual(['home', 'insert', 'addins', 'tableDesign']);
    expect(out[1]!.groups.map((g) => g.id)).toEqual(['insert-g', 'a']);
    expect(out[2]!.groups.map((g) => g.id)).toEqual(['addins-g', 'b', 'c', 'd']);
    expect(out[3]!.groups.map((g) => g.id)).toEqual(['tableDesign-g']);
  });
});

class FakeCatalog implements AddinCatalog {
  installs = new Map<string, string[]>();
  log: string[] = [];
  constructor(
    private manifests: unknown[],
    private on = true,
  ) {}
  async list(app: 'doc' | 'sheet' | 'slide'): Promise<AddinListing[] | null> {
    this.log.push(`list:${app}`);
    if (!this.on) return null;
    return this.manifests.map((manifest) => {
      const id = (manifest as { id: string }).id;
      return {
        manifest,
        trusted: false,
        installed: this.installs.has(id),
        granted: this.installs.get(id) ?? [],
      };
    });
  }
  async install(id: string, permissions: string[]) {
    this.log.push(`install:${id}`);
    this.installs.set(id, permissions);
  }
  async uninstall(id: string) {
    this.log.push(`uninstall:${id}`);
    this.installs.delete(id);
  }
}

class Registry {
  cmds = new Map<string, () => void | Promise<void>>();
  register(c: { id: string; run: () => void | Promise<void> }) {
    if (this.cmds.has(c.id)) throw new Error('dup');
    this.cmds.set(c.id, c.run);
    return () => this.cmds.delete(c.id);
  }
}

function Harness({
  catalog,
  registry,
  backing,
  outRef,
}: {
  catalog: AddinCatalog;
  registry: Registry;
  backing: ReturnType<typeof memoryBacking>;
  outRef: { current: Addins | null };
}) {
  const a = useAddins({
    app: 'doc',
    catalog,
    registry,
    host: () => null,
    selection: () => Promise.reject(new Error('none')),
    selectionKey: 0,
    documentKey: 0,
    backing,
    editorOrigin: 'https://office.example.vn',
  });
  useEffect(() => {
    outRef.current = a;
  });
  return (
    <div>
      {a.panes.map((p) => (
        <section key={p.id} data-pane={p.id}>
          {p.content}
        </section>
      ))}
      {a.element}
    </div>
  );
}

const i18n = createI18n('en-US');

function mount(catalog: AddinCatalog) {
  const registry = new Registry();
  const backing = memoryBacking();
  const out: { current: Addins | null } = { current: null };
  render(
    <I18nextProvider i18n={i18n}>
      <Harness catalog={catalog} registry={registry} backing={backing} outRef={out} />
    </I18nextProvider>,
  );
  return { registry, backing, out };
}

describe('useAddins', () => {
  it('feature off: no tab, no commands', async () => {
    const catalog = new FakeCatalog([hello], false);
    const { out, registry } = mount(catalog);
    await waitFor(() => expect(catalog.log).toEqual(['list:doc']));
    const tabs = [{ id: 'home', label: 'Home', groups: [] }];
    expect(out.current!.enabled).toBe(false);
    expect(out.current!.applyTabs(tabs)).toBe(tabs);
    expect(registry.cmds.size).toBe(0);
  });

  it('install asks for consent with the permissions, then shows ribbon commands and the frame', async () => {
    const broken = { ...hello, id: 'vn.example.broken', startUrl: 'https://evil.example/' };
    const sheetOnly = { ...hello, id: 'vn.example.sheet', apps: ['sheet'], ribbon: undefined };
    const catalog = new FakeCatalog([hello, broken, sheetOnly]);
    const { out, registry, backing } = mount(catalog);
    await waitFor(() => expect(out.current!.enabled).toBe(true));
    // Invalid manifests and add-ins of other apps are not offered.
    expect(out.current!.entries.map((e) => e.manifest.id)).toEqual(['vn.example.hello']);
    expect([...registry.cmds.keys()]).toEqual([MANAGE_COMMAND]);
    const tabs = out.current!.applyTabs([{ id: 'insert', label: 'Insert', groups: [] }]);
    expect(tabs.map((t) => t.id)).toEqual(['insert', 'addins']);
    expect(tabs[0]!.groups).toEqual([]);

    await act(async () => {
      await registry.cmds.get(MANAGE_COMMAND)!();
    });
    expect(out.current!.panes.map((p) => p.id)).toEqual([CATALOGUE_PANE]);
    fireEvent.click(await screen.findByTestId('addin-install'));
    const dialog = await screen.findByTestId('addin-consent');
    const perms = [...dialog.querySelectorAll('[data-permission]')].map((e) =>
      e.getAttribute('data-permission'),
    );
    expect(perms).toEqual([
      'selection',
      'write-document',
      'ui:ribbon',
      'network:https://api.example.vn',
    ]);
    expect(dialog.textContent).toContain('https://addins.example.vn');
    expect(catalog.installs.size).toBe(0);
    fireEvent.click(screen.getByTestId('addin-consent-accept'));
    await waitFor(() =>
      expect(catalog.installs.get('vn.example.hello')).toEqual(hello.permissions),
    );

    await waitFor(() =>
      expect(out.current!.panes.map((p) => p.id)).toEqual([CATALOGUE_PANE, addinPaneId(hello.id)]),
    );
    const frame = await screen.findByTestId('addin-frame');
    expect(frame.getAttribute('sandbox')).toBe('allow-scripts allow-forms');
    expect(frame.getAttribute('csp')).toContain('connect-src https://api.example.vn');
    expect(frame.getAttribute('src')).toBe(hello.startUrl);
    expect(registry.cmds.has(addinCommandId(hello.id, 'open'))).toBe(true);
    expect(registry.cmds.has(addinCommandId(hello.id, 'greet'))).toBe(true);
    const withAddin = out.current!.applyTabs([{ id: 'insert', label: 'Insert', groups: [] }]);
    expect(withAddin[0]!.groups[0]!.items.map((i) => i.label)).toEqual(['Open hello', 'Greet']);

    // Uninstall: catalogue told, pane closed, storage gone.
    addinStorage(hello.id, backing).set('k', 1);
    fireEvent.click(screen.getByTestId('addin-uninstall'));
    await waitFor(() => expect(catalog.log).toContain(`uninstall:${hello.id}`));
    await waitFor(() => expect(out.current!.panes.map((p) => p.id)).toEqual([CATALOGUE_PANE]));
    expect(backing.dump()).toEqual({});
    expect(registry.cmds.has(addinCommandId(hello.id, 'open'))).toBe(false);
    expect(out.current!.close(CATALOGUE_PANE)).toBe(true);
    expect(out.current!.close('styles')).toBe(false);
  });

  it('cancelling the consent installs nothing', async () => {
    const catalog = new FakeCatalog([hello]);
    const { out, registry } = mount(catalog);
    await waitFor(() => expect(out.current!.enabled).toBe(true));
    await act(async () => {
      await registry.cmds.get(MANAGE_COMMAND)!();
    });
    fireEvent.click(await screen.findByTestId('addin-install'));
    fireEvent.click(await screen.findByTestId('addin-consent-cancel'));
    await waitFor(() => expect(screen.queryByTestId('addin-consent')).toBeNull());
    expect(catalog.log).not.toContain(`install:${hello.id}`);
  });
});
