/**
 * BlueSlide notes master and handout master views (SLIDE-VIEW-001): the
 * contextual ribbon tab (placeholders, orientation, slides per handout page,
 * close) and the dashed slide boxes of the handout preview. The master page is
 * edited in the normal slide editor like a slide (see the engine's
 * `page_master` module); every action is a registry command sending one engine
 * command.
 */
import type { CommandRegistry } from '@blueoffice/editor';
import {
  HANDOUT_PER_PAGE,
  type SlideCommand,
  type SlideEditResult,
  type SlidePageMasterInfo,
} from '@blueoffice/platform';
import type { RibbonTab } from '@blueoffice/ui';
import {
  Calendar,
  FileText,
  Hash,
  Image,
  LayoutGrid,
  PanelBottom,
  PanelTop,
  RectangleHorizontal,
  RectangleVertical,
  X,
} from 'lucide-react';
import type { ReactNode } from 'react';
import styles from './views.module.css';

export interface PageMasterState {
  hasDeck: boolean;
  /** The notes / handout master view that is open. */
  pageMaster?: SlidePageMasterInfo | null;
  /** Another master view (slide master) is open. */
  masterView?: boolean;
}

export function pageMasterState(edit: SlideEditResult | null): {
  pageMaster: SlidePageMasterInfo | null;
} {
  return { pageMaster: edit?.pageMaster ?? null };
}

/** Placeholder toggles: command field, ribbon label key, icon, notes master only. */
const ELEMENTS = [
  ['header', 'header', <PanelTop key="h" size={16} />, false],
  ['date', 'date', <Calendar key="d" size={16} />, false],
  ['footer', 'footer', <PanelBottom key="f" size={16} />, false],
  ['number', 'number', <Hash key="n" size={16} />, false],
  ['slideImage', 'slideImage', <Image key="i" size={16} />, true],
  ['body', 'body', <FileText key="b" size={16} />, true],
] as const;

export type PageElement = (typeof ELEMENTS)[number][0];

/** `pageMasterElements` switching one placeholder on or off. */
export function elementCommand(field: PageElement, on: boolean): SlideCommand {
  const flags: Partial<Record<PageElement, boolean>> = { [field]: on };
  return { cmd: 'pageMasterElements', ...flags };
}

/** Registers `view.notesMaster`, `view.handoutMaster` and `pageMaster.*`. */
export function registerPageMasterCommands<S extends PageMasterState>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
): void {
  const canOpen = (s: S) => s.hasDeck && !s.masterView && !s.pageMaster;
  const inView = (s: S) => s.hasDeck && !!s.pageMaster;
  reg.register({
    id: 'view.notesMaster',
    isEnabled: (s) => canOpen(s) || s.pageMaster?.kind === 'notes',
    isChecked: (s) => s.pageMaster?.kind === 'notes',
    run: () => run({ cmd: 'pageMasterView', kind: 'notes', on: true }),
  });
  reg.register({
    id: 'view.handoutMaster',
    isEnabled: (s) => canOpen(s) || s.pageMaster?.kind === 'handout',
    isChecked: (s) => s.pageMaster?.kind === 'handout',
    run: () => run({ cmd: 'pageMasterView', kind: 'handout', on: true }),
  });
  reg.register({
    id: 'pageMaster.close',
    isEnabled: inView,
    run: () => run({ cmd: 'pageMasterView', on: false }),
  });
  for (const [field, , , notesOnly] of ELEMENTS) {
    const enabled = (s: S) => inView(s) && (!notesOnly || s.pageMaster?.kind === 'notes');
    // The registry has no access to the state inside `run`: one id per value.
    reg.register({
      id: `pageMaster.${field}.on`,
      isEnabled: enabled,
      run: () => run(elementCommand(field, true)),
    });
    reg.register({
      id: `pageMaster.${field}.off`,
      isEnabled: enabled,
      isChecked: (s) => !!s.pageMaster?.[field],
      run: () => run(elementCommand(field, false)),
    });
  }
  reg.register({
    id: 'pageMaster.portrait',
    isEnabled: inView,
    isChecked: (s) => !!s.pageMaster?.portrait,
    run: () => run({ cmd: 'pageOrientation', portrait: true }),
  });
  reg.register({
    id: 'pageMaster.landscape',
    isEnabled: inView,
    isChecked: (s) => !!s.pageMaster && !s.pageMaster.portrait,
    run: () => run({ cmd: 'pageOrientation', portrait: false }),
  });
  for (const n of HANDOUT_PER_PAGE) {
    reg.register({
      id: `pageMaster.perPage.${n}`,
      isEnabled: (s) => inView(s) && s.pageMaster?.kind === 'handout',
      isChecked: (s) => s.pageMaster?.perPage === n,
      run: () => run({ cmd: 'handoutSlides', perPage: n }),
    });
  }
}

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

/** The contextual "Notes master" / "Handout master" tab. */
export function pageMasterTab(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: Item,
  info: SlidePageMasterInfo,
): RibbonTab {
  const sm = 'small' as const;
  const notes = info.kind === 'notes';
  return {
    id: 'pageMaster',
    label: t(notes ? 'slideEditor.views.notesMasterTab' : 'slideEditor.views.handoutMasterTab'),
    groups: [
      {
        id: 'pageMasterPlaceholders',
        label: t('slideEditor.views.groupPlaceholders'),
        items: ELEMENTS.filter(([, , , notesOnly]) => notes || !notesOnly).map(
          ([field, key, icon]) =>
            item(
              `pageMaster.${field}.${info[field] ? 'off' : 'on'}`,
              t(`slideEditor.views.element.${key}`),
              icon,
              sm,
              info[field],
            ),
        ),
      },
      {
        id: 'pageMasterSetup',
        label: t('slideEditor.views.groupPageSetup'),
        items: [
          item(
            'pageMaster.portrait',
            t('slideEditor.views.portrait'),
            <RectangleVertical size={16} />,
            sm,
            info.portrait,
          ),
          item(
            'pageMaster.landscape',
            t('slideEditor.views.landscape'),
            <RectangleHorizontal size={16} />,
            sm,
            !info.portrait,
          ),
          ...(notes
            ? []
            : HANDOUT_PER_PAGE.map((n) =>
                item(
                  `pageMaster.perPage.${n}`,
                  t('slideEditor.views.perPage', { count: n }),
                  <LayoutGrid size={16} />,
                  sm,
                  info.perPage === n,
                ),
              )),
        ],
      },
      {
        id: 'pageMasterClose',
        label: t('slideEditor.master.groupClose'),
        items: [item('pageMaster.close', t('slideEditor.views.closeMaster'), <X size={18} />)],
      },
    ],
  };
}

/** Dashed boxes where the slides go on a handout page (preview, not editable). */
export function HandoutSlots({
  info,
  scale,
}: {
  info: SlidePageMasterInfo | null | undefined;
  /** CSS pixels per point. */
  scale: number;
}) {
  if (!info || info.kind !== 'handout') return null;
  return (
    <div className={styles.handoutSlots} aria-hidden="true" data-testid="handout-slots">
      {info.slots.map((r, i) => (
        <div
          key={i}
          className={styles.handoutSlot}
          data-testid={`handout-slot-${i + 1}`}
          style={{ left: r.x * scale, top: r.y * scale, width: r.w * scale, height: r.h * scale }}
        >
          {i + 1}
        </div>
      ))}
    </div>
  );
}
