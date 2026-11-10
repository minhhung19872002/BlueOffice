/**
 * Hyperlinks and actions (SLIDE-ACTION-001), the pure part: what a slide show
 * may do with an action (docs/security.md), the hot areas of a slide, mail
 * links, the built-in sounds and the action buttons. No DOM; unit-tested.
 */
import type {
  SlideActionOut,
  SlideActionSoundSpec,
  SlideActionSpec,
  SlideActionTrigger,
  SlideLinkAction,
  SlideShowLink,
} from '@blueoffice/platform';

// ------------------------------------------------------------------ policy

/** Why an action is not run. */
export type BlockedReason = 'program' | 'macro' | 'file' | 'presentation' | 'scheme' | 'unknown';

/** What the show does with an action. */
export type ActionPolicy =
  /** A jump inside the deck (or nothing but a sound / highlight): followed at once. */
  | { kind: 'internal' }
  /** A web page or mail address: opened by the host after the user confirms. */
  | { kind: 'external'; url: string; mail: boolean }
  /** Never opened or run; the show says so. */
  | { kind: 'blocked'; reason: BlockedReason; detail: string };

/**
 * Whether a URL may be handed to the host to open: `http(s)://host…` or
 * `mailto:…`, written plainly — no whitespace or control characters anywhere
 * (they hide other schemes from a reader: `java\tscript:`).
 */
export function isAllowedUrl(url: string): boolean {
  for (const ch of url) {
    const c = ch.codePointAt(0) ?? 0;
    if (c <= 0x20 || (c >= 0x7f && c <= 0x9f) || /\s/.test(ch)) return false;
  }
  const colon = url.indexOf(':');
  if (colon <= 0) return false;
  const scheme = url.slice(0, colon).toLowerCase();
  const rest = url.slice(colon + 1);
  if (scheme === 'http' || scheme === 'https') {
    return rest.startsWith('//') && rest.length > 2 && !rest.startsWith('///');
  }
  return scheme === 'mailto' && rest.length > 0;
}

/**
 * The policy for a link of the show. The action itself decides; the engine's
 * `safety` can only make it stricter (a link the engine blocked stays blocked).
 */
export function actionPolicy(link: Pick<SlideShowLink, 'action' | 'safety'>): ActionPolicy {
  const a = link.action;
  const blocked = (reason: BlockedReason, detail: string): ActionPolicy => ({
    kind: 'blocked',
    reason,
    detail,
  });
  switch (a.kind) {
    case 'program':
      return blocked('program', a.path);
    case 'macro':
      return blocked('macro', a.name);
    case 'file':
      return blocked('file', a.path);
    case 'presentation':
      return blocked('presentation', a.path);
    case 'unknown':
      return blocked('unknown', a.action);
    case 'url':
      if (!isAllowedUrl(a.url) || link.safety === 'blocked') return blocked('scheme', a.url);
      return { kind: 'external', url: a.url, mail: /^mailto:/i.test(a.url) };
    case 'none':
    case 'customShow':
    case 'slide':
    case 'next':
    case 'prev':
    case 'first':
    case 'last':
    case 'lastViewed':
    case 'end':
      return link.safety === 'blocked' || link.safety === 'external'
        ? blocked('unknown', a.kind)
        : { kind: 'internal' };
    default:
      // A kind this version does not know: never run.
      return blocked('unknown', String((a as { kind?: unknown }).kind ?? ''));
  }
}

// ------------------------------------------------------------------ hot areas

/** One clickable / hoverable box of the show with its click and hover action. */
export interface HotArea {
  /** React key and test id suffix: the shape id, plus an index after the first area. */
  id: string;
  shapeId: number;
  rect: [number, number, number, number];
  click?: SlideShowLink;
  hover?: SlideShowLink;
  /** Screen tip: the tooltip, else the address of a web link. */
  title?: string;
}

