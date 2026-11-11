/**
 * BlueOffice design tokens (docs/ui/DESIGN.md). This file is the single source: `tokens.css` is
 * generated from it (`BO_WRITE_TOKENS=1 pnpm --filter @blueoffice/ui test`) and a test keeps the
 * two in sync. Colours are BlueOffice's own; the Bluestar brand colour is provisional
 * (docs/parity/NEEDS_HUMAN.md UI-BRAND-001).
 */

export type ThemeName = 'light' | 'dark' | 'hc';
export type AppId = 'doc' | 'sheet' | 'slide';

/** Bluestar brand colour (provisional until confirmed from the official logo). */
export const BRAND = '#1F6FEB';

/** 12-step neutral scale, 1 = lightest surface … 12 = strongest text (light theme). */
const GRAY_LIGHT = [
  '#fcfcfd',
  '#f7f8fa',
  '#eef1f5',
  '#e6e9ef',
  '#dde2e9',
  '#d3d9e2',
  '#c3cad6',
  '#a9b2c1',
  '#7c8696',
  '#6d7787',
  '#4f5a6a',
  '#1b2430',
] as const;
const GRAY_DARK = [
  '#111418',
  '#171b21',
  '#1e232b',
  '#242a33',
  '#2a313b',
  '#323a46',
  '#3e4855',
  '#546070',
  '#6c788a',
  '#7d899a',
  '#a4afbd',
  '#e3e9f1',
] as const;
const GRAY_HC = [
  '#000000',
  '#000000',
  '#0a0a0a',
  '#141414',
  '#1f1f1f',
  '#ffffff',
  '#ffffff',
  '#ffffff',
  '#ffffff',
  '#ffffff',
  '#ffffff',
  '#ffffff',
] as const;

/** One app accent in one theme. */
export interface Accent {
  /** Identity colour: app icon, title-bar stripe, selected tab underline. */
  base: string;
  /** Filled backgrounds carrying `on` text (primary button, File tab). */
  strong: string;
  /** Text and icons drawn on `strong`. */
  on: string;
  /** Accent-coloured text / outlines on the surface. */
  text: string;
  /** Tinted background (selected item, hover on accent). */
  subtle: string;
}

/** App accents: indigo (BlueDoc), teal (BlueSheet), coral (BlueSlide) — DESIGN.md "Màu". */
export const ACCENTS: Record<ThemeName, Record<AppId | 'brand', Accent>> = {
  light: {
    brand: { base: BRAND, strong: '#1A5FCC', on: '#ffffff', text: '#1A5FCC', subtle: '#E8F0FD' },
    doc: { base: '#4F46E5', strong: '#4F46E5', on: '#ffffff', text: '#4338CA', subtle: '#EEEDFC' },
    sheet: {
      base: '#0F9D8A',
      strong: '#0B7A6B',
      on: '#ffffff',
      text: '#0B7A6B',
      subtle: '#E3F4F1',
    },
    slide: {
      base: '#E0457B',
      strong: '#C2185B',
      on: '#ffffff',
      text: '#B5195A',
      subtle: '#FCE9F0',
    },
  },
  dark: {
    brand: {
      base: '#4C8DF0',
      strong: '#1A5FCC',
      on: '#ffffff',
      text: '#7FB0FF',
      subtle: '#1A2C47',
    },
    doc: { base: '#7C75F0', strong: '#4F46E5', on: '#ffffff', text: '#A9A4FF', subtle: '#25234A' },
    sheet: {
      base: '#2CC2AD',
      strong: '#0B7A6B',
      on: '#ffffff',
      text: '#4FD6C2',
      subtle: '#14332F',
    },
    slide: {
      base: '#F0709C',
      strong: '#C2185B',
      on: '#ffffff',
      text: '#FF8FB4',
      subtle: '#3D1A28',
    },
  },
  hc: {
    brand: {
      base: '#FFD600',
      strong: '#FFD600',
      on: '#000000',
      text: '#FFD600',
      subtle: '#1f1f1f',
    },
    doc: { base: '#FFD600', strong: '#FFD600', on: '#000000', text: '#FFD600', subtle: '#1f1f1f' },
    sheet: {
      base: '#FFD600',
      strong: '#FFD600',
      on: '#000000',
      text: '#FFD600',
      subtle: '#1f1f1f',
    },
    slide: {
      base: '#FFD600',
      strong: '#FFD600',
      on: '#000000',
      text: '#FFD600',
      subtle: '#1f1f1f',
    },
  },
};

