/**
 * BlueSlide master view and Design tab (SLIDE-MASTER-001): the contextual
 * "Slide master" ribbon tab, the Design tab (themes, variants, slide size,
 * background, layouts), the slide size and rename dialogs. Every action is a
 * registry command that sends one engine command (undoable there).
 */
import type { CommandRegistry } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import {
  SLIDE_PLACEHOLDER_KINDS,
  SLIDE_SIZE_MAX,
  SLIDE_SIZE_MIN,
  SLIDE_SIZES,
  type SlideCommand,
  type SlideDesignChoice,
  type SlideDesignInfo,
  type SlideEditResult,
  type SlideMasterItem,
  type SlidePlaceholderKind,
  type SlideSizeMode,
} from '@blueoffice/platform';
import { Dialog, type RibbonTab } from '@blueoffice/ui';
import {
  Copy,
  FilePlus,
  FolderOpen,
  Image,
  LayoutTemplate,
  Paintbrush,
  PanelTop,
  Pencil,
  Pin,
  Proportions,
  SquareDashed,
  Trash,
  Type,
  X,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import styles from './masterView.module.css';

/** 1 cm = 360 000 EMU (§20.1.10.16 ST_Coordinate: 914 400 EMU per inch). */
export const EMU_PER_CM = 360_000;

/** What the master / design commands look at. */
export interface MasterState {
  hasDeck: boolean;
  /** The master view is open. */
  masterView?: boolean;
  /** Kind of the current view slide. */
  masterItem?: 'master' | 'layout';
  /** The current layout hides the master graphics. */
  hideGraphics?: boolean;
  /** The current master is preserved. */
  preserved?: boolean;
  /** The current layout has a title / footers. */
  layoutTitle?: boolean;
  layoutFooters?: boolean;
}

/** State fields of the current view slide. */
export function masterState(edit: SlideEditResult | null): Omit<MasterState, 'hasDeck'> {
  const items = edit?.masterView?.items;
  if (!items) return { masterView: false };
  const item: SlideMasterItem | undefined = items[edit.slide];
  return {
    masterView: true,
    masterItem: item?.kind,
    hideGraphics: item?.hideGraphics,
    preserved: item
      ? items.find((x) => x.kind === 'master' && x.master === item.master)?.preserved
      : false,
    layoutTitle: item?.title,
    layoutFooters: item?.footers,
  };
}

/** A custom slide size in centimetres → EMU, or `null` outside §19.2.1.39's range. */
export function sizeFromCm(width: number, height: number): { cx: number; cy: number } | null {
  if (!Number.isFinite(width) || !Number.isFinite(height)) return null;
  const cx = Math.round(width * EMU_PER_CM);
  const cy = Math.round(height * EMU_PER_CM);
  const ok = (v: number) => v >= SLIDE_SIZE_MIN && v <= SLIDE_SIZE_MAX;
  return ok(cx) && ok(cy) ? { cx, cy } : null;
}

/** Standard bytes → base64 (theme files sent through the JSON bridge). */
export function toBase64(bytes: Uint8Array): string {
  let bin = '';
  const step = 0x8000;
  for (let i = 0; i < bytes.length; i += step) {
    bin += String.fromCharCode(...bytes.subarray(i, i + step));
  }
  return btoa(bin);
}

export interface MasterDeps {
  run: (cmd: SlideCommand) => void;
  /** Opens the slide size dialog. */
  openSize: () => void;
  /** Opens the rename dialog for the current master / layout. */
  openRename: () => void;
  /** Lets the user pick a `.thmx` / `.pptx` and returns its bytes. */
  pickThemeFile: () => Promise<Uint8Array | null>;
  /** Localised default names. */
  names: () => { layout: string; master: string };
}

/** Registers the master view and Design commands (`master.*`, `design.*`, `view.slideMaster`). */
export function registerMasterCommands<S extends MasterState>(
  reg: CommandRegistry<S>,
  deps: MasterDeps,
): void {
  const deck = (s: S) => s.hasDeck;
  const inView = (s: S) => s.hasDeck && !!s.masterView;
  const onLayout = (s: S) => inView(s) && s.masterItem === 'layout';
  const normal = (s: S) => s.hasDeck && !s.masterView;
  const simple = (id: string, cmd: SlideCommand, isEnabled: (s: S) => boolean) =>
    reg.register({ id, isEnabled, run: () => deps.run(cmd) });

  reg.register({
    id: 'view.slideMaster',
    isEnabled: deck,
    isChecked: (s) => !!s.masterView,
    run: () => deps.run({ cmd: 'masterView', on: true }),
  });
  simple('master.close', { cmd: 'masterView', on: false }, inView);
  reg.register({
    id: 'master.insertLayout',
    isEnabled: inView,
    run: () => deps.run({ cmd: 'insertLayout', name: deps.names().layout }),
  });
  reg.register({
    id: 'master.insertMaster',
    isEnabled: inView,
    run: () => deps.run({ cmd: 'addMaster', name: deps.names().master }),
  });
  simple('master.duplicateLayout', { cmd: 'duplicateLayout' }, onLayout);
  simple('master.delete', { cmd: 'deleteMasterItem' }, inView);
  reg.register({ id: 'master.rename', isEnabled: inView, run: deps.openRename });
  reg.register({
    id: 'master.preserve',
    isEnabled: inView,
    isChecked: (s) => !!s.preserved,
    run: () => undefined,
  });
  // The registry has no access to the state inside `run`: toggles get one id per value.
  simple('master.preserve.on', { cmd: 'preserveMaster', on: true }, inView);
  simple('master.preserve.off', { cmd: 'preserveMaster', on: false }, inView);
  for (const kind of SLIDE_PLACEHOLDER_KINDS) {
    simple(`master.placeholder.${kind}`, { cmd: 'insertPlaceholder', kind }, onLayout);
  }
  simple('master.title.on', { cmd: 'layoutElements', title: true }, onLayout);
  simple('master.title.off', { cmd: 'layoutElements', title: false }, onLayout);
  simple('master.footers.on', { cmd: 'layoutElements', footers: true }, onLayout);
  simple('master.footers.off', { cmd: 'layoutElements', footers: false }, onLayout);
  simple(
    'design.hideGraphics.on',
    { cmd: 'hideBackgroundGraphics', hide: true },
    (s) => normal(s) || onLayout(s),
  );
  simple(
    'design.hideGraphics.off',
    { cmd: 'hideBackgroundGraphics', hide: false },
    (s) => normal(s) || onLayout(s),
  );
  simple('design.resetBackground', { cmd: 'setBackground' }, deck);
  reg.register({ id: 'design.slideSize', isEnabled: normal, run: deps.openSize });
  reg.register({
    id: 'design.themeFile',
    isEnabled: deck,
    run: async () => {
      const bytes = await deps.pickThemeFile();
      if (bytes) deps.run({ cmd: 'applyTheme', file: toBase64(bytes) });
    },
  });
}

/** Commands for one design (built-in theme, colour / font variant). */
export function registerDesignChoices<S extends MasterState>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
  design: SlideDesignInfo | undefined,
): () => void {
  const off: (() => void)[] = [];
  const deck = (s: S) => s.hasDeck;
  for (const c of design?.themes ?? []) {
    off.push(
      reg.register({
        id: `design.theme.${c.id}`,
        isEnabled: deck,
        run: () => run({ cmd: 'applyTheme', builtin: c.id }),
      }),
    );
  }
  for (const c of design?.colorVariants ?? []) {
    off.push(
      reg.register({
        id: `design.colors.${c.id}`,
        isEnabled: deck,
        run: () => run({ cmd: 'themeVariant', colors: c.id }),
      }),
    );
  }
  for (const c of design?.fontVariants ?? []) {
    off.push(
      reg.register({
        id: `design.fonts.${c.id}`,
        isEnabled: deck,
        run: () => run({ cmd: 'themeVariant', fonts: c.id }),
      }),
    );
  }
  return () => off.forEach((f) => f());
}