function tipOf(l: SlideShowLink | undefined): string | undefined {
  if (!l) return undefined;
  if (l.tooltip) return l.tooltip;
  return l.action.kind === 'url' ? l.action.url : undefined;
}

/**
 * The links of a slide as boxes, bottom to top: a click and a hover action over
 * the same box share it (so hovering does not hide the click).
 */
export function hotAreas(links: SlideShowLink[]): HotArea[] {
  const out: HotArea[] = [];
  const perShape = new Map<number, number>();
  for (const l of links) {
    const on: SlideActionTrigger = l.on ?? 'click';
    const same = out.find(
      (a) =>
        a.shapeId === l.shapeId &&
        a.rect.every((v, i) => Math.abs(v - l.rect[i]!) < 1e-6) &&
        !a[on],
    );
    if (same) {
      same[on] = l;
      same.title = tipOf(same.click) ?? tipOf(same.hover);
      continue;
    }
    const n = perShape.get(l.shapeId) ?? 0;
    perShape.set(l.shapeId, n + 1);
    out.push({
      id: n === 0 ? String(l.shapeId) : `${l.shapeId}-${n}`,
      shapeId: l.shapeId,
      rect: l.rect,
      [on]: l,
      title: tipOf(l),
    });
  }
  return out;
}

// ------------------------------------------------------------------ mail links

/** `mailto:` URL of an address and a subject (RFC 6068). */
export function mailtoUrl(address: string, subject = ''): string {
  const to = address
    .trim()
    .replace(/^mailto:/i, '')
    .split(/[\s,;]+/)
    .filter(Boolean)
    .map((a) => encodeURIComponent(a).replace(/%40/gi, '@'))
    .join(',');
  const s = subject.trim();
  return `mailto:${to}${s ? `?subject=${encodeURIComponent(s)}` : ''}`;
}

/** Address and subject of a `mailto:` URL; null for any other URL. */
export function parseMailto(url: string): { address: string; subject: string } | null {
  if (!/^mailto:/i.test(url)) return null;
  const [to = '', query = ''] = url.slice(7).split('?', 2);
  const dec = (s: string) => {
    try {
      return decodeURIComponent(s);
    } catch {
      return s;
    }
  };
  const subject = query
    .split('&')
    .map((kv) => kv.split('=', 2))
    .find(([k]) => k?.toLowerCase() === 'subject')?.[1];
  return { address: dec(to), subject: subject ? dec(subject) : '' };
}

/** A plausible mail address (one `@`, something on both sides, a dot in the domain). */
export function isMailAddress(s: string): boolean {
  return /^[^\s@,;]+@[^\s@,;]+\.[^\s@,;]+$/.test(s.trim());
}

/** A web address as typed: `example.vn` gets `https://`. */
export function normalizeUrl(typed: string): string {
  const s = typed.trim();
  if (!s) return '';
  if (/^[a-z][a-z0-9+.-]*:/i.test(s)) return s;
  return `https://${s}`;
}

// ------------------------------------------------------------------ dialog model

/** The choices of "hyperlink to" in the action dialog. */
export const JUMP_KINDS = ['next', 'prev', 'first', 'last', 'lastViewed', 'end'] as const;
export type JumpKind = (typeof JUMP_KINDS)[number];

export function isJump(a: SlideLinkAction): a is { kind: JumpKind } {
  return (JUMP_KINDS as readonly string[]).includes(a.kind);
}

/** Actions the editor can write (the engine refuses the others). */
export function isEditable(a: SlideLinkAction): boolean {
  return a.kind !== 'program' && a.kind !== 'macro' && a.kind !== 'unknown';
}

// ------------------------------------------------------------------ action buttons

