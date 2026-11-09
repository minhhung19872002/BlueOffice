/**
 * BlueSlide sections (SLIDE-VIEW-001): section headers in the slide pane and
 * the slide sorter (collapse / expand), the section commands and the rename
 * dialog. Sections are stored by the engine as the [MS-PPTX] `p14:sectionLst`
 * extension; collapsing is a view state of this window only.
 */
import type { CommandRegistry } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand, SlideSection } from '@blueoffice/platform';
import { Dialog, type RibbonTab } from '@blueoffice/ui';
import {
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  FolderMinus,
  FolderPlus,
  MoveDown,
  MoveUp,
  Pencil,
  Trash,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import styles from './views.module.css';

/** What the section commands look at. */
export interface SectionState {
  hasDeck: boolean;
  /** Number of sections (0: the deck has none). */
  sectionCount?: number;
  /** Index of the current section. */
  section?: number | null;
  /** Ids of the sections in order. */
  sectionIds?: string[];
  /** Current slide (where a new section starts). */
  current?: number;
}

/** Section holding slide `slide` (`pick`: a header the user clicked last). */
export function sectionAt(
  sections: SlideSection[] | undefined,
  slide: number,
  pick: number | null = null,
): number | null {
  if (!sections?.length) return null;
  if (pick !== null && pick < sections.length) return pick;
  const i = sections.findIndex((s) => slide >= s.first && slide < s.first + s.count);
  return i >= 0 ? i : 0;
}

export function sectionState(
  sections: SlideSection[] | undefined,
  slide: number,
  pick: number | null,
): Omit<SectionState, 'hasDeck' | 'current'> {
  return {
    sectionCount: sections?.length ?? 0,
    section: sectionAt(sections, slide, pick),
    sectionIds: (sections ?? []).map((s) => s.id),
  };
}

/** Pane entries: section headers and the slides of expanded sections. */
export type PaneEntry =
  { kind: 'section'; index: number; section: SlideSection } | { kind: 'slide'; index: number };

export function paneEntries(
  count: number,
  sections: SlideSection[] | undefined,
  collapsed: ReadonlySet<string>,
): PaneEntry[] {
  if (!sections?.length)
    return Array.from({ length: count }, (_, i) => ({ kind: 'slide', index: i }));
  const out: PaneEntry[] = [];
  sections.forEach((s, k) => {
    out.push({ kind: 'section', index: k, section: s });
    if (collapsed.has(s.id)) return;
    for (let i = s.first; i < s.first + s.count && i < count; i++)
      out.push({ kind: 'slide', index: i });
  });
  return out;
}

export interface SectionDeps {
  run: (cmd: SlideCommand) => void;
  /** Opens the rename dialog. */
  openRename: () => void;
  /** Localised default names: a new section, the first section of a deck without any. */
  names: () => { section: string; first: string };
  /** Collapse these sections (`[]`: expand every section). */
  collapse: (ids: string[]) => void;
}

/** Registers `section.*`. */
export function registerSectionCommands<S extends SectionState>(
  reg: CommandRegistry<S>,
  deps: SectionDeps,
): void {
  const deck = (s: S) => s.hasDeck;
  const has = (s: S) => s.hasDeck && (s.sectionCount ?? 0) > 0 && s.section != null;
  reg.register({
    id: 'section.add',
    isEnabled: deck,
    run: (s) =>
      deps.run({
        cmd: 'addSection',
        name: deps.names().section,
        firstName: deps.names().first,
        slide: s.current ?? 0,
      }),
  });
  reg.register({ id: 'section.rename', isEnabled: has, run: deps.openRename });
  reg.register({
    id: 'section.remove',
    isEnabled: has,
    run: (s) => deps.run({ cmd: 'removeSection', index: s.section ?? 0 }),
  });
  reg.register({
    id: 'section.removeWithSlides',
    isEnabled: has,
    run: (s) => deps.run({ cmd: 'removeSection', index: s.section ?? 0, slides: true }),
  });
  reg.register({
    id: 'section.removeAll',
    isEnabled: (s) => s.hasDeck && (s.sectionCount ?? 0) > 0,
    run: () => deps.run({ cmd: 'removeAllSections' }),
  });
  reg.register({
    id: 'section.moveUp',
    isEnabled: (s) => has(s) && (s.section ?? 0) > 0,
    run: (s) => deps.run({ cmd: 'moveSection', index: s.section ?? 0, to: (s.section ?? 0) - 1 }),
  });
  reg.register({
    id: 'section.moveDown',
    isEnabled: (s) => has(s) && (s.section ?? 0) + 1 < (s.sectionCount ?? 0),
    run: (s) => deps.run({ cmd: 'moveSection', index: s.section ?? 0, to: (s.section ?? 0) + 1 }),
  });
  reg.register({
    id: 'section.collapseAll',
    isEnabled: (s) => s.hasDeck && (s.sectionCount ?? 0) > 0,
    run: (s) => deps.collapse(s.sectionIds ?? []),
  });
  reg.register({
    id: 'section.expandAll',
    isEnabled: (s) => s.hasDeck && (s.sectionCount ?? 0) > 0,
    run: () => deps.collapse([]),
  });
}

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
) => RibbonTab['groups'][number]['items'][number];

