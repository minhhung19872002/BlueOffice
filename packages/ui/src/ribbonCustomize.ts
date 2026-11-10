/**
 * Ribbon and Quick Access Toolbar customisation (CORE-UI-005). The user's changes are kept as
 * a small diff over the ribbon the app builds — order lists, hidden ids, new labels and the
 * user's own tabs / groups — so commands, groups and tabs the app adds later still appear
 * (at their default place). Contextual tabs are never changed. Built-in tabs and groups can be
 * hidden, renamed and reordered but not deleted; custom groups hold command ids.
 */
import type { RibbonGroup, RibbonItem, RibbonTab } from './Ribbon';

export interface CustomGroup {
  id: string;
  label: string;
  /** Command ids, in order; ids the app does not have are skipped when rendering. */
  commands: string[];
}

export interface CustomTab {
  id: string;
  label: string;
}

export interface RibbonCustomization {
  /** Order of the regular tabs (built-in and custom ids). */
  tabOrder?: string[];
  /** Order of the groups of each tab (built-in and custom group ids). */
  groupOrder?: Record<string, string[]>;
  /** Hidden tabs / groups ({@link nodeKey}). */
  hidden?: string[];
  /** New labels of built-in tabs / groups ({@link nodeKey}). */
  labels?: Record<string, string>;
  /** The user's tabs. */
  tabs?: CustomTab[];
  /** The user's groups, by tab id (built-in or custom tab). */
  groups?: Record<string, CustomGroup[]>;
}

export interface QatCustomization {
  /** Command ids; undefined = the default ones. */
  commands?: string[];
  /** Shown below the ribbon instead of in the title bar. */
  below?: boolean;
}

/** Commands of the Quick Access Toolbar when the user has not changed it. */
export const DEFAULT_QAT: readonly string[] = ['file.save', 'edit.undo', 'edit.redo'];

/** A tab or a group of the customisation tree. */
export type RibbonNode = { kind: 'tab'; id: string } | { kind: 'group'; tab: string; id: string };

/** `tab:<id>` / `group:<tab>/<id>`. */
export function nodeKey(n: RibbonNode): string {
  return n.kind === 'tab' ? `tab:${n.id}` : `group:${n.tab}/${n.id}`;
}

const CUSTOM_PREFIX = 'custom.';
export const isCustomId = (id: string) => id.startsWith(CUSTOM_PREFIX);

/** Smallest free `custom.<kind><n>` id. */
function freeId(c: RibbonCustomization, kind: 'tab' | 'group'): string {
  const used = new Set<string>([
    ...(c.tabs ?? []).map((t) => t.id),
    ...Object.values(c.groups ?? {}).flatMap((gs) => gs.map((g) => g.id)),
  ]);
  for (let n = 1; ; n++) {
    const id = `${CUSTOM_PREFIX}${kind}${n}`;
    if (!used.has(id)) return id;
  }
}

/**
 * `stored` with unknown / repeated ids dropped, the `defaults` it lacks inserted after their
 * preceding default (new built-ins appear where the app puts them), then the missing `extras`.
 */
export function mergeOrder(
  defaults: readonly string[],
  stored: readonly string[] | undefined,
  extras: readonly string[] = [],
): string[] {
  const known = new Set([...defaults, ...extras]);
  const out: string[] = [];
  for (const id of stored ?? []) if (known.has(id) && !out.includes(id)) out.push(id);
  defaults.forEach((id, i) => {
    if (out.includes(id)) return;
    let at = 0;
    for (let j = i - 1; j >= 0; j--) {
      const k = out.indexOf(defaults[j] ?? '');
      if (k >= 0) {
        at = k + 1;
        break;
      }
    }
    out.splice(at, 0, id);
  });
  for (const id of extras) if (!out.includes(id)) out.push(id);
  return out;
}

const plainTabs = (tabs: readonly RibbonTab[]) => tabs.filter((t) => !t.context);

/** One entry of the customisation tree (hidden ones included). */
export interface TreeGroup {
  id: string;
  tab: string;
  label: string;
  builtIn: boolean;
  hidden: boolean;
  /** Commands of the group: its items (built-in) or command ids (custom). */
  commands: string[];
}