/** Colour preview of a theme or variant. */
export function Swatch({ choice }: { choice: SlideDesignChoice }) {
  return (
    <span className={styles.swatch} aria-hidden="true">
      {choice.colors.length > 0
        ? choice.colors.map((c, i) => (
            <span key={i} className={styles.chip} style={{ background: `#${c}` }} />
          ))
        : (choice.fonts?.[0] ?? '').slice(0, 2)}
    </span>
  );
}

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

/** Background colour picker shown in the ribbon. */
function BackgroundColor({
  label,
  onPick,
  testId,
}: {
  label: string;
  onPick: (hex: string) => void;
  testId: string;
}) {
  return (
    <label className={styles.colorPick}>
      <Paintbrush size={16} aria-hidden="true" />
      <span>{label}</span>
      <input
        type="color"
        data-testid={testId}
        aria-label={label}
        defaultValue="#ffffff"
        onChange={(e) => onPick(e.target.value.slice(1).toUpperCase())}
      />
    </label>
  );
}

const PH_ICONS: Record<SlidePlaceholderKind, ReactNode> = {
  content: <SquareDashed size={16} />,
  text: <Type size={16} />,
  picture: <Image size={16} />,
  chart: <SquareDashed size={16} />,
  table: <SquareDashed size={16} />,
  diagram: <SquareDashed size={16} />,
  media: <SquareDashed size={16} />,
  onlineImage: <Image size={16} />,
};