/** Action buttons (§20.1.10.56 ST_ShapeType `actionButton*`) and what each starts with. */
export const ACTION_BUTTONS: { preset: string; action: SlideLinkAction['kind'] }[] = [
  { preset: 'actionButtonBackPrevious', action: 'prev' },
  { preset: 'actionButtonForwardNext', action: 'next' },
  { preset: 'actionButtonBeginning', action: 'first' },
  { preset: 'actionButtonEnd', action: 'last' },
  { preset: 'actionButtonHome', action: 'first' },
  { preset: 'actionButtonReturn', action: 'lastViewed' },
  { preset: 'actionButtonInformation', action: 'none' },
  { preset: 'actionButtonHelp', action: 'none' },
  { preset: 'actionButtonDocument', action: 'none' },
  { preset: 'actionButtonSound', action: 'none' },
  { preset: 'actionButtonMovie', action: 'none' },
  { preset: 'actionButtonBlank', action: 'none' },
];

// ------------------------------------------------------------------ built-in sounds

/** Built-in sounds: short tones synthesised here (no recorded material). */
export const BUILTIN_SOUNDS = ['click', 'beep', 'chime', 'drum'] as const;
export type BuiltinSound = (typeof BUILTIN_SOUNDS)[number];

const RATE = 22_050;

/** Samples (−1…1) of a built-in sound. */
function samples(id: BuiltinSound): Float32Array {
  const tone = (out: Float32Array, hz: number, from: number, len: number, decay: number) => {
    const start = Math.floor(from * RATE);
    const n = Math.floor(len * RATE);
    for (let i = 0; i < n && start + i < out.length; i++) {
      const t = i / RATE;
      // A short attack avoids a click at the start; the tail decays exponentially.
      const env = Math.min(1, i / (0.004 * RATE)) * Math.exp(-t * decay);
      out[start + i] = (out[start + i] ?? 0) + Math.sin(2 * Math.PI * hz * t) * env;
    }
  };
  switch (id) {
    case 'click': {
      const out = new Float32Array(Math.floor(0.05 * RATE));
      tone(out, 1800, 0, 0.05, 110);
      tone(out, 900, 0, 0.05, 90);
      return out;
    }
    case 'beep': {
      const out = new Float32Array(Math.floor(0.18 * RATE));
      tone(out, 880, 0, 0.18, 6);
      // Fade the end out.
      const fade = Math.floor(0.02 * RATE);
      for (let i = 0; i < fade; i++) {
        const k = out.length - 1 - i;
        out[k] = (out[k] ?? 0) * (i / fade);
      }
      return out;
    }
    case 'chime': {
      const out = new Float32Array(Math.floor(0.7 * RATE));
      tone(out, 1046.5, 0, 0.7, 5);
      tone(out, 1318.5, 0.12, 0.58, 5);
      tone(out, 1568, 0.24, 0.46, 5);
      return out;
    }
    case 'drum': {
      const out = new Float32Array(Math.floor(0.25 * RATE));
      const n = out.length;
      for (let i = 0; i < n; i++) {
        const t = i / RATE;
        // The pitch falls from 160 Hz to 50 Hz.
        const hz = 50 + 110 * Math.exp(-t * 25);
        out[i] = Math.sin(2 * Math.PI * hz * t) * Math.exp(-t * 16);
      }
      return out;
    }
  }
}

/** A built-in sound as a WAV file (RIFF, PCM 16-bit mono, 22.05 kHz). */
export function builtinWav(id: BuiltinSound): Uint8Array {
  const s = samples(id);
  let peak = 0;
  for (const v of s) peak = Math.max(peak, Math.abs(v));
  const gain = peak > 0 ? 0.8 / peak : 1;
  const data = s.length * 2;
  const buf = new ArrayBuffer(44 + data);
  const v = new DataView(buf);
  const ascii = (at: number, text: string) => {
    for (let i = 0; i < text.length; i++) v.setUint8(at + i, text.charCodeAt(i));
  };
  ascii(0, 'RIFF');
  v.setUint32(4, 36 + data, true);
  ascii(8, 'WAVE');
  ascii(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true); // PCM
  v.setUint16(22, 1, true); // mono
  v.setUint32(24, RATE, true);
  v.setUint32(28, RATE * 2, true); // bytes per second
  v.setUint16(32, 2, true); // block align
  v.setUint16(34, 16, true); // bits per sample
  ascii(36, 'data');
  v.setUint32(40, data, true);
  for (let i = 0; i < s.length; i++) {
    const x = Math.max(-1, Math.min(1, (s[i] ?? 0) * gain));
    v.setInt16(44 + i * 2, Math.round(x * 32767), true);
  }
  return new Uint8Array(buf);
}