export interface TreeTab {
  id: string;
  label: string;
  builtIn: boolean;
  hidden: boolean;
  groups: TreeGroup[];
}

/** The regular tabs with their groups as customised, hidden ones included (dialog tree). */
export function customizationTree(
  builtins: readonly RibbonTab[],
  c: RibbonCustomization,
): TreeTab[] {
  const plain = plainTabs(builtins);
  const hidden = new Set(c.hidden ?? []);
  const customTabs = c.tabs ?? [];
  const order = mergeOrder(
    plain.map((t) => t.id),
    c.tabOrder,
    customTabs.map((t) => t.id),
  );
  return order.map((id) => {
    const b = plain.find((t) => t.id === id);
    const custom = customTabs.find((t) => t.id === id);
    const tabKey = nodeKey({ kind: 'tab', id });
    const own = c.groups?.[id] ?? [];
    const builtGroups = b?.groups ?? [];
    const gOrder = mergeOrder(
      builtGroups.map((g) => g.id),
      c.groupOrder?.[id],
      own.map((g) => g.id),
    );
    const groups = gOrder.map((gid): TreeGroup => {
      const key = nodeKey({ kind: 'group', tab: id, id: gid });
      const bg = builtGroups.find((g) => g.id === gid);
      if (bg)
        return {
          id: gid,
          tab: id,
          label: c.labels?.[key] ?? bg.label,
          builtIn: true,
          hidden: hidden.has(key),
          commands: bg.items.map((i) => i.id),
        };
      const cg = own.find((g) => g.id === gid);
      return {
        id: gid,
        tab: id,
        label: cg?.label ?? gid,
        builtIn: false,
        hidden: hidden.has(key),
        commands: [...(cg?.commands ?? [])],
      };
    });
    return {
      id,
      label: b ? (c.labels?.[tabKey] ?? b.label) : (custom?.label ?? id),
      builtIn: !!b,
      hidden: hidden.has(tabKey),
      groups,
    };
  });
}

/** Every command item of the ribbon by id (first occurrence; custom controls skipped). */
export function ribbonItems(tabs: readonly RibbonTab[]): Map<string, RibbonItem> {
  const m = new Map<string, RibbonItem>();
  for (const t of tabs)
    for (const g of t.groups)
      for (const i of g.items) if (!i.render && !m.has(i.id)) m.set(i.id, i);
  return m;
}

/**
 * The tabs the ribbon shows: regular tabs as customised (order, hidden, labels, the user's
 * tabs and groups), then the contextual tabs unchanged. Commands of custom groups come from
 * the ribbon's own items, else from `resolve`; unknown ones are skipped.
 */
export function applyRibbonCustomization(
  tabs: readonly RibbonTab[],
  c: RibbonCustomization | null | undefined,
  resolve?: (id: string) => RibbonItem | undefined,
): RibbonTab[] {
  if (!c || isEmptyCustomization(c)) return [...tabs];
  const items = ribbonItems(tabs);
  const item = (id: string) => items.get(id) ?? resolve?.(id);
  const out: RibbonTab[] = [];
  for (const t of customizationTree(tabs, c)) {
    if (t.hidden) continue;
    const built = tabs.find((x) => x.id === t.id && !x.context);
    const groups: RibbonGroup[] = [];
    for (const g of t.groups) {
      if (g.hidden) continue;
      const bg = built?.groups.find((x) => x.id === g.id);
      if (bg && g.builtIn) {
        groups.push(g.label === bg.label ? bg : { ...bg, label: g.label });
        continue;
      }
      const list: RibbonItem[] = [];
      for (const id of g.commands) {
        const i = item(id);
        // The user's order decides what the simplified row keeps (first commands).
        if (i && !list.some((x) => x.id === id)) list.push({ ...i, simplified: undefined });
      }
      groups.push({ id: g.id, label: g.label, items: list });
    }
    if (built && t.label === built.label && groups.length === built.groups.length) {
      const same = groups.every((g, n) => g === built.groups[n]);
      out.push(same ? built : { ...built, groups });
    } else out.push({ ...(built ?? {}), id: t.id, label: t.label, groups });
  }
  for (const t of tabs) if (t.context) out.push(t);
  return out;
}

