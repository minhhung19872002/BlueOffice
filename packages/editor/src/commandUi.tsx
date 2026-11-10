/**
 * Shared pieces of the command UI of the three editors (CORE-UI-003 / CORE-UI-004): the open
 * context menu, and the command list of the shortcut dialog.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { ContextMenu, flattenRibbon, type MenuLine, type RibbonTab } from '@blueoffice/ui';
import type { CommandRegistry } from './commands';
import type { ShortcutCommand } from './ShortcutsDialog';

/** Where a context menu opened and for what (`text`, `table`…). */
export interface OpenContextMenu {
  x: number;
  y: number;
  context: string;
  /** `data-testid` of the menu (default `context-menu-<context>`). */
  testId?: string;
}

/**
 * State of an editor's context menu. Its lines are resolved at every render by `resolve`
 * (so contextual commands of the object just selected by the right click are there, with
 * their current enabled state); `element` renders it (null while closed).
 */
export function useContextMenu<M extends OpenContextMenu>(
  label: string,
  resolve: (m: M) => MenuLine[],
  onClosed?: () => void,
) {
  const [menu, setMenu] = useState<M | null>(null);
  const closed = useRef(onClosed);
  useEffect(() => {
    closed.current = onClosed;
  });
  const close = useCallback(() => {
    setMenu(null);
    closed.current?.();
  }, []);
  const items = menu ? resolve(menu) : [];
  const element =
    menu && items.length ? (
      <ContextMenu
        key={`${menu.x},${menu.y},${menu.context}`}
        x={menu.x}
        y={menu.y}
        items={items}
        label={label}
        testId={menu.testId ?? `context-menu-${menu.context}`}
        onClose={close}
      />
    ) : null;
  return { menu, open: setMenu, close, element };
}

/**
 * Commands of the shortcut dialog: every ribbon command (with its place), then the registered
 * commands that have a default shortcut but no ribbon button (labelled by `label`, skipped
 * when it has none).
 */
export function shortcutCommands<S>(
  tabs: readonly RibbonTab[],
  registry: CommandRegistry<S>,
  label: (id: string) => string | null,
): ShortcutCommand[] {
  const out: ShortcutCommand[] = flattenRibbon([...tabs]).map((e) => ({
    id: e.id,
    label: e.label,
    path: e.path,
  }));
  const seen = new Set(out.map((c) => c.id));
  for (const id of registry.ids()) {
    if (seen.has(id) || !registry.defaultShortcut(id)) continue;
    const text = label(id);
    if (text) out.push({ id, label: text });
  }
  return out;
}

/** i18n label of a command id (`commands.<id>`), null when there is none. */
export function commandLabel(
  i18n: { exists(key: string): boolean; t(key: string): string },
  id: string,
): string | null {
  const key = `commands.${id}`;
  return i18n.exists(key) ? i18n.t(key) : null;
}
