import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { FileAccess } from '@blueoffice/platform';
import {
  addCustomGroup,
  addCustomTab,
  addGroupCommand,
  customizationTree,
  Dialog,
  flattenRibbon,
  isCustomId,
  moveGroupCommand,
  moveNode,
  normalize,
  qatAdd,
  qatCommands,
  qatMove,
  qatRemove,
  removeCustomNode,
  ribbonItems,
  removeGroupCommand,
  renameNode,
  resetTab,
  setNodeHidden,
  type QuickAccessItem,
  type RibbonCustomizeMenu,
  type RibbonCustomization,
  type RibbonItem,
  type RibbonTab,
  type SettingStore,
  type TreeTab,
} from '@blueoffice/ui';
import type { CommandRegistry } from './commands';
import {
  exportCustomization,
  importCustomization,
  loadCustomization,
  saveCustomization,
  type UiCustomization,
} from './ribbonCustomization';
import type { ShortcutCommand } from './ShortcutsDialog';
import type { ShortcutApp, ShortcutMap } from './shortcuts';

/** A command of the dialog's list: label, place and the tab it comes from. */
export interface CustomizeCommand {
  id: string;
  label: string;
  path?: string;
  /** Ribbon tab holding it (filter); none for commands that only have a shortcut. */
  tab?: string;
}

export type CustomizePage = 'ribbon' | 'qat';

type Sel =
  | { kind: 'tab'; id: string }
  | { kind: 'group'; tab: string; id: string }
  | { kind: 'cmd'; tab: string; group: string; id: string };

type Drag = Sel | { kind: 'new'; id: string } | { kind: 'qat'; id: string };

const selKey = (s: Sel | null) =>
  !s
    ? ''
    : s.kind === 'tab'
      ? `tab:${s.id}`
      : s.kind === 'group'
        ? `group:${s.tab}/${s.id}`
        : `cmd:${s.tab}/${s.group}/${s.id}`;

export interface RibbonCustomizeDialogProps {
  page: CustomizePage;
  onPage: (page: CustomizePage) => void;
  onClose: () => void;
  /** The ribbon's regular tabs as the app builds them (contextual ones are left out). */
  builtins: readonly RibbonTab[];
  /** Every command of the app (left list). */
  commands: readonly CustomizeCommand[];
  /** Tabs offered by the filter (id, label). */
  filterTabs: readonly { id: string; label: string }[];
  value: UiCustomization;
  onChange: (value: UiCustomization) => void;
  onExport: () => void;
  onImport: () => void;
  notice?: string | null;
}

const listStyle = {
  border: '1px solid var(--bo-border)',
  height: 320,
  overflow: 'auto',
  margin: 0,
  padding: 2,
  listStyle: 'none',
  fontSize: 13,
} as const;

/**
 * Customise the ribbon and the Quick Access Toolbar (CORE-UI-005): the app's commands on the
 * left (search, filter by tab); on the right the tabs → groups → commands tree (new tab / group,
 * rename, reorder with the buttons or by dragging, hide built-in tabs and groups, remove
 * commands of custom groups, reset one tab or everything) or the toolbar's commands. Changes
 * apply at once.
 */