/** Bytes → base64 (a WAV for the engine). */
export function base64Of(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

/** File name a built-in sound is stored under (`a:snd@name`). */
export function builtinName(id: BuiltinSound): string {
  return `${id}.wav`;
}

/** The built-in sound a stored sound name stands for, if any. */
export function builtinOf(name: string | undefined): BuiltinSound | null {
  const id = (name ?? '').replace(/\.wav$/i, '');
  return (BUILTIN_SOUNDS as readonly string[]).includes(id) ? (id as BuiltinSound) : null;
}

// ------------------------------------------------------------------ dialog drafts

/** Where "hyperlink to" of the action dialog leads. */
export type LinkToKind = JumpKind | 'slide' | 'customShow' | 'url' | 'file';

/** The sound choice of the action dialog. */
export type SoundChoice = 'none' | 'stop' | 'keep' | 'file' | BuiltinSound;

/** One tab (click or hover) of the action dialog while it is edited. */
export interface ActionDraft {
  /** `keep`: an action the editor cannot write (program, macro…) is left as it is. */
  mode: 'none' | 'link' | 'keep';
  to: LinkToKind;
  /** 0-based slide for `slide`. */
  slide: number;
  show: number | null;
  ret: boolean;
  url: string;
  path: string;
  sound: SoundChoice;
  /** Name of the sound the action has now (`keep`). */
  soundName: string;
  /** A picked WAV file (`file`). */
  file: { name: string; data: string } | null;
  highlight: boolean;
  tooltip: string;
}

/** The draft of an action as it is in the deck (`null`: no action). */
export function draftOf(a: SlideActionOut | null, firstShow: number | null = null): ActionDraft {
  const d: ActionDraft = {
    mode: 'none',
    to: 'next',
    slide: 0,
    show: firstShow,
    ret: true,
    url: '',
    path: '',
    sound: 'none',
    soundName: '',
    file: null,
    highlight: false,
    tooltip: '',
  };
  if (!a) return d;
  d.highlight = a.highlight;
  d.tooltip = a.tooltip ?? '';
  if (a.sound) {
    d.sound = a.sound.stop ? 'stop' : 'keep';
    d.soundName = a.sound.name;
  }
  const t = a.target;
  if (a.locked || !isEditable(t)) return { ...d, mode: 'keep' };
  if (t.kind === 'none') return d;
  d.mode = 'link';
  if (isJump(t)) d.to = t.kind;
  else if (t.kind === 'slide') {
    d.to = 'slide';
    d.slide = t.slide;
  } else if (t.kind === 'customShow') {
    d.to = 'customShow';
    d.show = t.id;
    d.ret = !!t.return;
  } else if (t.kind === 'url') {
    d.to = 'url';
    d.url = t.url;
  } else if (t.kind === 'file' || t.kind === 'presentation') {
    d.to = 'file';
    d.path = t.path;
  }
  return d;
}

/** A path to another presentation (`ppaction://hlinkpres`) rather than any file. */
function isDeckPath(path: string): boolean {
  return /\.(pptx|pptm|ppsx|ppsm|potx|ppt|pps|odp)$/i.test(path.trim());
}

function fileTarget(typed: string): SlideLinkAction | null {
  const path = typed.trim();
  if (!path) return null;
  return isDeckPath(path) ? { kind: 'presentation', path } : { kind: 'file', path };
}

/** The target a draft stands for; `null` while it is incomplete or not allowed. */
export function targetOf(d: ActionDraft): SlideLinkAction | null {
  if (d.mode !== 'link') return { kind: 'none' };
  switch (d.to) {
    case 'slide':
      return Number.isInteger(d.slide) && d.slide >= 0 ? { kind: 'slide', slide: d.slide } : null;
    case 'customShow':
      return d.show === null ? null : { kind: 'customShow', id: d.show, return: d.ret };
    case 'url': {
      const url = normalizeUrl(d.url);
      return isAllowedUrl(url) ? { kind: 'url', url } : null;
    }
    case 'file':
      return fileTarget(d.path);
    default:
      return { kind: d.to };
  }
}

/** The command payload of a draft; `null`: nothing to send (kept or incomplete). */
export function specOf(d: ActionDraft): SlideActionSpec | null {
  if (d.mode === 'keep') return null;
  const target = targetOf(d);
  if (!target) return null;
  let sound: SlideActionSoundSpec;
  if (d.sound === 'none' || d.sound === 'stop' || d.sound === 'keep') sound = { kind: d.sound };
  else if (d.sound === 'file') {
    if (!d.file) return null;
    sound = { kind: 'file', name: d.file.name, data: d.file.data };
  } else {
    sound = { kind: 'file', name: builtinName(d.sound), data: base64Of(builtinWav(d.sound)) };
  }
  const tooltip = d.tooltip.trim();
  return {
    target,
    ...(tooltip ? { tooltip } : {}),
    highlight: d.highlight,
    sound,
  };
}

/** The kinds of the link dialog. */
export type LinkKind = 'url' | 'place' | 'show' | 'mail' | 'file';

/** A place in this deck: a slide (0-based) or a relative one. */
export type LinkPlace =
  { kind: 'slide'; slide: number } | { kind: 'first' | 'last' | 'next' | 'prev' };

/** The link dialog while it is edited. */
export interface LinkDraft {
  kind: LinkKind;
  url: string;
  place: LinkPlace;
  show: number | null;
  ret: boolean;
  address: string;
  subject: string;
  path: string;
  tooltip: string;
}

/** The link draft of the click action as it is in the deck. */
export function linkDraftOf(a: SlideActionOut | null, firstShow: number | null = null): LinkDraft {
  const d: LinkDraft = {
    kind: 'url',
    url: '',
    place: { kind: 'next' },
    show: firstShow,
    ret: true,
    address: '',
    subject: '',
    path: '',
    tooltip: a?.tooltip ?? '',
  };
  const t = a?.target;
  if (!t) return d;
  if (t.kind === 'url') {
    const mail = parseMailto(t.url);
    return mail ? { ...d, kind: 'mail', ...mail } : { ...d, url: t.url };
  }
  if (t.kind === 'slide') return { ...d, kind: 'place', place: { kind: 'slide', slide: t.slide } };
  if (t.kind === 'first' || t.kind === 'last' || t.kind === 'next' || t.kind === 'prev') {
    return { ...d, kind: 'place', place: { kind: t.kind } };
  }
  if (t.kind === 'customShow') return { ...d, kind: 'show', show: t.id, ret: !!t.return };
  if (t.kind === 'file' || t.kind === 'presentation') return { ...d, kind: 'file', path: t.path };
  return d;
}

/** The target of a link draft; `null` while it is incomplete or not allowed. */
export function linkTargetOf(d: LinkDraft): SlideLinkAction | null {
  switch (d.kind) {
    case 'url': {
      const url = normalizeUrl(d.url);
      // Web pages only: mail has its own tab, other schemes are never opened.
      return isAllowedUrl(url) && !/^mailto:/i.test(url) ? { kind: 'url', url } : null;
    }
    case 'place':
      return d.place;
    case 'show':
      return d.show === null ? null : { kind: 'customShow', id: d.show, return: d.ret };
    case 'mail':
      return isMailAddress(d.address)
        ? { kind: 'url', url: mailtoUrl(d.address, d.subject) }
        : null;
    case 'file':
      return fileTarget(d.path);
  }
}