/** The contextual "Slide master" tab (master view only). */
export function masterTab(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: Item,
  state: MasterState,
  run: (cmd: SlideCommand) => void,
): RibbonTab {
  const sm = 'small' as const;
  const toggle = (base: string, on: boolean | undefined, label: string, icon: ReactNode) =>
    item(`${base}.${on ? 'off' : 'on'}`, label, icon, sm, !!on);
  return {
    id: 'slideMaster',
    label: t('slideEditor.master.tab'),
    groups: [
      {
        id: 'masterEdit',
        label: t('slideEditor.master.groupEdit'),
        items: [
          item('master.insertMaster', t('slideEditor.master.insertMaster'), <FilePlus size={18} />),
          item(
            'master.insertLayout',
            t('slideEditor.master.insertLayout'),
            <LayoutTemplate size={18} />,
          ),
          item(
            'master.duplicateLayout',
            t('slideEditor.master.duplicateLayout'),
            <Copy size={16} />,
            sm,
          ),
          item('master.delete', t('slideEditor.master.delete'), <Trash size={16} />, sm),
          item('master.rename', t('slideEditor.master.rename'), <Pencil size={16} />, sm),
          toggle(
            'master.preserve',
            state.preserved,
            t('slideEditor.master.preserve'),
            <Pin size={16} />,
          ),
        ],
      },
      {
        id: 'masterLayout',
        label: t('slideEditor.master.groupLayout'),
        items: [
          ...SLIDE_PLACEHOLDER_KINDS.map((k) =>
            item(`master.placeholder.${k}`, t(`slideEditor.master.ph.${k}`), PH_ICONS[k], sm),
          ),
          toggle(
            'master.title',
            state.layoutTitle,
            t('slideEditor.master.title'),
            <Type size={16} />,
          ),
          toggle(
            'master.footers',
            state.layoutFooters,
            t('slideEditor.master.footers'),
            <PanelTop size={16} />,
          ),
        ],
      },
      {
        id: 'masterBackground',
        label: t('slideEditor.design.groupBackground'),
        items: [
          {
            id: 'master.backgroundColor',
            label: t('slideEditor.design.background'),
            onSelect: () => undefined,
            render: (
              <BackgroundColor
                label={t('slideEditor.design.background')}
                testId="master-background"
                onPick={(color) => run({ cmd: 'setBackground', color })}
              />
            ),
          },
          item(
            'design.resetBackground',
            t('slideEditor.design.resetBackground'),
            <Paintbrush size={16} />,
            sm,
          ),
          toggle(
            'design.hideGraphics',
            state.hideGraphics,
            t('slideEditor.design.hideGraphics'),
            <SquareDashed size={16} />,
          ),
        ],
      },
      {
        id: 'masterClose',
        label: t('slideEditor.master.groupClose'),
        items: [item('master.close', t('slideEditor.master.close'), <X size={18} />)],
      },
    ],
  };
}

