/**
 * Context menus of the three apps (CORE-UI-003): which registry commands a right click (or
 * Shift+F10) offers for what is under the pointer. The menus are data; the commands, labels,
 * enabled / checked state and shortcuts come from the ribbon items (which read the command
 * registry), so a menu never runs something the ribbon would refuse.
 */
import type { MenuEntry, MenuLine } from './ContextMenu';
import type { RibbonTab } from './Ribbon';

/** `-` separates groups of a menu. */
export type MenuSpec = readonly string[];

export type DocMenuContext =
  'text' | 'table' | 'picture' | 'shape' | 'header' | 'comment' | 'spelling';
export type SheetMenuContext =
  'cell' | 'rowHeader' | 'columnHeader' | 'sheetTab' | 'chart' | 'picture';
export type SlideMenuContext =
  'shape' | 'text' | 'picture' | 'table' | 'chart' | 'thumbnail' | 'canvas';

const CLIPBOARD = ['edit.cut', 'edit.copy', 'edit.paste'] as const;

export const DOC_MENUS: Record<DocMenuContext, MenuSpec> = {
  text: [
    ...CLIPBOARD,
    'edit.pasteSpecial',
    '-',
    'format.fontDialog',
    'format.clear',
    'format.bullets',
    'format.numbering',
    '-',
    'review.newComment',
    'references.insertCaption',
    'review.language',
  ],
  // The spelling suggestions come first (added by the editor), then the text commands.
  spelling: [...CLIPBOARD, '-', 'format.fontDialog', 'review.newComment', 'review.language'],
  table: [
    ...CLIPBOARD,
    '-',
    'table.insertRowAbove',
    'table.insertRowBelow',
    'table.insertColumnLeft',
    'table.insertColumnRight',
    '-',
    'table.deleteRow',
    'table.deleteColumn',
    'table.deleteCell',
    'table.deleteTable',
    '-',
    'table.mergeCells',
    'table.splitCell',
    '-',
    'table.textDirection',
    'table.autoFitContents',
    'table.properties',
  ],
  picture: [
    ...CLIPBOARD,
    '-',
    'picture.change',
    'picture.reset',
    'picture.customCrop',
    'picture.adjust',
    'picture.position',
    '-',
    'references.insertCaption',
    'review.newComment',
  ],
  shape: [
    ...CLIPBOARD,
    '-',
    'smartart.edit',
    'object.edit',
    'object.updateLinks',
    'picture.position',
  ],
  header: [
    ...CLIPBOARD,
    '-',
    'insert.pageNumber',
    'hf.linkToPrevious',
    '-',
    'insert.header',
    'insert.footer',
    'hf.close',
  ],
  comment: [
    'review.resolveComment',
    'review.deleteComment',
    '-',
    'review.previousComment',
    'review.nextComment',
    '-',
    'review.newComment',
    'review.comments',
  ],
};

export const SHEET_MENUS: Record<SheetMenuContext, MenuSpec> = {
  cell: [
    ...CLIPBOARD,
    'edit.pasteSpecial',
    '-',
    // Insert… / Delete… ask how the other cells shift (or take whole rows / columns).
    'cells.insert',
    'cells.delete',
    'edit.clear',
    '-',
    'data.sortAsc',
    'data.sortDesc',
    'data.filter',
    '-',
    'format.cells',
    'data.validation',
    'insert.pictureInCell',
  ],
  rowHeader: [
    ...CLIPBOARD,
    'edit.pasteSpecial',
    '-',
    'cells.insertRow',
    'cells.deleteRow',
    'edit.clear',
    '-',
    'format.cells',
    'cells.rowHeight',
    'cells.autofitRow',
    'cells.hideRows',
    'cells.unhideRows',
  ],
  columnHeader: [
    ...CLIPBOARD,
    'edit.pasteSpecial',
    '-',
    'cells.insertCol',
    'cells.deleteCol',
    'edit.clear',
    '-',
    'format.cells',
    'cells.colWidth',
    'cells.autofitCol',
    'cells.hideCols',
    'cells.unhideCols',
  ],
  // `sheet.tabColor` is a local action of the editor (it opens the tab colour palette).
  sheetTab: [
    'sheet.insertBefore',
    'sheet.add',
    'sheet.delete',
    'sheet.rename',
    'sheet.tabColor',
    'sheet.moveLeft',
    'sheet.moveRight',
    '-',
    'review.protectSheet',
  ],
  chart: ['chart.edit', 'chart.styles', 'chart.saveTemplate', '-', 'chart.delete'],
  picture: [...CLIPBOARD, '-', 'cell.pictureAltText', 'edit.clear', '-', 'format.cells'],
};

