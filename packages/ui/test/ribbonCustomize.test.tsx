// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  addCustomGroup,
  addCustomTab,
  addGroupCommand,
  applyRibbonCustomization,
  compactCustomization,
  customizationTree,
  mergeOrder,
  moveGroupCommand,
  moveNode,
  qatAdd,
  qatCommands,
  qatMove,
  qatRemove,
  removeCustomNode,
  removeGroupCommand,
  renameNode,
  resetTab,
  Ribbon,
  sanitizeCustomization,
  sanitizeQat,
  setNodeHidden,
  setRibbonPrefs,
  simplifiedItems,
  type RibbonCustomization,
  type RibbonTab,
} from '../src';

afterEach(cleanup);

const noop = () => {};
const item = (id: string, label = id) => ({ id, label, onSelect: noop });
const TABS: RibbonTab[] = [
  {
    id: 'home',
    label: 'Trang đầu',
    groups: [
      {
        id: 'clip',
        label: 'Bộ nhớ tạm',
        items: [item('edit.paste', 'Dán'), item('edit.copy', 'Sao chép')],
      },
      {
        id: 'font',
        label: 'Phông',
        items: [item('format.bold', 'Đậm'), item('format.italic', 'Nghiêng')],
      },
    ],
  },
  {
    id: 'insert',
    label: 'Chèn',
    groups: [{ id: 'tables', label: 'Bảng', items: [item('insert.table', 'Bảng')] }],
  },
  {
    id: 'tableDesign',
    label: 'Thiết kế bảng',
    context: 'table',
    groups: [{ id: 'styles', label: 'Kiểu', items: [item('table.style', 'Kiểu bảng')] }],
  },
];