/** The Design tab: themes, variants, background, slide size, layouts. */
export function designTab(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: Item,
  design: SlideDesignInfo | undefined,
  state: MasterState,
  current: number,
  run: (cmd: SlideCommand) => void,
): RibbonTab {
  const sm = 'small' as const;
  const swatchItem = (prefix: string, c: SlideDesignChoice, label: string) =>
    item(`design.${prefix}.${c.id}`, label, <Swatch choice={c} />, sm);
  const layouts = state.masterView
    ? []
    : (design?.masters ?? []).flatMap((m) =>
        m.layouts.map((l) => ({
          id: `design.layout.${l.index}`,
          label: design && design.masters.length > 1 ? `${m.name} · ${l.name}` : l.name,
          size: sm,
          enabled: state.hasDeck,
          onSelect: () => run({ cmd: 'applyLayout', layout: l.index, slides: [current] }),
        })),
      );
  return {
    id: 'design',
    label: t('slideEditor.design.tab'),
    groups: [
      {
        id: 'designThemes',
        label: t('slideEditor.design.groupThemes'),
        items: [
          ...(design?.themes ?? []).map((c) =>
            swatchItem('theme', c, t(`slideEditor.design.theme.${c.id}`)),
          ),
          item('design.themeFile', t('slideEditor.design.themeFile'), <FolderOpen size={16} />, sm),
        ],
      },
      {
        id: 'designVariants',
        label: t('slideEditor.design.groupVariants'),
        items: [
          ...(design?.colorVariants ?? []).map((c) =>
            swatchItem(
              'colors',
              c,
              t('slideEditor.design.colorsOf', { name: t(`slideEditor.design.theme.${c.id}`) }),
            ),
          ),
          ...(design?.fontVariants ?? []).map((c) =>
            swatchItem(
              'fonts',
              c,
              t('slideEditor.design.fontsOf', { major: c.fonts?.[0], minor: c.fonts?.[1] }),
            ),
          ),
        ],
      },
      {
        id: 'designCustomize',
        label: t('slideEditor.design.groupCustomize'),
        items: [
          item('design.slideSize', t('slideEditor.design.slideSize'), <Proportions size={18} />),
          {
            id: 'design.backgroundColor',
            label: t('slideEditor.design.background'),
            onSelect: () => undefined,
            render: (
              <BackgroundColor
                label={t('slideEditor.design.background')}
                testId="design-background"
                onPick={(color) => run({ cmd: 'setBackground', color })}
              />
            ),
          },
          item(
            'design.resetBackground',
            t('slideEditor.design.resetBackground'),
            <Paintbrush size={16} />,
            sm,
          ),
          item(
            `design.hideGraphics.${state.hideGraphics ? 'off' : 'on'}`,
            t('slideEditor.design.hideGraphics'),
            <SquareDashed size={16} />,
            sm,
            !!state.hideGraphics,
          ),
        ],
      },
      ...(layouts.length
        ? [{ id: 'designLayouts', label: t('slideEditor.design.groupLayouts'), items: layouts }]
        : []),
    ],
  };
}

export interface SlideSizeDialogProps {
  open: boolean;
  /** Current size, EMU. */
  cx: number;
  cy: number;
  onClose: () => void;
  onApply: (cmd: Extract<SlideCommand, { cmd: 'setSlideSize' }>) => void;
}

type Preset = 'standard' | 'widescreen' | 'custom';

/** Slide size: 4:3, 16:9 or custom (cm), with "scale" or "fit" of the content. */
export function SlideSizeDialog(props: SlideSizeDialogProps) {
  // Mounted while open: the fields start from the current size each time.
  return props.open ? <SlideSizeForm {...props} /> : null;
}