export function isEmptyCustomization(c: RibbonCustomization): boolean {
  return (
    !c.tabOrder?.length &&
    !Object.keys(c.groupOrder ?? {}).length &&
    !c.hidden?.length &&
    !Object.keys(c.labels ?? {}).length &&
    !c.tabs?.length &&
    !Object.values(c.groups ?? {}).some((g) => g.length)
  );
}

/** Drops empty fields (stored and exported form). */
export function compactCustomization(c: RibbonCustomization): RibbonCustomization {
  const out: RibbonCustomization = {};
  if (c.tabOrder?.length) out.tabOrder = [...c.tabOrder];
  const go = Object.entries(c.groupOrder ?? {}).filter(([, v]) => v.length);
  if (go.length) out.groupOrder = Object.fromEntries(go);
  if (c.hidden?.length) out.hidden = [...new Set(c.hidden)];
  if (Object.keys(c.labels ?? {}).length) out.labels = { ...c.labels };
  if (c.tabs?.length) out.tabs = c.tabs.map((t) => ({ ...t }));
  const gs = Object.entries(c.groups ?? {}).filter(([, v]) => v.length);
  if (gs.length) out.groups = Object.fromEntries(gs);
  return out;
}

// ---- Edits (each returns a new customisation) -------------------------------------------

/** Adds a tab (with one empty group, as a new tab needs somewhere to put commands). */
export function addCustomTab(
  c: RibbonCustomization,
  builtins: readonly RibbonTab[],
  label: string,
  groupLabel: string,
  after?: string,
): { value: RibbonCustomization; tab: string; group: string } {
  const tab = freeId(c, 'tab');
  const withTab: RibbonCustomization = { ...c, tabs: [...(c.tabs ?? []), { id: tab, label }] };
  const group = freeId(withTab, 'group');
  const value: RibbonCustomization = {
    ...withTab,
    groups: { ...c.groups, [tab]: [{ id: group, label: groupLabel, commands: [] }] },
  };
  if (after !== undefined) {
    const order = customizationTree(builtins, c).map((t) => t.id);
    const at = order.indexOf(after);
    order.splice(at < 0 ? order.length : at + 1, 0, tab);
    value.tabOrder = order;
  }
  return { value, tab, group };
}

export function addCustomGroup(
  c: RibbonCustomization,
  tab: string,
  label: string,
): { value: RibbonCustomization; group: string } {
  const group = freeId(c, 'group');
  const list = [...(c.groups?.[tab] ?? []), { id: group, label, commands: [] }];
  return { value: { ...c, groups: { ...c.groups, [tab]: list } }, group };
}

export function renameNode(
  c: RibbonCustomization,
  builtins: readonly RibbonTab[],
  n: RibbonNode,
  label: string,
): RibbonCustomization {
  const text = label.trim();
  if (!text) return c;
  if (n.kind === 'tab' && isCustomId(n.id))
    return { ...c, tabs: (c.tabs ?? []).map((t) => (t.id === n.id ? { ...t, label: text } : t)) };
  if (n.kind === 'group' && isCustomId(n.id))
    return updateGroup(c, n.tab, n.id, (g) => ({ ...g, label: text }));
  const key = nodeKey(n);
  const original =
    n.kind === 'tab'
      ? builtins.find((t) => t.id === n.id)?.label
      : builtins.find((t) => t.id === n.tab)?.groups.find((g) => g.id === n.id)?.label;
  const labels = { ...c.labels };
  if (text === original) delete labels[key];
  else labels[key] = text;
  return { ...c, labels };
}

export function setNodeHidden(
  c: RibbonCustomization,
  n: RibbonNode,
  hidden: boolean,
): RibbonCustomization {
  const key = nodeKey(n);
  const rest = (c.hidden ?? []).filter((k) => k !== key);
  return { ...c, hidden: hidden ? [...rest, key] : rest };
}