describe('ribbon customisation model', () => {
  it('merges a stored order with built-ins the app added later', () => {
    expect(mergeOrder(['a', 'b', 'c'], ['c', 'a'])).toEqual(['c', 'a', 'b']);
    // A new first default goes first; unknown / repeated ids are dropped; extras last.
    expect(mergeOrder(['n', 'a', 'b'], ['b', 'x', 'a', 'b'], ['custom.tab1'])).toEqual([
      'n',
      'b',
      'a',
      'custom.tab1',
    ]);
  });

  it('applies hide / rename / reorder and keeps contextual tabs as they are', () => {
    let c: RibbonCustomization = {};
    c = setNodeHidden(c, { kind: 'group', tab: 'home', id: 'clip' }, true);
    c = renameNode(c, TABS, { kind: 'tab', id: 'insert' }, 'Thêm');
    c = moveNode(c, TABS, { kind: 'tab', id: 'insert' }, 0);
    const out = applyRibbonCustomization(TABS, c);
    expect(out.map((t) => t.id)).toEqual(['insert', 'home', 'tableDesign']);
    expect(out[0]?.label).toBe('Thêm');
    expect(out[1]?.groups.map((g) => g.id)).toEqual(['font']);
    expect(out[2]).toBe(TABS[2]);
    // Hiding a tab removes it; contextual tabs never are in the tree.
    const hidden = applyRibbonCustomization(
      TABS,
      setNodeHidden(c, { kind: 'tab', id: 'home' }, true),
    );
    expect(hidden.map((t) => t.id)).toEqual(['insert', 'tableDesign']);
    expect(customizationTree(TABS, c).map((t) => t.id)).toEqual(['insert', 'home']);
    // Renaming back to the original label drops the change.
    expect(renameNode(c, TABS, { kind: 'tab', id: 'insert' }, 'Chèn').labels).toEqual({});
  });

  it('new built-in tabs, groups and commands show up under an old customisation', () => {
    const c = moveNode({}, TABS, { kind: 'tab', id: 'insert' }, 0);
    const grown: RibbonTab[] = [
      { id: 'draw', label: 'Vẽ', groups: [] },
      {
        ...TABS[0]!,
        groups: [...TABS[0]!.groups, { id: 'para', label: 'Đoạn', items: [item('para.center')] }],
      },
      TABS[1]!,
    ];
    const out = applyRibbonCustomization(grown, c);
    expect(out.map((t) => t.id)).toEqual(['draw', 'insert', 'home']);
    expect(out.find((t) => t.id === 'home')?.groups.map((g) => g.id)).toEqual([
      'clip',
      'font',
      'para',
    ]);
  });

  it('custom tabs and groups hold commands; built-ins are only hidden, never deleted', () => {
    const t = addCustomTab({}, TABS, 'Của tôi', 'Nhóm', 'home');
    let c = addGroupCommand(t.value, t.tab, t.group, 'format.bold');
    c = addGroupCommand(c, t.tab, t.group, 'insert.table');
    c = addGroupCommand(c, t.tab, t.group, 'format.bold'); // once per group
    c = addGroupCommand(c, t.tab, t.group, 'gone.command');
    expect(t.tab).toBe('custom.tab1');
    expect(t.group).toBe('custom.group1');
    let out = applyRibbonCustomization(TABS, c);
    expect(out.map((x) => x.id)).toEqual(['home', 'custom.tab1', 'insert', 'tableDesign']);
    const mine = out[1]!;
    expect(mine.label).toBe('Của tôi');
    // Unknown commands are skipped; the user's order is the simplified row's.
    expect(mine.groups[0]?.items.map((i) => i.id)).toEqual(['format.bold', 'insert.table']);
    c = moveGroupCommand(c, t.tab, t.group, 'insert.table', 0);
    out = applyRibbonCustomization(TABS, c);
    expect(out[1]?.groups[0]?.items.map((i) => i.id)).toEqual(['insert.table', 'format.bold']);
    expect(simplifiedItems(out[1]!.groups[0]!).shown.map((i) => i.id)[0]).toBe('insert.table');
    c = removeGroupCommand(c, t.tab, t.group, 'insert.table');
    expect(customizationTree(TABS, c)[1]?.groups[0]?.commands).toEqual([
      'format.bold',
      'gone.command',
    ]);
    // A group in a built-in tab, then removed again.
    const g = addCustomGroup(c, 'home', 'Thêm');
    expect(g.group).toBe('custom.group2');
    expect(applyRibbonCustomization(TABS, g.value)[0]?.groups.map((x) => x.id)).toEqual([
      'clip',
      'font',
      'custom.group2',
    ]);
    expect(removeCustomNode(g.value, { kind: 'group', tab: 'home', id: 'clip' })).toBe(g.value);
    const del = removeCustomNode(g.value, { kind: 'tab', id: t.tab });
    expect(applyRibbonCustomization(TABS, del).map((x) => x.id)).toEqual([
      'home',
      'insert',
      'tableDesign',
    ]);
    expect(del.groups?.[t.tab]).toBeUndefined();
  });

  it('resets one tab or everything', () => {
    let c: RibbonCustomization = {};
    c = setNodeHidden(c, { kind: 'group', tab: 'home', id: 'font' }, true);
    c = renameNode(c, TABS, { kind: 'tab', id: 'home' }, 'Nhà');
    c = moveNode(c, TABS, { kind: 'group', tab: 'home', id: 'font' }, 0);
    c = addCustomGroup(c, 'home', 'X').value;
    c = setNodeHidden(c, { kind: 'tab', id: 'insert' }, true);
    const r = resetTab(c, 'home');
    expect(applyRibbonCustomization(TABS, r)[0]).toBe(TABS[0]);
    // The other tab keeps its change.
    expect(applyRibbonCustomization(TABS, r).map((x) => x.id)).toEqual(['home', 'tableDesign']);
    expect(compactCustomization(resetTab(r, 'insert'))).toEqual({});
  });

  it('edits the Quick Access Toolbar', () => {
    expect(qatCommands({})).toEqual(['file.save', 'edit.undo', 'edit.redo']);
    let q = qatAdd({}, 'format.bold');
    q = qatAdd(q, 'format.bold');
    expect(q.commands).toEqual(['file.save', 'edit.undo', 'edit.redo', 'format.bold']);
    q = qatMove(q, 'format.bold', 0);
    q = qatRemove(q, 'edit.redo');
    expect(qatCommands(q)).toEqual(['format.bold', 'file.save', 'edit.undo']);
  });

  it('sanitises stored data and reports unknown commands', () => {
    const r = sanitizeCustomization(
      {
        tabOrder: ['home', 3, 'bad id!'],
        hidden: ['group:home/clip', 'nonsense'],
        labels: { 'tab:home': '  Nhà ', 'x:y': 'z' },
        tabs: [
          { id: 'custom.tab1', label: 'A' },
          { id: 'home', label: 'not custom' },
          { id: 'custom.tab2' },
        ],
        groups: {
          'custom.tab1': [
            { id: 'custom.group1', label: 'G', commands: ['format.bold', 'nope.cmd'] },
          ],
          'custom.tab9': [{ id: 'custom.group2', label: 'orphan', commands: [] }],
        },
      },
      (id) => id !== 'nope.cmd',
    );
    expect(r.value).toEqual({
      tabOrder: ['home'],
      hidden: ['group:home/clip'],
      labels: { 'tab:home': 'Nhà' },
      tabs: [{ id: 'custom.tab1', label: 'A' }],
      groups: { 'custom.tab1': [{ id: 'custom.group1', label: 'G', commands: ['format.bold'] }] },
    });
    expect(r.ignored).toEqual(['nope.cmd']);
    expect(sanitizeCustomization('x').value).toEqual({});
    expect(sanitizeQat({ commands: ['a.b', 'zz'], below: true }, (id) => id === 'a.b')).toEqual({
      value: { commands: ['a.b'], below: true },
      ignored: ['zz'],
    });
  });
});

