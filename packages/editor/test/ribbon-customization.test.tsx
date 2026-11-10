// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  addCustomTab,
  addGroupCommand,
  applyRibbonCustomization,
  type RibbonTab,
} from '@blueoffice/ui';
import {
  CUSTOMIZATION_FORMAT,
  CommandRegistry,
  RibbonCustomizeDialog,
  customizeCommands,
  exportCustomization,
  exportShortcuts,
  importCustomization,
  loadCustomization,
  ribbonKey,
  saveCustomization,
  useRibbonCustomization,
  type UiCustomization,
} from '../src';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const noop = () => {};
const TABS: RibbonTab[] = [
  {
    id: 'home',
    label: 'Trang đầu',
    groups: [
      {
        id: 'font',
        label: 'Phông',
        items: [
          { id: 'format.bold', label: 'Đậm', onSelect: noop },
          { id: 'format.italic', label: 'Nghiêng', onSelect: noop },
        ],
      },
    ],
  },
  {
    id: 'insert',
    label: 'Chèn',
    groups: [
      {
        id: 'tables',
        label: 'Bảng',
        items: [{ id: 'insert.table', label: 'Bảng', onSelect: noop }],
      },
    ],
  },
];
const KNOWN = new Set(['format.bold', 'format.italic', 'insert.table', 'file.save', 'edit.undo']);
const known = (id: string) => KNOWN.has(id);

function sample(): UiCustomization {
  const t = addCustomTab({}, TABS, 'Của tôi', 'Nhóm');
  const ribbon = addGroupCommand(
    addGroupCommand(t.value, t.tab, t.group, 'format.bold'),
    t.tab,
    t.group,
    'insert.table',
  );
  return { ribbon, qat: { commands: ['file.save', 'format.bold'], below: true } };
}

describe('customisation file', () => {
  it('round-trips ribbon, Quick Access Toolbar and shortcuts', () => {
    const c = sample();
    const text = exportCustomization('doc', c, { 'format.bold': 'Ctrl+Alt+K' });
    const f = JSON.parse(text) as { format: string; version: number; app: string };
    expect(f.format).toBe(CUSTOMIZATION_FORMAT);
    expect(f.version).toBe(1);
    expect(f.app).toBe('doc');
    const r = importCustomization(text, 'doc', known, { ribbon: {}, qat: {} });
    expect(r).toEqual({
      ok: true,
      value: c,
      shortcuts: { 'format.bold': 'Ctrl+Alt+K' },
      ignored: [],
    });
  });

  it('leaves out unknown command ids and reports them', () => {
    const c = sample();
    c.qat.commands = [...(c.qat.commands ?? []), 'plugin.gone'];
    const g = c.ribbon.groups!['custom.tab1']![0]!;
    g.commands.push('other.app');
    const text = exportCustomization('doc', c, { 'old.cmd': 'Ctrl+Alt+J' });
    const r = importCustomization(text, 'doc', known, { ribbon: {}, qat: {} });
    if (!r.ok) throw new Error('import failed');
    expect(r.ignored.sort()).toEqual(['old.cmd', 'other.app', 'plugin.gone']);
    expect(r.value.qat.commands).toEqual(['file.save', 'format.bold']);
    expect(r.value.ribbon.groups?.['custom.tab1']?.[0]?.commands).toEqual([
      'format.bold',
      'insert.table',
    ]);
    expect(r.shortcuts).toEqual({});
  });

  it('refuses invalid files, other apps and newer versions', () => {
    const cur = { ribbon: {}, qat: {} };
    const imp = (t: string) => importCustomization(t, 'doc', known, cur);
    expect(imp('not json')).toEqual({ ok: false, error: 'invalid' });
    expect(imp('[]')).toEqual({ ok: false, error: 'invalid' });
    expect(imp(JSON.stringify({ format: 'other', version: 1, app: 'doc' }))).toEqual({
      ok: false,
      error: 'invalid',
    });
    expect(imp(JSON.stringify({ format: CUSTOMIZATION_FORMAT, app: 'doc' }))).toEqual({
      ok: false,
      error: 'invalid',
    });
    const other = exportCustomization('sheet', sample(), {});
    expect(imp(other)).toEqual({ ok: false, error: 'wrongApp' });
    const newer = JSON.stringify({ format: CUSTOMIZATION_FORMAT, version: 99, app: 'doc' });
    expect(imp(newer)).toEqual({ ok: false, error: 'newer' });
    // Malformed parts of a valid file are dropped.
    const messy = JSON.stringify({
      format: CUSTOMIZATION_FORMAT,
      version: 1,
      app: 'doc',
      ribbon: { tabs: 'x', hidden: [1, 'tab:home'] },
      qat: { commands: 'nope' },
      shortcuts: { 'format.bold': 'B' },
    });
    expect(imp(messy)).toEqual({
      ok: true,
      value: { ribbon: { hidden: ['tab:home'] }, qat: {} },
      shortcuts: {},
      ignored: [],
    });
  });

  it('accepts a shortcuts-only file and keeps the ribbon', () => {
    const cur = sample();
    const r = importCustomization(
      exportShortcuts('doc', { 'format.italic': 'Ctrl+Alt+I' }),
      'doc',
      known,
      cur,
    );
    expect(r).toEqual({
      ok: true,
      value: cur,
      shortcuts: { 'format.italic': 'Ctrl+Alt+I' },
      ignored: [],
    });
  });

  it('is kept per app in the setting store, else localStorage', async () => {
    const c = sample();
    const kept = new Map<string, string | null>();
    const store = {
      getSetting: async (k: string) => kept.get(k) ?? null,
      setSetting: async (k: string, v: string | null) => void kept.set(k, v),
    };
    await saveCustomization(store, 'slide', c);
    expect(kept.has(ribbonKey('slide'))).toBe(true);
    expect(await loadCustomization(store, 'slide')).toEqual(c);
    expect(await loadCustomization(store, 'doc')).toEqual({ ribbon: {}, qat: {} });
    await saveCustomization({}, 'sheet', c);
    expect(localStorage.getItem('blueoffice.ribbon.sheet')).toBeTruthy();
    expect(await loadCustomization({}, 'sheet')).toEqual(c);
    await saveCustomization({}, 'sheet', { ribbon: {}, qat: {} });
    expect(localStorage.getItem('blueoffice.ribbon.sheet')).toBeNull();
    localStorage.setItem('blueoffice.ribbon.doc', '{broken');
    expect(await loadCustomization({}, 'doc')).toEqual({ ribbon: {}, qat: {} });
  });
});