/** The "Sections" ribbon group. */
export function sectionsGroup(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: Item,
): RibbonTab['groups'][number] {
  const sm = 'small' as const;
  return {
    id: 'sections',
    label: t('slideEditor.sections.group'),
    items: [
      item('section.add', t('slideEditor.sections.add'), <FolderPlus size={18} />),
      item('section.rename', t('slideEditor.sections.rename'), <Pencil size={16} />, sm),
      item('section.remove', t('slideEditor.sections.remove'), <FolderMinus size={16} />, sm),
      item(
        'section.removeWithSlides',
        t('slideEditor.sections.removeWithSlides'),
        <Trash size={16} />,
        sm,
      ),
      item('section.removeAll', t('slideEditor.sections.removeAll'), <Trash size={16} />, sm),
      item('section.moveUp', t('slideEditor.sections.moveUp'), <MoveUp size={16} />, sm),
      item('section.moveDown', t('slideEditor.sections.moveDown'), <MoveDown size={16} />, sm),
      item(
        'section.collapseAll',
        t('slideEditor.sections.collapseAll'),
        <ChevronsDownUp size={16} />,
        sm,
      ),
      item(
        'section.expandAll',
        t('slideEditor.sections.expandAll'),
        <ChevronsUpDown size={16} />,
        sm,
      ),
    ],
  };
}

/** Header of a section in the slide pane / sorter. */
export function SectionHeader({
  section,
  index,
  collapsed,
  current,
  onToggle,
  onPick,
  onRename,
}: {
  section: SlideSection;
  index: number;
  collapsed: boolean;
  current: boolean;
  onToggle: () => void;
  /** The header was clicked: its section becomes the current one. */
  onPick: () => void;
  onRename: () => void;
}) {
  const { t } = useTranslation();
  const name = section.name || t('slideEditor.sections.unnamed');
  return (
    <div
      className={styles.sectionHeader}
      data-testid={`section-header-${index + 1}`}
      data-current={current ? 'true' : undefined}
    >
      <button
        type="button"
        className={styles.sectionToggle}
        data-testid={`section-toggle-${index + 1}`}
        aria-expanded={!collapsed}
        aria-label={t(collapsed ? 'slideEditor.sections.expand' : 'slideEditor.sections.collapse', {
          name,
        })}
        onClick={onToggle}
      >
        {collapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
      </button>
      <button
        type="button"
        className={styles.sectionName}
        data-testid={`section-name-${index + 1}`}
        title={t('slideEditor.sections.slides', { count: section.count })}
        onClick={onPick}
        onDoubleClick={onRename}
      >
        {name}
        <span className={styles.sectionCount}>({section.count})</span>
      </button>
    </div>
  );
}

export interface SectionRenameProps {
  open: boolean;
  initial: string;
  onClose: () => void;
  onRename: (name: string) => void;
}

/** Rename dialog of the current section. */
export function SectionRenameDialog(props: SectionRenameProps) {
  return props.open ? <RenameForm {...props} /> : null;
}

function RenameForm({ open, initial, onClose, onRename }: SectionRenameProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial);
  const ok = name.trim().length > 0;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.sections.rename')}
      testId="section-rename-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="section-rename-ok"
            disabled={!ok}
            onClick={() => ok && onRename(name.trim())}
          >
            {t('slideEditor.sections.rename')}
          </button>
        </>
      }
    >
      <label className={styles.form}>
        {t('slideEditor.sections.name')}
        <input
          data-testid="section-rename-input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && ok) onRename(name.trim());
          }}
          autoFocus
        />
      </label>
    </Dialog>
  );
}