/** Status colours in three steps: tinted background, border/icon, text. */
export interface Status {
  bg: string;
  border: string;
  text: string;
}
export type StatusName = 'success' | 'warning' | 'danger' | 'info';

export const STATUS: Record<ThemeName, Record<StatusName, Status>> = {
  light: {
    success: { bg: '#E6F4EA', border: '#2E8540', text: '#1E6B30' },
    warning: { bg: '#FFF4CE', border: '#A87400', text: '#7A4F00' },
    danger: { bg: '#FDECEA', border: '#D93025', text: '#B42318' },
    info: { bg: '#E8F0FD', border: '#1F6FEB', text: '#1A5FCC' },
  },
  dark: {
    success: { bg: '#15301D', border: '#4CAF68', text: '#7DD69A' },
    warning: { bg: '#3A2E0A', border: '#E0A800', text: '#F5CF5B' },
    danger: { bg: '#3B1715', border: '#F2685C', text: '#FF8A80' },
    info: { bg: '#1A2C47', border: '#4C8DF0', text: '#7FB0FF' },
  },
  hc: {
    success: { bg: '#000000', border: '#3FF23F', text: '#3FF23F' },
    warning: { bg: '#000000', border: '#FFD600', text: '#FFD600' },
    danger: { bg: '#000000', border: '#FF6B6B', text: '#FF6B6B' },
    info: { bg: '#000000', border: '#6FC3FF', text: '#6FC3FF' },
  },
};

/** Semantic surface/text roles used across the UI (`--bo-<role>`). */
export interface Semantic {
  bg: string;
  surface: string;
  'surface-2': string;
  border: string;
  text: string;
  'text-muted': string;
  hover: string;
  active: string;
  focus: string;
  'canvas-bg': string;
  /** Title bar / header background (Bluestar brand) and its text. */
  brand: string;
  'brand-strong': string;
  'brand-contrast': string;
}

function semantic(theme: ThemeName): Semantic {
  const g = theme === 'light' ? GRAY_LIGHT : theme === 'dark' ? GRAY_DARK : GRAY_HC;
  if (theme === 'hc') {
    return {
      bg: '#000000',
      surface: '#000000',
      'surface-2': '#0a0a0a',
      border: '#ffffff',
      text: '#ffffff',
      'text-muted': '#ffffff',
      hover: 'rgba(255, 214, 0, 0.2)',
      active: 'rgba(255, 214, 0, 0.35)',
      focus: '#FFD600',
      'canvas-bg': '#000000',
      brand: '#000000',
      'brand-strong': '#000000',
      'brand-contrast': '#ffffff',
    };
  }
  const light = theme === 'light';
  return {
    bg: g[2],
    surface: light ? '#ffffff' : '#1a1f27',
    'surface-2': light ? g[1] : g[3],
    border: g[5],
    text: g[11],
    'text-muted': g[10],
    hover: light ? 'rgba(27, 36, 48, 0.06)' : 'rgba(227, 233, 241, 0.08)',
    active: light ? 'rgba(27, 36, 48, 0.12)' : 'rgba(227, 233, 241, 0.16)',
    focus: light ? BRAND : '#7FB0FF',
    'canvas-bg': light ? '#e3e7ee' : '#0d1015',
    brand: light ? '#1A5FCC' : '#173B73',
    'brand-strong': light ? '#154FA9' : '#102C57',
    'brand-contrast': light ? '#ffffff' : '#E8EEF7',
  };
}

/**
 * Chrome drawn over the document canvas (`--bo-chrome-*`, CORE-UI-007): caret, selection,
 * selection handles, the cell cursor, comment markers and ruler marks. The paper keeps the
 * colours of the document (white unless the document says otherwise), so these are chosen
 * against `PAPER`, not against the UI surface. `halo` is the second tone drawn around the
 * mark: with it the mark stays visible on dark document content as well.
 */