export function RibbonCustomizeDialog({
  page,
  onPage,
  onClose,
  builtins,
  commands,
  filterTabs,
  value,
  onChange,
  onExport,
  onImport,
  notice,
}: RibbonCustomizeDialogProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('');
  const [picked, setPicked] = useState<string | null>(null);
  const [sel, setSel] = useState<Sel | null>(null);
  const [qatSel, setQatSel] = useState<string | null>(null);
  const [open, setOpen] = useState<Set<string>>(() => new Set());
  const [renaming, setRenaming] = useState<string | null>(null);
  const drag = useRef<Drag | null>(null);
  const ribbon = value.ribbon;
  const tree = useMemo(() => customizationTree(builtins, ribbon), [builtins, ribbon]);
  const labelOf = useMemo(() => {
    const m = new Map(commands.map((c) => [c.id, c.label]));
    return (id: string) => m.get(id) ?? id;
  }, [commands]);
  const rows = useMemo(() => {
    const q = normalize(query);
    return commands.filter(
      (c) =>
        (!filter || c.tab === filter) &&
        (!q || `${normalize(c.label)} ${normalize(c.path ?? '')} ${c.id}`.includes(q)),
    );
  }, [commands, query, filter]);

  const setRibbon = (r: RibbonCustomization) => onChange({ ...value, ribbon: r });
  const expand = (...keys: string[]) =>
    setOpen((o) => {
      const n = new Set(o);
      for (const k of keys) n.add(k);
      return n;
    });
  const tabOf = (s: Sel | null) => (s ? (s.kind === 'tab' ? s.id : s.tab) : null);
  const selTab = tree.find((x) => x.id === tabOf(sel));
  const selGroup =
    sel && sel.kind !== 'tab'
      ? selTab?.groups.find((g) => g.id === (sel.kind === 'group' ? sel.id : sel.group))
      : undefined;
  const customGroup = selGroup && !selGroup.builtIn ? selGroup : undefined;
  const qat = qatCommands(value.qat);

  const add = (id: string | null, at?: { tab: string; group: string; index?: number }) => {
    if (!id) return;
    const target =
      at ??
      (customGroup && selGroup
        ? {
            tab: selGroup.tab,
            group: selGroup.id,
            index: sel?.kind === 'cmd' ? customGroup.commands.indexOf(sel.id) + 1 : undefined,
          }
        : null);
    if (!target) return;
    setRibbon(addGroupCommand(ribbon, target.tab, target.group, id, target.index));
    expand(`tab:${target.tab}`, `group:${target.tab}/${target.group}`);
    setSel({ kind: 'cmd', tab: target.tab, group: target.group, id });
  };
  const canRemove = !!sel && (sel.kind === 'cmd' ? !!customGroup : isCustomId(sel.id));
  const remove = () => {
    if (!sel || !canRemove) return;
    if (sel.kind === 'cmd') setRibbon(removeGroupCommand(ribbon, sel.tab, sel.group, sel.id));
    else setRibbon(removeCustomNode(ribbon, sel));
    setSel(
      sel.kind === 'tab'
        ? null
        : sel.kind === 'group'
          ? { kind: 'tab', id: sel.tab }
          : {
              kind: 'group',
              tab: sel.tab,
              id: sel.group,
            },
    );
  };
  /** Position of the selection among its siblings, and their count. */
  const place = (s: Sel | null): [number, number] => {
    if (!s) return [-1, 0];
    if (s.kind === 'tab') return [tree.findIndex((x) => x.id === s.id), tree.length];
    const tb = tree.find((x) => x.id === s.tab);
    if (s.kind === 'group') {
      const gs = tb?.groups ?? [];
      return [gs.findIndex((g) => g.id === s.id), gs.length];
    }
    const g = tb?.groups.find((x) => x.id === s.group);
    if (!g || g.builtIn) return [-1, 0];
    return [g.commands.indexOf(s.id), g.commands.length];
  };
  const moveTo = (s: Sel, index: number) => {
    if (s.kind === 'cmd') setRibbon(moveGroupCommand(ribbon, s.tab, s.group, s.id, index));
    else setRibbon(moveNode(ribbon, builtins, s, index));
  };
  const [pos, count] = place(sel);

  const newTab = () => {
    const r = addCustomTab(
      ribbon,
      builtins,
      t('customize.newTabLabel'),
      t('customize.newGroupLabel'),
      tabOf(sel) ?? undefined,
    );
    setRibbon(r.value);
    expand(`tab:${r.tab}`, `group:${r.tab}/${r.group}`);
    setSel({ kind: 'group', tab: r.tab, id: r.group });
  };
  const newGroup = () => {
    const tab = tabOf(sel);
    if (!tab) return;
    const r = addCustomGroup(ribbon, tab, t('customize.newGroupLabel'));
    setRibbon(r.value);
    expand(`tab:${tab}`, `group:${tab}/${r.group}`);
    setSel({ kind: 'group', tab, id: r.group });
  };
  const nodeLabel = (s: Sel | null) => {
    if (!s || s.kind === 'cmd') return '';
    if (s.kind === 'tab') return selTab?.label ?? '';
    return selGroup?.label ?? '';
  };

  // Drag and drop: siblings reorder; a command from the left list drops into a custom group.
  const onDrop = (e: DragEvent, target: Sel) => {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    e.preventDefault();
    const tb = tree.find((x) => x.id === tabOf(target));
    if (d.kind === 'new') {
      if (target.kind === 'tab') return;
      const groupId = target.kind === 'group' ? target.id : target.group;
      const g = tb?.groups.find((x) => x.id === groupId);
      if (!g || g.builtIn) return;
      add(d.id, {
        tab: g.tab,
        group: g.id,
        index: target.kind === 'cmd' ? g.commands.indexOf(target.id) : undefined,
      });
      return;
    }
    if (d.kind === 'qat' || d.kind !== target.kind) return;
    if (d.kind === 'tab')
      moveTo(
        d,
        tree.findIndex((x) => x.id === target.id),
      );
    else if (d.kind === 'group' && target.kind === 'group' && d.tab === target.tab)
      moveTo(d, tb?.groups.findIndex((g) => g.id === target.id) ?? 0);
    else if (d.kind === 'cmd' && target.kind === 'cmd' && d.group === target.group)
      moveTo(d, tb?.groups.find((g) => g.id === d.group)?.commands.indexOf(target.id) ?? 0);
    setSel(d);
  };
  const dragProps = (s: Drag) => ({
    draggable: true,
    onDragStart: (e: DragEvent) => {
      drag.current = s;
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', s.id);
    },
  });
  const dropProps = (s: Sel) => ({
    onDragOver: (e: DragEvent) => {
      if (drag.current) e.preventDefault();
    },
    onDrop: (e: DragEvent) => onDrop(e, s),
  });

  // Visible tree lines in order (arrow keys).
  const visible: Sel[] = [];
  for (const tb of tree) {
    visible.push({ kind: 'tab', id: tb.id });
    if (!open.has(`tab:${tb.id}`)) continue;
    for (const g of tb.groups) {
      visible.push({ kind: 'group', tab: tb.id, id: g.id });
      if (!open.has(`group:${tb.id}/${g.id}`)) continue;
      for (const c of g.commands) visible.push({ kind: 'cmd', tab: tb.id, group: g.id, id: c });
    }
  }
  const treeRef = useRef<HTMLUListElement>(null);
  useEffect(() => {
    const el = treeRef.current?.querySelector<HTMLElement>(
      `[data-node="${CSS.escape(selKey(sel))}"]`,
    );
    if (el && treeRef.current?.contains(document.activeElement)) el.focus();
  }, [sel]);
  const onTreeKey = (e: React.KeyboardEvent) => {
    const i = visible.findIndex((v) => selKey(v) === selKey(sel));
    const key = selKey(sel);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n =
        visible[Math.max(0, Math.min(visible.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
      if (n) setSel(n);
    } else if ((e.key === 'ArrowRight' || e.key === 'ArrowLeft') && sel && sel.kind !== 'cmd') {
      e.preventDefault();
      setOpen((o) => {
        const n = new Set(o);
        if (e.key === 'ArrowRight') n.add(key);
        else n.delete(key);
        return n;
      });
    }
  };

  const toggle = (key: string) =>
    setOpen((o) => {
      const n = new Set(o);
      if (n.has(key)) n.delete(key);
      else n.add(key);
      return n;
    });
  const renderTab = (tb: TreeTab) => {
    const s: Sel = { kind: 'tab', id: tb.id };
    const key = selKey(s);
    const expanded = open.has(key);
    return (
      <li key={key} role="none">
        <div
          role="treeitem"
          aria-level={1}
          aria-selected={selKey(sel) === key}
          aria-expanded={expanded}
          tabIndex={selKey(sel) === key || (!sel && tb === tree[0]) ? 0 : -1}
          data-node={key}
          style={lineStyle(0, selKey(sel) === key)}
          onClick={() => setSel(s)}
          {...dragProps(s)}
          {...dropProps(s)}
        >
          <Twisty expanded={expanded} label={t('customize.expand')} onClick={() => toggle(key)} />
          <input
            type="checkbox"
            checked={!tb.hidden}
            aria-label={t('customize.show', { name: tb.label })}
            data-testid="customize-visible"
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => setRibbon(setNodeHidden(ribbon, s, !e.target.checked))}
          />{' '}
          {tb.label}
          {!tb.builtIn && <Muted text={t('customize.custom')} />}
        </div>
        {expanded && (
          <ul role="group" style={{ listStyle: 'none', margin: 0, padding: 0 }}>
            {tb.groups.map((g) => {
              const gs: Sel = { kind: 'group', tab: tb.id, id: g.id };
              const gk = selKey(gs);
              const gOpen = open.has(gk);
              return (
                <li key={gk} role="none">
                  <div
                    role="treeitem"
                    aria-level={2}
                    aria-selected={selKey(sel) === gk}
                    aria-expanded={gOpen}
                    tabIndex={selKey(sel) === gk ? 0 : -1}
                    data-node={gk}
                    style={lineStyle(1, selKey(sel) === gk)}
                    onClick={() => setSel(gs)}
                    onDoubleClick={() => toggle(gk)}
                    {...dragProps(gs)}
                    {...dropProps(gs)}
                  >
                    <Twisty
                      expanded={gOpen}
                      label={t('customize.expand')}
                      onClick={() => toggle(gk)}
                    />
                    <input
                      type="checkbox"
                      checked={!g.hidden}
                      aria-label={t('customize.show', { name: g.label })}
                      data-testid="customize-visible"
                      onClick={(e) => e.stopPropagation()}
                      onChange={(e) => setRibbon(setNodeHidden(ribbon, gs, !e.target.checked))}
                    />{' '}
                    {g.label}
                    {!g.builtIn && <Muted text={t('customize.custom')} />}
                  </div>
                  {gOpen && (
                    <ul role="group" style={{ listStyle: 'none', margin: 0, padding: 0 }}>
                      {g.commands.map((c) => {
                        const cs: Sel = { kind: 'cmd', tab: tb.id, group: g.id, id: c };
                        const ck = selKey(cs);
                        return (
                          <li key={ck} role="none">
                            <div
                              role="treeitem"
                              aria-level={3}
                              aria-selected={selKey(sel) === ck}
                              tabIndex={selKey(sel) === ck ? 0 : -1}
                              data-node={ck}
                              style={lineStyle(2, selKey(sel) === ck)}
                              onClick={() => setSel(cs)}
                              {...(g.builtIn ? {} : { ...dragProps(cs), ...dropProps(cs) })}
                            >
                              {labelOf(c)}
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </li>
    );
  };

  const commandList = (
    <div style={{ flex: 1, minWidth: 0 }}>
      <input
        type="search"
        aria-label={t('customize.search')}
        placeholder={t('customize.search')}
        value={query}
        data-testid="customize-search"
        onChange={(e) => setQuery(e.target.value)}
        style={{ width: '100%', boxSizing: 'border-box', marginBottom: 4 }}
      />
      <select
        aria-label={t('customize.filter')}
        value={filter}
        data-testid="customize-filter"
        onChange={(e) => setFilter(e.target.value)}
        style={{ width: '100%', marginBottom: 4 }}
      >
        <option value="">{t('customize.allTabs')}</option>
        {filterTabs.map((ft) => (
          <option key={ft.id} value={ft.id}>
            {ft.label}
          </option>
        ))}
      </select>
      <ul
        role="listbox"
        aria-label={t('customize.commands')}
        data-testid="customize-commands"
        style={{ ...listStyle, height: 268 }}
      >
        {rows.map((c) => (
          <li
            key={c.id}
            role="option"
            aria-selected={picked === c.id}
            data-command-option={c.id}
            title={c.path}
            style={lineStyle(0, picked === c.id)}
            onClick={() => setPicked(c.id)}
            onDoubleClick={() => (page === 'qat' ? addQat(c.id) : add(c.id))}
            {...dragProps({ kind: 'new', id: c.id })}
          >
            {c.label}
          </li>
        ))}
      </ul>
    </div>
  );

  const setQat = (q: UiCustomization['qat']) => onChange({ ...value, qat: q });
  const addQat = (id: string | null) => {
    if (!id) return;
    const at = qatSel ? qat.indexOf(qatSel) + 1 : undefined;
    setQat(qatAdd(value.qat, id, at));
    setQatSel(id);
  };
  const qi = qatSel ? qat.indexOf(qatSel) : -1;

  const ribbonPage = (
    <div style={{ display: 'flex', gap: 8 }}>
      {commandList}
      <div style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 6 }}>
        <button
          type="button"
          data-testid="customize-add"
          disabled={!picked || !customGroup}
          onClick={() => add(picked)}
        >
          {t('customize.add')}
        </button>
        <button type="button" data-testid="customize-remove" disabled={!canRemove} onClick={remove}>
          {t('customize.remove')}
        </button>
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <ul
          ref={treeRef}
          role="tree"
          aria-label={t('customize.ribbonTree')}
          data-testid="customize-tree"
          style={listStyle}
          onKeyDown={onTreeKey}
        >
          {tree.map(renderTab)}
        </ul>
        {renaming !== null && sel && sel.kind !== 'cmd' ? (
          <form
            style={{ display: 'flex', gap: 4, marginTop: 4 }}
            onSubmit={(e) => {
              e.preventDefault();
              setRibbon(renameNode(ribbon, builtins, sel, renaming));
              setRenaming(null);
            }}
          >
            <input
              autoFocus
              aria-label={t('customize.renameLabel')}
              data-testid="customize-rename-input"
              value={renaming}
              maxLength={100}
              onChange={(e) => setRenaming(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  e.stopPropagation();
                  setRenaming(null);
                }
              }}
              style={{ flex: 1 }}
            />
            <button type="submit" data-testid="customize-rename-ok" disabled={!renaming.trim()}>
              {t('customize.ok')}
            </button>
            <button type="button" onClick={() => setRenaming(null)}>
              {t('customize.cancel')}
            </button>
          </form>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 4 }}>
            <button type="button" data-testid="customize-new-tab" onClick={newTab}>
              {t('customize.newTab')}
            </button>
            <button
              type="button"
              data-testid="customize-new-group"
              disabled={!sel}
              onClick={newGroup}
            >
              {t('customize.newGroup')}
            </button>
            <button
              type="button"
              data-testid="customize-rename"
              disabled={!sel || sel.kind === 'cmd'}
              onClick={() => setRenaming(nodeLabel(sel))}
            >
              {t('customize.rename')}
            </button>
            <button
              type="button"
              data-testid="customize-up"
              aria-label={t('customize.up')}
              title={t('customize.up')}
              disabled={!sel || pos <= 0}
              onClick={() => sel && moveTo(sel, pos - 1)}
            >
              ↑
            </button>
            <button
              type="button"
              data-testid="customize-down"
              aria-label={t('customize.down')}
              title={t('customize.down')}
              disabled={!sel || pos < 0 || pos >= count - 1}
              onClick={() => sel && moveTo(sel, pos + 1)}
            >
              ↓
            </button>
            <button
              type="button"
              data-testid="customize-reset-tab"
              disabled={!selTab?.builtIn}
              onClick={() => selTab && setRibbon(resetTab(ribbon, selTab.id))}
            >
              {t('customize.resetTab')}
            </button>
            <button
              type="button"
              data-testid="customize-reset-all"
              onClick={() => {
                setRibbon({});
                setSel(null);
              }}
            >
              {t('customize.resetAll')}
            </button>
          </div>
        )}
      </div>
    </div>
  );

  const qatPage = (
    <div style={{ display: 'flex', gap: 8 }}>
      {commandList}
      <div style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 6 }}>
        <button
          type="button"
          data-testid="customize-qat-add"
          disabled={!picked || qat.includes(picked)}
          onClick={() => addQat(picked)}
        >
          {t('customize.add')}
        </button>
        <button
          type="button"
          data-testid="customize-qat-remove"
          disabled={!qatSel}
          onClick={() => {
            if (!qatSel) return;
            setQat(qatRemove(value.qat, qatSel));
            setQatSel(null);
          }}
        >
          {t('customize.remove')}
        </button>
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <ul
          role="listbox"
          aria-label={t('customize.qatList')}
          data-testid="customize-qat-list"
          style={listStyle}
          onKeyDown={(e) => {
            if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
            e.preventDefault();
            const n =
              qat[Math.max(0, Math.min(qat.length - 1, qi + (e.key === 'ArrowDown' ? 1 : -1)))];
            if (n) setQatSel(n);
          }}
          tabIndex={0}
        >
          {qat.map((id) => (
            <li
              key={id}
              role="option"
              aria-selected={qatSel === id}
              data-qat-option={id}
              style={lineStyle(0, qatSel === id)}
              onClick={() => setQatSel(id)}
              {...dragProps({ kind: 'qat', id })}
              onDragOver={(e) => {
                if (drag.current) e.preventDefault();
              }}
              onDrop={(e) => {
                const d = drag.current;
                drag.current = null;
                if (!d) return;
                e.preventDefault();
                if (d.kind === 'qat') setQat(qatMove(value.qat, d.id, qat.indexOf(id)));
                else if (d.kind === 'new') setQat(qatAdd(value.qat, d.id, qat.indexOf(id)));
              }}
            >
              {labelOf(id)}
            </li>
          ))}
        </ul>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 4 }}>
          <button
            type="button"
            data-testid="customize-qat-up"
            aria-label={t('customize.up')}
            title={t('customize.up')}
            disabled={qi <= 0}
            onClick={() => qatSel && setQat(qatMove(value.qat, qatSel, qi - 1))}
          >
            ↑
          </button>
          <button
            type="button"
            data-testid="customize-qat-down"
            aria-label={t('customize.down')}
            title={t('customize.down')}
            disabled={qi < 0 || qi >= qat.length - 1}
            onClick={() => qatSel && setQat(qatMove(value.qat, qatSel, qi + 1))}
          >
            ↓
          </button>
          <button
            type="button"
            data-testid="customize-qat-reset"
            onClick={() => setQat({ below: value.qat.below })}
          >
            {t('customize.resetQat')}
          </button>
        </div>
        <label style={{ display: 'block', marginTop: 6, fontSize: 13 }}>
          <input
            type="checkbox"
            data-testid="customize-qat-below"
            checked={!!value.qat.below}
            onChange={(e) => setQat({ ...value.qat, below: e.target.checked || undefined })}
          />{' '}
          {t('customize.below')}
        </label>
      </div>
    </div>
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => {
        if (!o) onClose();
      }}
      title={t(page === 'qat' ? 'customize.titleQat' : 'customize.title')}
      width={760}
      testId="customize-dialog"
      footer={
        <>
          <button type="button" data-testid="customize-export" onClick={onExport}>
            {t('customize.export')}
          </button>
          <button type="button" data-testid="customize-import" onClick={onImport}>
            {t('customize.import')}
          </button>
          <button type="button" data-testid="customize-close" onClick={onClose}>
            {t('customize.close')}
          </button>
        </>
      }
    >
      <div
        role="tablist"
        aria-label={t('customize.title')}
        style={{ display: 'flex', gap: 4, marginBottom: 8 }}
      >
        {(['ribbon', 'qat'] as const).map((p) => (
          <button
            key={p}
            type="button"
            role="tab"
            aria-selected={page === p}
            data-testid={`customize-page-${p}`}
            onClick={() => onPage(p)}
            style={{ fontWeight: page === p ? 600 : undefined }}
          >
            {t(p === 'qat' ? 'customize.pageQat' : 'customize.pageRibbon')}
          </button>
        ))}
      </div>
      <div role="tabpanel">{page === 'qat' ? qatPage : ribbonPage}</div>
      {notice && (
        <p role="status" data-testid="customize-notice" style={{ fontSize: 12 }}>
          {notice}
        </p>
      )}
    </Dialog>
  );
}

function lineStyle(level: number, selected: boolean) {
  return {
    padding: `2px 4px 2px ${4 + level * 16}px`,
    cursor: 'default',
    whiteSpace: 'nowrap' as const,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    background: selected ? 'var(--bo-selected, var(--bo-hover))' : undefined,
    outlineOffset: -2,
  };
}

function Twisty({
  expanded,
  label,
  onClick,
}: {
  expanded: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={label}
      data-testid="customize-twisty"
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      style={{ all: 'unset', width: 14, display: 'inline-block', textAlign: 'center' }}
    >
      <span aria-hidden="true">{expanded ? '▾' : '▸'}</span>
    </button>
  );
}

function Muted({ text }: { text: string }) {
  return <span style={{ marginLeft: 4, fontSize: 11, opacity: 0.7 }}>{text}</span>;
}

// ---- Hook ---------------------------------------------------------------------------------

export interface RibbonCustomizationOptions<S> {
  app: ShortcutApp;
  registry: CommandRegistry<S>;
  /** Where the customisation is kept (`platform.shell`). */
  store: SettingStore;
  files: Pick<FileAccess, 'openFile' | 'saveFile'>;
  /** Every ribbon tab the app builds (regular, contextual, File), read when needed. */
  tabs: () => readonly RibbonTab[];
  /** Commands besides the ribbon's (with a shortcut, see `shortcutCommands`). */
  extraCommands: () => readonly ShortcutCommand[];
  /** The custom shortcuts (`useCustomShortcuts`): exported and imported with the ribbon. */
  shortcuts: { map: ShortcutMap; apply: (m: ShortcutMap) => void };
  /** State of a command (Quick Access Toolbar, custom groups of registry commands). */
  enabled: (id: string) => boolean;
  run: (id: string) => void;
}

/** Ribbon tabs of the tree: the regular ones (the File tab is the File screen). */
const treeTabs = (tabs: readonly RibbonTab[]) => tabs.filter((x) => !x.context && x.id !== 'file');

/** The app's commands for the dialog: ribbon commands (with their tab), then the others. */
export function customizeCommands(
  tabs: readonly RibbonTab[],
  extra: readonly ShortcutCommand[],
): CustomizeCommand[] {
  const out: CustomizeCommand[] = [];
  const seen = new Set<string>();
  for (const tab of tabs)
    for (const e of flattenRibbon([tab])) {
      if (seen.has(e.id) || e.id.startsWith('qat:')) continue;
      seen.add(e.id);
      out.push({ id: e.id, label: e.label, path: e.path, tab: tab.id });
    }
  for (const c of extra)
    if (!seen.has(c.id)) {
      seen.add(c.id);
      out.push({ id: c.id, label: c.label, path: c.path });
    }
  return out;
}

/**
 * The user's ribbon and Quick Access Toolbar of an app (CORE-UI-005): loaded on start, changed
 * through the dialog (saved at once), opened by `options.customizeRibbon` /
 * `options.customizeQat` (registered here) and the ribbon's right-click menu.
 */
export function useRibbonCustomization<S>(o: RibbonCustomizationOptions<S>) {
  const { t } = useTranslation();
  const { app, registry } = o;
  const [value, setValue] = useState<UiCustomization>({ ribbon: {}, qat: {} });
  const [page, setPage] = useState<CustomizePage | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const opts = useRef(o);
  useEffect(() => {
    opts.current = o;
  });
  useEffect(() => {
    let live = true;
    void loadCustomization(opts.current.store, app).then((v) => {
      if (live) setValue(v);
    });
    return () => {
      live = false;
    };
  }, [app]);
  const apply = useCallback(
    (v: UiCustomization) => {
      setValue(v);
      void saveCustomization(opts.current.store, app, v).catch(() => {});
    },
    [app],
  );
  useEffect(() => {
    const offs = [
      registry.register({ id: 'options.customizeRibbon', run: () => setPage('ribbon') }),
      registry.register({ id: 'options.customizeQat', run: () => setPage('qat') }),
    ];
    return () => offs.forEach((off) => off());
  }, [registry]);

  const qat = qatCommands(value.qat);
  const toggleQat = (id: string) =>
    apply({ ...value, qat: qat.includes(id) ? qatRemove(value.qat, id) : qatAdd(value.qat, id) });

  /** Props for `<Ribbon>`: the customisation, registry commands of custom groups, the menu. */
  const ribbonProps = () => ({
    customization: value.ribbon,
    resolveItem: (id: string): RibbonItem | undefined => {
      if (!registry.has(id)) return undefined;
      const c = customizeCommands([], o.extraCommands()).find((x) => x.id === id);
      return {
        id,
        label: c?.label ?? id,
        size: 'small',
        enabled: o.enabled(id),
        shortcut: registry.shortcut(id),
        onSelect: () => opts.current.run(id),
      };
    },
    customizeMenu: {
      labels: {
        menu: t('customize.menu'),
        customizeRibbon: t('commands.options.customizeRibbon'),
        customizeQat: t('commands.options.customizeQat'),
        addToQat: t('customize.addToQat'),
        removeFromQat: t('customize.removeFromQat'),
      },
      onCustomizeRibbon: () => setPage('ribbon'),
      onCustomizeQat: () => setPage('qat'),
      inQat: (id: string) => qat.includes(id),
      onToggleQat: toggleQat,
    } satisfies RibbonCustomizeMenu,
  });

  /** The toolbar's buttons, labelled from the ribbon `tabs`, then `extra` (bound scripts). */
  const quickAccess = (tabs: readonly RibbonTab[], extra: readonly QuickAccessItem[] = []) => {
    const onRibbon = ribbonItems(tabs);
    const others = new Map(o.extraCommands().map((c) => [c.id, c.label]));
    const items: QuickAccessItem[] = [];
    for (const id of qat) {
      const ri = onRibbon.get(id);
      if (ri) {
        items.push({
          id,
          label: ri.label,
          shortcut: registry.shortcut(id) ?? ri.shortcut,
          enabled: ri.enabled !== false && (!registry.has(id) || o.enabled(id)),
          onSelect: () => (registry.has(id) ? opts.current.run(id) : ri.onSelect()),
        });
      } else if (registry.has(id))
        items.push({
          id,
          label: others.get(id) ?? i18nHas(t, `commands.${id}`) ?? id,
          shortcut: registry.shortcut(id),
          enabled: o.enabled(id),
          onSelect: () => opts.current.run(id),
        });
    }
    return [...items, ...extra];
  };

  const close = () => {
    setPage(null);
    setNotice(null);
  };
  let element = null;
  if (page) {
    const all = o.tabs();
    const builtins = treeTabs(all);
    const commands = customizeCommands(all, o.extraCommands());
    const listed = new Set(commands.map((c) => c.id));
    const known = (id: string) => listed.has(id) || registry.has(id);
    element = (
      <RibbonCustomizeDialog
        page={page}
        onPage={setPage}
        onClose={close}
        builtins={builtins}
        commands={commands}
        filterTabs={all
          .filter((x) => x.groups.length > 0)
          .map((x) => ({ id: x.id, label: x.label }))
          .filter((x, i, l) => l.findIndex((y) => y.id === x.id) === i)}
        value={value}
        onChange={apply}
        notice={notice}
        onExport={() => {
          const text = exportCustomization(app, value, o.shortcuts.map);
          void o.files
            .saveFile(
              new TextEncoder().encode(text),
              `blueoffice-customization-${app}.json`,
              'application/json',
            )
            .then((saved) => saved !== null && setNotice(t('customize.exported')))
            .catch(() => setNotice(t('customize.exportFailed')));
        }}
        onImport={() => {
          void o.files
            .openFile(['.json'])
            .then((f) => {
              if (!f) return;
              const r = importCustomization(new TextDecoder().decode(f.bytes), app, known, value);
              if (!r.ok) {
                setNotice(
                  t(
                    r.error === 'wrongApp'
                      ? 'customize.importWrongApp'
                      : r.error === 'newer'
                        ? 'customize.importNewer'
                        : 'customize.importInvalid',
                  ),
                );
                return;
              }
              apply(r.value);
              opts.current.shortcuts.apply(r.shortcuts);
              setNotice(
                r.ignored.length
                  ? t('customize.importedIgnored', {
                      count: r.ignored.length,
                      ids: r.ignored.slice(0, 5).join(', '),
                    })
                  : t('customize.imported'),
              );
            })
            .catch(() => setNotice(t('customize.importInvalid')));
        }}
      />
    );
  }
  return {
    value,
    element,
    open: setPage,
    ribbonProps,
    quickAccess,
    below: !!value.qat.below,
    qatLabel: t('titleBar.quickAccess'),
  };
}

/** Translation of `key` when it exists (i18next returns the key itself otherwise). */
function i18nHas(t: (k: string) => string, key: string): string | null {
  const v = t(key);
  return v && v !== key ? v : null;
}
