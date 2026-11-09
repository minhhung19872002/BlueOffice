/**
 * Shape contextual tab (CORE-SHAPE-001): "Edit shape ▾" (edit points of the selected shape's
 * outline) and "Merge shapes ▾" (union, exclude overlap, split into pieces, keep overlap,
 * subtract) for two or more selected shapes. Every entry is a command in the registry; the
 * geometry work is done by the engine (`convertToFreeform`, `editPoint`, `mergeShapes`).
 */
import type { CommandRegistry } from '@blueoffice/editor';
import type { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand, SlideMergeOp } from '@blueoffice/platform';
import { Popover, type RibbonTab } from '@blueoffice/ui';
import { Combine, PenTool } from 'lucide-react';
import styles from './shapeFormat.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Merge operations in menu order. */
export const MERGE_OPS: readonly SlideMergeOp[] = [
  'union',
  'combine',
  'fragment',
  'intersect',
  'subtract',
];

/** Selection facts the shape tools depend on. */
export interface ShapeToolState {
  /** Exactly one plain shape is selected. */
  isShape?: boolean;
  /** Two or more top-level shapes are selected (merge). */
  mergeable?: boolean;
  /** The point editor is open. */
  editingPoints?: boolean;
}

/** Register the shape tool commands; returns the unregister function. */
export function registerShapeTools<S extends ShapeToolState>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
  setEditingPoints: (on: boolean) => void,
): () => void {
  const off: (() => void)[] = [];
  off.push(
    reg.register({
      id: 'shape.editPoints',
      isEnabled: (s) => !!s.isShape,
      isChecked: (s) => !!s.editingPoints,
      run: (s) => {
        if (s.editingPoints) {
          setEditingPoints(false);
          return;
        }
        run({ cmd: 'convertToFreeform' });
        setEditingPoints(true);
      },
    }),
  );
  for (const op of MERGE_OPS) {
    off.push(
      reg.register({
        id: `shape.merge.${op}`,
        isEnabled: (s) => !!s.mergeable,
        run: () => {
          setEditingPoints(false);
          run({ cmd: 'mergeShapes', op });
        },
      }),
    );
  }
  return () => off.forEach((f) => f());
}

interface MenuEntry {
  id: string;
  label: string;
  enabled: boolean;
  checked?: boolean;
}

function Menu({
  entries,
  exec,
  close,
  testId,
}: {
  entries: MenuEntry[];
  exec: (id: string) => void;
  close: () => void;
  testId: string;
}) {
  return (
    <div className={styles.menu} role="menu" data-testid={testId}>
      {entries.map((e) => (
        <button
          key={e.id}
          type="button"
          role={e.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
          aria-checked={e.checked}
          className={styles.entry}
          disabled={!e.enabled}
          data-command={e.id}
          data-testid={`menu-${e.id}`}
          onClick={() => {
            close();
            exec(e.id);
          }}
        >
          {e.label}
        </button>
      ))}
    </div>
  );
}

/** The shape contextual tab. */
export function shapeFormatTab<S extends ShapeToolState>({
  t,
  state,
  enabled,
  exec,
}: {
  t: T;
  state: S;
  enabled: (id: string) => boolean;
  exec: (id: string) => void;
}): RibbonTab {
  const none = () => {};
  const mergeEntries: MenuEntry[] = MERGE_OPS.map((op) => ({
    id: `shape.merge.${op}`,
    label: t(`slideEditor.shapeTools.${op}`),
    enabled: enabled(`shape.merge.${op}`),
  }));
  const editEntries: MenuEntry[] = [
    {
      id: 'shape.editPoints',
      label: state.editingPoints
        ? t('slideEditor.shapeTools.exitEditPoints')
        : t('slideEditor.shapeTools.editPoints'),
      enabled: enabled('shape.editPoints'),
      checked: !!state.editingPoints,
    },
  ];
  const anyMerge = mergeEntries.some((e) => e.enabled);
  return {
    id: 'shapeFormat',
    label: t('slideEditor.shapeTools.tab'),
    groups: [
      {
        id: 'shapeOutline',
        label: t('slideEditor.shapeTools.group'),
        items: [
          {
            id: 'shape.editShape',
            label: t('slideEditor.shapeTools.editShape'),
            onSelect: none,
            enabled: editEntries[0]?.enabled,
            render: (
              <Popover
                trigger={
                  <span className={styles.trigger}>
                    <PenTool size={18} aria-hidden="true" />
                    <span>{t('slideEditor.shapeTools.editShape')}</span>
                  </span>
                }
                label={t('slideEditor.shapeTools.editShape')}
                disabled={!editEntries[0]?.enabled}
                active={!!state.editingPoints}
                testId="shape-edit-menu"
              >
                {(close) => (
                  <Menu entries={editEntries} exec={exec} close={close} testId="shape-edit-list" />
                )}
              </Popover>
            ),
          },
          {
            id: 'shape.mergeShapes',
            label: t('slideEditor.shapeTools.mergeShapes'),
            onSelect: none,
            enabled: anyMerge,
            render: (
              <Popover
                trigger={
                  <span className={styles.trigger}>
                    <Combine size={18} aria-hidden="true" />
                    <span>{t('slideEditor.shapeTools.mergeShapes')}</span>
                  </span>
                }
                label={t('slideEditor.shapeTools.mergeShapes')}
                disabled={!anyMerge}
                testId="shape-merge-menu"
              >
                {(close) => (
                  <Menu
                    entries={mergeEntries}
                    exec={exec}
                    close={close}
                    testId="shape-merge-list"
                  />
                )}
              </Popover>
            ),
          },
        ],
      },
    ],
  };
}