export interface Chrome {
  caret: string;
  /** Selection tint: `selection` at `selection-alpha` opacity over the content. */
  selection: string;
  'selection-alpha': string;
  /** Outline of the selection / cell cursor / selected object. */
  cursor: string;
  /** Fill and outline of resize handles. */
  handle: string;
  'handle-border': string;
  /** Comment and note markers. */
  comment: string;
  /** Second tone around carets, cursors and handles (transparent where not needed). */
  halo: string;
  /** Width of focus rings and of the cursor outline. */
  'focus-width': string;
}

/** Paper colour the canvas chrome is checked against. */
export const PAPER = '#ffffff';

export const CHROME: Record<ThemeName, Chrome> = {
  light: {
    caret: '#000000',
    // Neutral grey over the text, as word processors show a selection (black text stays
    // black, the white paper turns #CCCCCC).
    selection: '#000000',
    'selection-alpha': '0.2',
    cursor: '#1A5FCC',
    handle: '#ffffff',
    'handle-border': '#1A5FCC',
    comment: '#B45309',
    halo: 'transparent',
    'focus-width': '2px',
  },
  dark: {
    caret: '#000000',
    // Neutral grey over the text, as word processors show a selection (black text stays
    // black, the white paper turns #CCCCCC).
    selection: '#000000',
    'selection-alpha': '0.2',
    cursor: '#1A5FCC',
    handle: '#ffffff',
    'handle-border': '#1A5FCC',
    comment: '#B45309',
    halo: 'transparent',
    'focus-width': '2px',
  },
  hc: {
    caret: '#000000',
    selection: '#0037DA',
    'selection-alpha': '0.45',
    cursor: '#000000',
    handle: '#FFD600',
    'handle-border': '#000000',
    comment: '#B00020',
    halo: '#ffffff',
    'focus-width': '3px',
  },
};

/**
 * `forced-colors: active`: the same roles from the system palette. The halo is the opposite
 * system colour, so a mark is two-toned whatever the document content under it.
 */
export const CHROME_FORCED: Chrome = {
  caret: 'CanvasText',
  selection: 'Highlight',
  'selection-alpha': '0.45',
  cursor: 'Highlight',
  handle: 'HighlightText',
  'handle-border': 'Highlight',
  comment: 'LinkText',
  halo: 'Canvas',
  'focus-width': '3px',
};

export function gray(theme: ThemeName): readonly string[] {
  return theme === 'light' ? GRAY_LIGHT : theme === 'dark' ? GRAY_DARK : GRAY_HC;
}

export const THEMES: Record<ThemeName, Semantic> = {
  light: semantic('light'),
  dark: semantic('dark'),
  hc: semantic('hc'),
};

/** UI type scale (px). */
export const FONT_SIZES = [11, 12, 13, 14, 16, 20, 24, 32] as const;
/** UI font stack: Be Vietnam Pro / Inter (OFL), then system fonts. */
export const FONT_UI =
  "'Be Vietnam Pro', 'Inter', 'Segoe UI', system-ui, -apple-system, 'Noto Sans', Roboto, sans-serif";
/** 4px spacing grid: --bo-space-1 = 4px … --bo-space-8 = 32px. */
export const SPACE = [4, 8, 12, 16, 20, 24, 28, 32] as const;
export const RADIUS = { sm: 4, md: 8 } as const;
export const SHADOWS = {
  /** Menus, popovers, tooltips. */
  1: '0 2px 6px rgba(15, 23, 42, 0.12)',
  /** Dialogs. */
  2: '0 8px 24px rgba(15, 23, 42, 0.18)',
  /** Floating windows, drag images. */
  3: '0 16px 40px rgba(15, 23, 42, 0.24)',
} as const;
/** Motion durations (ms); all become 0 under prefers-reduced-motion. */
export const MOTION = { fast: 120, normal: 160, slow: 200 } as const;
/** Densities: ribbon/control heights (px). Touch keeps targets ≥ 40px. */
export const DENSITY = {
  normal: { control: 28, ribbonLarge: 66 },
  compact: { control: 24, ribbonLarge: 58 },
  touch: { control: 40, ribbonLarge: 76 },
} as const;

const vars = (o: Record<string, string>, prefix = '--bo-'): string =>
  Object.entries(o)
    .map(([k, v]) => `  ${prefix}${k}: ${v};`)
    .join('\n');