const ARRANGE = ['arrange.front', 'arrange.back', 'arrange.forward', 'arrange.backward'] as const;

export const SLIDE_MENUS: Record<SlideMenuContext, MenuSpec> = {
  shape: [
    ...CLIPBOARD,
    '-',
    'edit.duplicate',
    'edit.delete',
    '-',
    ...ARRANGE,
    'arrange.group',
    'arrange.ungroup',
    '-',
    'format.fill',
    'format.line',
  ],
  text: [
    ...CLIPBOARD,
    '-',
    'format.bold',
    'format.italic',
    'format.underline',
    'format.bullets',
    'format.numbering',
    '-',
    'format.alignLeft',
    'format.alignCenter',
    'format.alignRight',
  ],
  picture: [
    ...CLIPBOARD,
    '-',
    'picture.change',
    'picture.reset',
    'picture.customCrop',
    'picture.adjust',
    '-',
    ...ARRANGE,
    '-',
    'edit.delete',
  ],
  table: [
    ...CLIPBOARD,
    '-',
    'table.insertRowAbove',
    'table.insertRowBelow',
    'table.insertColLeft',
    'table.insertColRight',
    '-',
    'table.deleteRows',
    'table.deleteCols',
    'table.merge',
    'table.split',
  ],
  chart: ['chart.edit', '-', ...ARRANGE, '-', 'edit.delete'],
  thumbnail: [
    'slide.new',
    'slide.duplicate',
    'slide.delete',
    '-',
    'slide.cut',
    'slide.copy',
    'slide.paste',
    '-',
    'section.add',
  ],
  canvas: ['edit.paste', '-', 'edit.selectAll', 'slide.new', '-', 'view.selectionPane'],
};

/** Every menu, by app. */
export const CONTEXT_MENUS = { doc: DOC_MENUS, sheet: SHEET_MENUS, slide: SLIDE_MENUS } as const;

/** Resolves a command id to a menu entry (undefined: the app has no such command). */
export type MenuLookup = (id: string) => MenuEntry | undefined;

/**
 * The menu lines of a spec: unknown commands are left out, separators collapse (none at the
 * start, the end, or twice in a row).
 */
export function resolveMenu(spec: MenuSpec, lookup: MenuLookup): MenuLine[] {
  const out: MenuLine[] = [];
  for (const id of spec) {
    if (id === '-') {
      if (out.length && !('separator' in out[out.length - 1]!)) out.push({ separator: true });
      continue;
    }
    const e = lookup(id);
    if (e) out.push(e);
  }
  while (out.length && 'separator' in out[out.length - 1]!) out.pop();
  return out;
}

/**
 * Lookup over the ribbon items (every tab, contextual ones included; first occurrence wins),
 * then `fallback` for commands the ribbon does not show. Custom controls (comboboxes) are
 * skipped: a menu only holds plain commands.
 */
export function ribbonMenuLookup(tabs: readonly RibbonTab[], fallback?: MenuLookup): MenuLookup {
  const byId = new Map<string, MenuEntry>();
  for (const tab of tabs)
    for (const group of tab.groups) {
      for (const item of group.items) {
        if (item.render || byId.has(item.id) || !item.label || item.label === item.id) continue;
        byId.set(item.id, {
          id: item.id,
          label: item.label,
          shortcut: item.shortcut,
          enabled: item.enabled !== false,
          checked: item.checked,
          onSelect: item.onSelect,
        });
      }
      const l = group.launcher;
      if (l && !byId.has(l.id)) byId.set(l.id, { id: l.id, label: l.label, onSelect: l.onSelect });
    }
  return (id) => byId.get(id) ?? fallback?.(id);
}

/** Lookup over a registry-like object: commands it has, labelled by `label` (null: skip). */
export function registryMenuLookup<S>(
  registry: {
    has(id: string): boolean;
    isEnabled(id: string, s: S): boolean;
    get(id: string): { shortcut?: string } | undefined;
  },
  state: () => S,
  label: (id: string) => string | null,
  run: (id: string) => void,
): MenuLookup {
  return (id) => {
    if (!registry.has(id)) return undefined;
    const text = label(id);
    if (!text) return undefined;
    return {
      id,
      label: text,
      shortcut: registry.get(id)?.shortcut,
      enabled: registry.isEnabled(id, state()),
      onSelect: () => run(id),
    };
  };
}