describe('Ribbon with a customisation', () => {
  it('renders custom tabs (with KeyTips) and hidden groups; simplified mode follows it', () => {
    const t = addCustomTab({}, TABS, 'Của tôi', 'Nhóm của tôi');
    let c = addGroupCommand(t.value, t.tab, t.group, 'format.italic');
    c = addGroupCommand(c, t.tab, t.group, 'edit.copy');
    c = setNodeHidden(c, { kind: 'group', tab: 'home', id: 'clip' }, true);
    const { container } = render(<Ribbon tabs={TABS} customization={c} autoActivate={[]} />);
    expect(container.querySelector('[data-group="clip"]')).toBeNull();
    const tab = screen.getByRole('tab', { name: 'Của tôi' });
    expect(tab.getAttribute('data-tab')).toBe('custom.tab1');
    fireEvent.mouseDown(tab);
    fireEvent.click(tab);
    const group = container.querySelector('[data-group="custom.group1"]')!;
    expect(group.getAttribute('aria-label')).toBe('Nhóm của tôi');
    expect(
      [...group.querySelectorAll('button[data-command]')].map((b) =>
        b.getAttribute('data-command'),
      ),
    ).toEqual(['format.italic', 'edit.copy']);
    // KeyTips cover the custom tab too.
    fireEvent.keyDown(window, { key: 'Alt', code: 'AltLeft', altKey: true });
    fireEvent.keyUp(window, { key: 'Alt', code: 'AltLeft' });
    const tips = Object.fromEntries(
      [...document.querySelectorAll('[data-keytip-for]')].map((e) => [
        e.getAttribute('data-keytip-for'),
        e.getAttribute('data-keytip'),
      ]),
    );
    expect(tips['tab:custom.tab1']).toBeTruthy();
    cleanup();
    setRibbonPrefs({ simplified: true });
    try {
      const r = render(
        <Ribbon tabs={TABS} customization={c} value="custom.tab1" autoActivate={[]} />,
      );
      const g = r.container.querySelector('[data-group="custom.group1"]')!;
      expect(g.querySelectorAll('button[data-command]').length).toBe(2);
    } finally {
      setRibbonPrefs({ simplified: false });
    }
  });

  it('right click offers customising and adding a command to the Quick Access Toolbar', () => {
    const onToggleQat = vi.fn();
    const onCustomizeRibbon = vi.fn();
    const { container } = render(
      <Ribbon
        tabs={TABS}
        autoActivate={[]}
        customizeMenu={{
          labels: {
            menu: 'Tùy biến',
            customizeRibbon: 'Tùy biến ribbon…',
            customizeQat: 'Tùy biến thanh truy cập nhanh…',
            addToQat: 'Thêm vào thanh truy cập nhanh',
            removeFromQat: 'Xóa khỏi thanh truy cập nhanh',
          },
          onCustomizeRibbon,
          onCustomizeQat: noop,
          inQat: (id) => id === 'edit.copy',
          onToggleQat,
        }}
      />,
    );
    fireEvent.contextMenu(container.querySelector('button[data-command="edit.paste"]')!);
    const menu = screen.getByTestId('context-menu-ribbon');
    fireEvent.click(menu.querySelector('[data-menu-command="ribbon.addToQat"]')!);
    expect(onToggleQat).toHaveBeenCalledWith('edit.paste');
    fireEvent.contextMenu(container.querySelector('button[data-command="edit.copy"]')!);
    expect(
      screen
        .getByTestId('context-menu-ribbon')
        .querySelector('[data-menu-command="ribbon.removeFromQat"]'),
    ).not.toBeNull();
    fireEvent.click(screen.getByText('Tùy biến ribbon…'));
    expect(onCustomizeRibbon).toHaveBeenCalled();
    // On empty ribbon space: only the customise entries.
    fireEvent.contextMenu(screen.getByTestId('ribbon'));
    const lines = screen.getByTestId('context-menu-ribbon').querySelectorAll('[data-menu-command]');
    expect([...lines].map((l) => l.getAttribute('data-menu-command'))).toEqual([
      'options.customizeRibbon',
      'options.customizeQat',
    ]);
  });
});