function themeBlock(theme: ThemeName): string {
  const lines: string[] = [vars(THEMES[theme] as unknown as Record<string, string>)];
  lines.push(
    gray(theme)
      .map((c, i) => `  --bo-gray-${i + 1}: ${c};`)
      .join('\n'),
  );
  lines.push(vars(CHROME[theme] as unknown as Record<string, string>, '--bo-chrome-'));
  for (const [name, s] of Object.entries(STATUS[theme])) {
    lines.push(
      `  --bo-${name}-bg: ${s.bg};\n  --bo-${name}-border: ${s.border};\n  --bo-${name}-text: ${s.text};`,
    );
  }
  for (const [app, a] of Object.entries(ACCENTS[theme])) {
    lines.push(
      Object.entries(a)
        .map(([k, v]) => `  --bo-${app}-accent${k === 'base' ? '' : `-${k}`}: ${v};`)
        .join('\n'),
    );
  }
  // Legacy aliases kept for existing component CSS.
  lines.push(
    [
      '  --bo-surface-alt: var(--bo-surface-2);',
      '  --bo-muted: var(--bo-text-muted);',
      '  --bo-danger: var(--bo-danger-text);',
    ].join('\n'),
  );
  // Outside an app (shared screens) the accent is the Bluestar brand; [data-app] overrides below.
  lines.push(
    ['', '-strong', '-on', '-text', '-subtle']
      .map((s) => `  --bo-app-accent${s}: var(--bo-brand-accent${s});`)
      .join('\n') + '\n  --bo-accent: var(--bo-app-accent-text);',
  );
  lines.push(`  color-scheme: ${theme === 'light' ? 'light' : 'dark'};`);
  return lines.join('\n');
}

/** Render `tokens.css` from the constants above. */
export function renderTokensCss(): string {
  const scale = [
    `  --bo-font: ${FONT_UI};`,
    '  --bo-font-ui: var(--bo-font);',
    ...FONT_SIZES.map((s) => `  --bo-font-size-${s}: ${s}px;`),
    ...SPACE.map((s, i) => `  --bo-space-${i + 1}: ${s}px;`),
    `  --bo-radius-sm: ${RADIUS.sm}px;`,
    `  --bo-radius-md: ${RADIUS.md}px;`,
    '  --bo-radius: var(--bo-radius-sm);',
    ...Object.entries(SHADOWS).map(([k, v]) => `  --bo-shadow-${k}: ${v};`),
    ...Object.entries(MOTION).map(([k, v]) => `  --bo-duration-${k}: ${v}ms;`),
    '  --bo-ease: cubic-bezier(0.2, 0, 0, 1);',
    `  --bo-control-h: ${DENSITY.normal.control}px;`,
    `  --bo-ribbon-large-h: ${DENSITY.normal.ribbonLarge}px;`,
  ].join('\n');
  const apps = (['doc', 'sheet', 'slide'] as const)
    .map(
      (app) =>
        `[data-app='${app}'] {\n` +
        ['', '-strong', '-on', '-text', '-subtle']
          .map((s) => `  --bo-app-accent${s}: var(--bo-${app}-accent${s});`)
          .join('\n') +
        '\n  --bo-accent: var(--bo-app-accent-text);\n}',
    )
    .join('\n\n');
  const density = (['compact', 'touch'] as const)
    .map(
      (d) =>
        `[data-density='${d}'] {\n  --bo-control-h: ${DENSITY[d].control}px;\n` +
        `  --bo-ribbon-large-h: ${DENSITY[d].ribbonLarge}px;\n}`,
    )
    .join('\n\n');
  return `/* Generated from tokens.ts — do not edit by hand (BO_WRITE_TOKENS=1 pnpm --filter @blueoffice/ui test). */
:root {
${scale}
${themeBlock('light')}
}

[data-theme='dark'] {
${themeBlock('dark')}
}

[data-theme='hc'] {
${themeBlock('hc')}
}

${apps}

${density}

@media (forced-colors: active) {
  :root,
  [data-theme] {
${Object.entries(CHROME_FORCED)
  .map(([k, v]) => `    --bo-chrome-${k}: ${v};`)
  .join('\n')}
  }
}

@media (prefers-reduced-motion: reduce) {
  :root {
${Object.keys(MOTION)
  .map((k) => `    --bo-duration-${k}: 0ms;`)
  .join('\n')}
  }
}
`;
}