describe('RibbonCustomizeDialog', () => {
  it('adds a custom tab, puts commands in it, hides a group, edits the toolbar', () => {
    const i18n = createI18n('vi-VN');
    let value: UiCustomization = { ribbon: {}, qat: {} };
    let page: 'ribbon' | 'qat' = 'ribbon';
    const view = () => (
      <I18nextProvider i18n={i18n}>
        <RibbonCustomizeDialog
          page={page}
          onPage={(p) => {
            page = p;
          }}
          onClose={noop}
          builtins={TABS}
          commands={customizeCommands(TABS, [{ id: 'edit.undo', label: 'Hoàn tác' }])}
          filterTabs={TABS.map((t) => ({ id: t.id, label: t.label }))}
          value={value}
          onChange={(v) => {
            value = v;
          }}
          onExport={noop}
          onImport={noop}
        />
      </I18nextProvider>
    );
    const { rerender } = render(view());
    const update = () => rerender(view());
    const option = (id: string) => document.querySelector(`[data-command-option="${id}"]`)!;

    // Filter by tab.
    fireEvent.change(screen.getByTestId('customize-filter'), { target: { value: 'insert' } });
    expect(document.querySelectorAll('[data-command-option]').length).toBe(1);
    fireEvent.change(screen.getByTestId('customize-filter'), { target: { value: '' } });
    fireEvent.change(screen.getByTestId('customize-search'), { target: { value: 'nghieng' } });
    expect(
      [...document.querySelectorAll('[data-command-option]')].map((e) =>
        e.getAttribute('data-command-option'),
      ),
    ).toEqual(['format.italic']);
    fireEvent.change(screen.getByTestId('customize-search'), { target: { value: '' } });

    // New tab (after the selected one) with its group selected; add two commands.
    fireEvent.click(document.querySelector('[data-node="tab:home"]')!);
    fireEvent.click(screen.getByTestId('customize-new-tab'));
    update();
    fireEvent.click(option('format.italic'));
    fireEvent.click(screen.getByTestId('customize-add'));
    update();
    fireEvent.click(option('edit.undo'));
    fireEvent.click(screen.getByTestId('customize-add'));
    update();
    expect(value.ribbon.groups?.['custom.tab1']?.[0]?.commands).toEqual([
      'format.italic',
      'edit.undo',
    ]);
    let out = applyRibbonCustomization(TABS, value.ribbon);
    expect(out.map((t) => t.id)).toEqual(['home', 'custom.tab1', 'insert']);

    // Reorder a command, rename the tab.
    fireEvent.click(screen.getByTestId('customize-up'));
    update();
    expect(value.ribbon.groups?.['custom.tab1']?.[0]?.commands).toEqual([
      'edit.undo',
      'format.italic',
    ]);
    fireEvent.click(document.querySelector('[data-node="tab:custom.tab1"]')!);
    fireEvent.click(screen.getByTestId('customize-rename'));
    fireEvent.change(screen.getByTestId('customize-rename-input'), {
      target: { value: 'Hay dùng' },
    });
    fireEvent.click(screen.getByTestId('customize-rename-ok'));
    update();
    expect(value.ribbon.tabs).toEqual([{ id: 'custom.tab1', label: 'Hay dùng' }]);
    fireEvent.click(screen.getByTestId('customize-down'));
    update();
    out = applyRibbonCustomization(TABS, value.ribbon);
    expect(out.map((t) => t.id)).toEqual(['home', 'insert', 'custom.tab1']);

    // Built-ins: no remove, hide with the checkbox, reset the tab.
    const home = document.querySelector('[data-node="tab:home"]') as HTMLElement;
    fireEvent.click(home);
    update();
    expect((screen.getByTestId('customize-remove') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(home.querySelector('[data-testid="customize-twisty"]')!);
    update();
    const font = document.querySelector('[data-node="group:home/font"]')!;
    fireEvent.click(font.querySelector('input[type="checkbox"]')!);
    update();
    expect(value.ribbon.hidden).toEqual(['group:home/font']);
    fireEvent.click(screen.getByTestId('customize-reset-tab'));
    update();
    expect(value.ribbon.hidden).toEqual([]);

    // Toolbar page.
    act(() => fireEvent.click(screen.getByTestId('customize-page-qat')));
    expect(page).toBe('qat');
    update();
    fireEvent.click(option('format.bold'));
    fireEvent.click(screen.getByTestId('customize-qat-add'));
    update();
    expect(value.qat.commands).toEqual(['file.save', 'edit.undo', 'edit.redo', 'format.bold']);
    fireEvent.click(document.querySelector('[data-qat-option="file.save"]')!);
    fireEvent.click(screen.getByTestId('customize-qat-remove'));
    update();
    fireEvent.click(screen.getByTestId('customize-qat-below'));
    update();
    expect(value.qat).toEqual({ commands: ['edit.undo', 'edit.redo', 'format.bold'], below: true });
  });

  it('the hook registers the commands, renders the toolbar and toggles it from the ribbon menu', async () => {
    const i18n = createI18n('vi-VN');
    const r = new CommandRegistry<null>();
    const run = vi.fn();
    for (const id of KNOWN) r.register({ id, run: noop });
    let api: ReturnType<typeof useRibbonCustomization<null>> | null = null;
    function Host() {
      api = useRibbonCustomization({
        app: 'doc',
        registry: r,
        store: {},
        files: { openFile: vi.fn(), saveFile: vi.fn() },
        tabs: () => TABS,
        extraCommands: () => [{ id: 'edit.undo', label: 'Hoàn tác' }],
        shortcuts: { map: {}, apply: vi.fn() },
        enabled: () => true,
        run,
      });
      return api.element;
    }
    render(
      <I18nextProvider i18n={i18n}>
        <Host />
      </I18nextProvider>,
    );
    await act(async () => {});
    expect(r.has('options.customizeRibbon')).toBe(true);
    expect(api!.quickAccess(TABS).map((q) => q.id)).toEqual(['file.save', 'edit.undo']);
    act(() => api!.ribbonProps().customizeMenu.onToggleQat('format.bold'));
    expect(api!.quickAccess(TABS).map((q) => q.id)).toEqual([
      'file.save',
      'edit.undo',
      'format.bold',
    ]);
    api!.quickAccess(TABS)[2]!.onSelect();
    expect(run).toHaveBeenCalledWith('format.bold');
    expect(JSON.parse(localStorage.getItem('blueoffice.ribbon.doc')!)).toEqual({
      qat: { commands: ['file.save', 'edit.undo', 'edit.redo', 'format.bold'] },
    });
    await act(async () => void r.execute('options.customizeQat', null));
    expect(screen.getByTestId('customize-dialog')).toBeTruthy();
    expect(screen.getByTestId('customize-qat-list')).toBeTruthy();
  });
});