/** Deletes a custom tab (with its groups) or group; built-in ones can only be hidden. */
export function removeCustomNode(c: RibbonCustomization, n: RibbonNode): RibbonCustomization {
  if (!isCustomId(n.id)) return c;
  const groups = { ...c.groups };
  const drop = (key: string) => (c.hidden ?? []).filter((k) => k !== key);
  if (n.kind === 'tab') {
    delete groups[n.id];
    const groupOrder = { ...c.groupOrder };
    delete groupOrder[n.id];
    return {
      ...c,
      tabs: (c.tabs ?? []).filter((t) => t.id !== n.id),
      groups,
      groupOrder,
      tabOrder: c.tabOrder?.filter((id) => id !== n.id),
      hidden: drop(nodeKey(n)).filter((k) => !k.startsWith(`group:${n.id}/`)),
    };
  }
  groups[n.tab] = (groups[n.tab] ?? []).filter((g) => g.id !== n.id);
  return { ...c, groups, hidden: drop(nodeKey(n)) };
}

function moveIn<T>(list: readonly T[], from: number, to: number): T[] {
  const out = [...list];
  if (from < 0 || from >= out.length) return out;
  const [x] = out.splice(from, 1);
  out.splice(Math.max(0, Math.min(to, out.length)), 0, x as T);
  return out;
}

/** Moves a tab among the regular tabs, or a group among the groups of its tab, to `index`. */
export function moveNode(
  c: RibbonCustomization,
  builtins: readonly RibbonTab[],
  n: RibbonNode,
  index: number,
): RibbonCustomization {
  const tree = customizationTree(builtins, c);
  if (n.kind === 'tab') {
    const order = tree.map((t) => t.id);
    return { ...c, tabOrder: moveIn(order, order.indexOf(n.id), index) };
  }
  const order = (tree.find((t) => t.id === n.tab)?.groups ?? []).map((g) => g.id);
  return {
    ...c,
    groupOrder: { ...c.groupOrder, [n.tab]: moveIn(order, order.indexOf(n.id), index) },
  };
}

function updateGroup(
  c: RibbonCustomization,
  tab: string,
  group: string,
  f: (g: CustomGroup) => CustomGroup,
): RibbonCustomization {
  const list = c.groups?.[tab];
  if (!list?.some((g) => g.id === group)) return c;
  return { ...c, groups: { ...c.groups, [tab]: list.map((g) => (g.id === group ? f(g) : g)) } };
}

/** Adds a command to a custom group (at `index`, default the end); a group holds it once. */
export function addGroupCommand(
  c: RibbonCustomization,
  tab: string,
  group: string,
  command: string,
  index?: number,
): RibbonCustomization {
  return updateGroup(c, tab, group, (g) => {
    if (g.commands.includes(command)) return g;
    const commands = [...g.commands];
    commands.splice(index ?? commands.length, 0, command);
    return { ...g, commands };
  });
}

export function removeGroupCommand(
  c: RibbonCustomization,
  tab: string,
  group: string,
  command: string,
): RibbonCustomization {
  return updateGroup(c, tab, group, (g) => ({
    ...g,
    commands: g.commands.filter((x) => x !== command),
  }));
}

export function moveGroupCommand(
  c: RibbonCustomization,
  tab: string,
  group: string,
  command: string,
  index: number,
): RibbonCustomization {
  return updateGroup(c, tab, group, (g) => ({
    ...g,
    commands: moveIn(g.commands, g.commands.indexOf(command), index),
  }));
}

/**
 * Back to the app's version of a built-in tab: its label, visibility, group order, hidden /
 * renamed groups and the custom groups added to it. Its position among the tabs is kept.
 */
export function resetTab(c: RibbonCustomization, tab: string): RibbonCustomization {
  const tabKey = nodeKey({ kind: 'tab', id: tab });
  const ofTab = (k: string) => k === tabKey || k.startsWith(`group:${tab}/`);
  const labels = Object.fromEntries(Object.entries(c.labels ?? {}).filter(([k]) => !ofTab(k)));
  const groupOrder = { ...c.groupOrder };
  delete groupOrder[tab];
  const groups = { ...c.groups };
  if (!isCustomId(tab)) delete groups[tab];
  return { ...c, labels, groupOrder, groups, hidden: (c.hidden ?? []).filter((k) => !ofTab(k)) };
}

// ---- Quick Access Toolbar ---------------------------------------------------------------

export const qatCommands = (q: QatCustomization | null | undefined): string[] => [
  ...(q?.commands ?? DEFAULT_QAT),
];

export function qatAdd(q: QatCustomization, id: string, index?: number): QatCustomization {
  const list = qatCommands(q);
  if (list.includes(id)) return q;
  list.splice(index ?? list.length, 0, id);
  return { ...q, commands: list };
}