function SlideSizeForm({ open, cx, cy, onClose, onApply }: SlideSizeDialogProps) {
  const { t } = useTranslation();
  const initial: Preset =
    cx === SLIDE_SIZES.standard.cx && cy === SLIDE_SIZES.standard.cy
      ? 'standard'
      : cx === SLIDE_SIZES.widescreen.cx && cy === SLIDE_SIZES.widescreen.cy
        ? 'widescreen'
        : 'custom';
  const [preset, setPreset] = useState<Preset>(initial);
  const [w, setW] = useState(String(Math.round((cx / EMU_PER_CM) * 100) / 100));
  const [h, setH] = useState(String(Math.round((cy / EMU_PER_CM) * 100) / 100));
  const [mode, setMode] = useState<SlideSizeMode>('fit');
  const custom = sizeFromCm(Number(w), Number(h));
  const size =
    preset === 'custom'
      ? custom && { ...custom, kind: 'custom' }
      : { cx: SLIDE_SIZES[preset].cx, cy: SLIDE_SIZES[preset].cy, kind: SLIDE_SIZES[preset].kind };
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.design.slideSize')}
      testId="slide-size-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="slide-size-ok"
            disabled={!size}
            onClick={() => {
              if (!size) return;
              onApply({
                cmd: 'setSlideSize',
                cx: size.cx,
                cy: size.cy,
                ...(size.kind ? { kind: size.kind } : {}),
                mode,
              });
            }}
          >
            {t('slideEditor.design.apply')}
          </button>
        </>
      }
    >
      <fieldset className={styles.form}>
        <legend>{t('slideEditor.design.size')}</legend>
        {(['standard', 'widescreen', 'custom'] as const).map((p) => (
          <label key={p} className={styles.row}>
            <input
              type="radio"
              name="slide-size"
              data-testid={`slide-size-${p}`}
              checked={preset === p}
              onChange={() => setPreset(p)}
            />
            {t(`slideEditor.design.size_${p}`)}
          </label>
        ))}
        {preset === 'custom' && (
          <div className={styles.row}>
            <label>
              {t('slideEditor.design.widthCm')}
              <input
                type="number"
                step="0.01"
                data-testid="slide-size-width"
                value={w}
                onChange={(e) => setW(e.target.value)}
              />
            </label>
            <label>
              {t('slideEditor.design.heightCm')}
              <input
                type="number"
                step="0.01"
                data-testid="slide-size-height"
                value={h}
                onChange={(e) => setH(e.target.value)}
              />
            </label>
          </div>
        )}
        {preset === 'custom' && !custom && (
          <p role="alert" className={styles.hint}>
            {t('slideEditor.design.sizeRange')}
          </p>
        )}
      </fieldset>
      <fieldset className={styles.form}>
        <legend>{t('slideEditor.design.content')}</legend>
        {(['scale', 'fit'] as const).map((m) => (
          <label key={m} className={styles.row}>
            <input
              type="radio"
              name="slide-size-mode"
              data-testid={`slide-size-${m}`}
              checked={mode === m}
              onChange={() => setMode(m)}
            />
            {t(`slideEditor.design.mode_${m}`)}
          </label>
        ))}
      </fieldset>
    </Dialog>
  );
}

export interface RenameDialogProps {
  open: boolean;
  initial: string;
  onClose: () => void;
  onRename: (name: string) => void;
}

/** Rename the current master or layout. */
export function RenameDialog(props: RenameDialogProps) {
  return props.open ? <RenameForm {...props} /> : null;
}

function RenameForm({ open, initial, onClose, onRename }: RenameDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial);
  const ok = name.trim().length > 0;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.master.rename')}
      testId="master-rename-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="master-rename-ok"
            disabled={!ok}
            onClick={() => ok && onRename(name.trim())}
          >
            {t('slideEditor.master.rename')}
          </button>
        </>
      }
    >
      <label className={styles.form}>
        {t('slideEditor.master.name')}
        <input
          data-testid="master-rename-input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </label>
    </Dialog>
  );
}

/** Label of a master view thumbnail (name and number of slides using it). */
export function masterThumbLabel(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: SlideMasterItem,
): string {
  return t(
    item.kind === 'master' ? 'slideEditor.master.thumbMaster' : 'slideEditor.master.thumbLayout',
    {
      name: item.name || t('slideEditor.master.unnamed'),
      count: item.uses,
    },
  );
}