export function qatRemove(q: QatCustomization, id: string): QatCustomization {
  return { ...q, commands: qatCommands(q).filter((x) => x !== id) };
}

export function qatMove(q: QatCustomization, id: string, index: number): QatCustomization {
  const list = qatCommands(q);
  return { ...q, commands: moveIn(list, list.indexOf(id), index) };
}

// ---- Validation (stored / imported data) ------------------------------------------------

const ID = /^[\w.:-]{1,200}$/;
const KEY = /^(tab:[\w.:-]{1,200}|group:[\w.:-]{1,200}\/[\w.:-]{1,200})$/;
const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);
const label = (v: unknown): string | null =>
  typeof v === 'string' && v.trim() ? v.trim().slice(0, 100) : null;
const ids = (v: unknown, re = ID): string[] =>
  Array.isArray(v)
    ? [...new Set(v.filter((x): x is string => typeof x === 'string' && re.test(x)))]
    : [];

/**
 * The customisation in `v` with malformed parts dropped; command ids `known` rejects are left
 * out of custom groups and reported in `ignored`.
 */
export function sanitizeCustomization(
  v: unknown,
  known?: (command: string) => boolean,
): { value: RibbonCustomization; ignored: string[] } {
  const ignored: string[] = [];
  if (!isObj(v)) return { value: {}, ignored };
  const out: RibbonCustomization = {};
  const order = ids(v.tabOrder);
  if (order.length) out.tabOrder = order;
  if (isObj(v.groupOrder)) {
    const go: Record<string, string[]> = {};
    for (const [t, l] of Object.entries(v.groupOrder))
      if (ID.test(t) && ids(l).length) go[t] = ids(l);
    if (Object.keys(go).length) out.groupOrder = go;
  }
  const hidden = ids(v.hidden, KEY);
  if (hidden.length) out.hidden = hidden;
  if (isObj(v.labels)) {
    const ls: Record<string, string> = {};
    for (const [k, l] of Object.entries(v.labels)) {
      const text = label(l);
      if (KEY.test(k) && text) ls[k] = text;
    }
    if (Object.keys(ls).length) out.labels = ls;
  }
  const seen = new Set<string>();
  if (Array.isArray(v.tabs)) {
    const tabs: CustomTab[] = [];
    for (const t of v.tabs) {
      if (!isObj(t) || typeof t.id !== 'string' || !isCustomId(t.id) || !ID.test(t.id)) continue;
      const text = label(t.label);
      if (!text || seen.has(t.id)) continue;
      seen.add(t.id);
      tabs.push({ id: t.id, label: text });
    }
    if (tabs.length) out.tabs = tabs;
  }
  if (isObj(v.groups)) {
    const groups: Record<string, CustomGroup[]> = {};
    for (const [tab, list] of Object.entries(v.groups)) {
      if (!ID.test(tab) || !Array.isArray(list)) continue;
      if (isCustomId(tab) && !seen.has(tab)) continue;
      const gs: CustomGroup[] = [];
      for (const g of list) {
        if (!isObj(g) || typeof g.id !== 'string' || !isCustomId(g.id) || !ID.test(g.id)) continue;
        const text = label(g.label);
        if (!text || seen.has(g.id)) continue;
        seen.add(g.id);
        const commands = ids(g.commands).filter((id) => {
          if (!known || known(id)) return true;
          ignored.push(id);
          return false;
        });
        gs.push({ id: g.id, label: text, commands });
      }
      if (gs.length) groups[tab] = gs;
    }
    if (Object.keys(groups).length) out.groups = groups;
  }
  return { value: out, ignored };
}

export function sanitizeQat(
  v: unknown,
  known?: (command: string) => boolean,
): { value: QatCustomization; ignored: string[] } {
  const ignored: string[] = [];
  if (!isObj(v)) return { value: {}, ignored };
  const out: QatCustomization = {};
  if (Array.isArray(v.commands))
    out.commands = ids(v.commands).filter((id) => {
      if (!known || known(id)) return true;
      ignored.push(id);
      return false;
    });
  if (v.below === true) out.below = true;
  return { value: out, ignored };
}
