import { describe, expect, it } from 'vitest';
import type {
  SlideActionOut,
  SlideCustomShow,
  SlideLinkAction,
  SlideShowLink,
} from '@blueoffice/platform';
import {
  ACTION_BUTTONS,
  BUILTIN_SOUNDS,
  actionPolicy,
  base64Of,
  builtinName,
  builtinOf,
  builtinWav,
  draftOf,
  hotAreas,
  isAllowedUrl,
  isMailAddress,
  linkDraftOf,
  linkTargetOf,
  mailtoUrl,
  normalizeUrl,
  parseMailto,
  specOf,
  targetOf,
} from '../src/actions';
import { newRun, runLink, runStep, type RunPos, type ShowRun } from '../src/presenting';

const link = (action: SlideLinkAction, more: Partial<SlideShowLink> = {}): SlideShowLink => ({
  shapeId: 4,
  rect: [10, 20, 100, 40],
  action,
  ...more,
});

// source: docs/security.md — the slide show policy for hyperlinks and actions.
describe('security policy of the slide show', () => {
  it('opens only web and mail addresses written plainly', () => {
    for (const ok of [
      'http://example.vn',
      'https://example.vn/a?b=c#d',
      'HTTPS://EXAMPLE.VN',
      'mailto:a@example.vn?subject=Hi',
    ]) {
      expect(isAllowedUrl(ok), ok).toBe(true);
    }
    for (const bad of [
      '',
      'example.vn',
      'file:///C:/Windows/System32/calc.exe',
      'FILE://server/share/run.bat',
      'C:\\Windows\\System32\\calc.exe',
      '\\\\server\\share\\run.bat',
      'javascript:alert(1)',
      'JaVaScRiPt:alert(1)',
      'data:text/html,<script>1</script>',
      'vbscript:msgbox(1)',
      'ms-msdt:/id PCWDiagnostic',
      'search-ms:query=x',
      'ftp://example.vn/x',
      'bluedoc://open?file=x',
      'blob:https://example.vn/1',
      ' https://example.vn',
      'https://exa mple.vn',
      'java\tscript:alert(1)',
      'https://example.vn/\n',
      'https://example.vn/\u0000',
      'https:example.vn',
      'https:///etc/passwd',
      'http://',
      'mailto:',
      ':https://example.vn',
    ]) {
      expect(isAllowedUrl(bad), JSON.stringify(bad)).toBe(false);
    }
  });

  it('never runs programs, macros, files, other decks or unknown actions', () => {
    const blocked: [SlideLinkAction, string][] = [
      [{ kind: 'program', path: 'file:///C:/Windows/System32/calc.exe' }, 'program'],
      [{ kind: 'program', path: 'https://example.vn/tool.exe' }, 'program'],
      [{ kind: 'macro', name: 'AutoRun' }, 'macro'],
      [{ kind: 'file', path: 'report.docx' }, 'file'],
      [{ kind: 'file', path: 'https://example.vn/report.docx' }, 'file'],
      [{ kind: 'presentation', path: 'other.pptx', slide: 2 }, 'presentation'],
      [{ kind: 'unknown', action: 'ppaction://frobnicate?x=1' }, 'unknown'],
      [{ kind: 'url', url: 'file:///C:/Windows/System32/cmd.exe' }, 'scheme'],
      [{ kind: 'url', url: 'javascript:alert(1)' }, 'scheme'],
    ];
    for (const [action, reason] of blocked) {
      // Whatever the engine said about it — even "internal" — it stays blocked.
      for (const safety of [undefined, 'blocked', 'external', 'internal'] as const) {
        const p = actionPolicy(link(action, { safety }));
        expect(p.kind, JSON.stringify(action)).toBe('blocked');
        expect(p.kind === 'blocked' && p.reason).toBe(reason);
      }
    }
    // A kind from a newer engine is not run either.
    const future = { kind: 'shell', command: 'calc' } as unknown as SlideLinkAction;
    expect(actionPolicy(link(future)).kind).toBe('blocked');
  });

  it('asks before a web or mail address and follows jumps at once', () => {
    expect(actionPolicy(link({ kind: 'url', url: 'https://example.vn/a' }))).toEqual({
      kind: 'external',
      url: 'https://example.vn/a',
      mail: false,
    });
    expect(
      actionPolicy(link({ kind: 'url', url: 'mailto:a@example.vn' }, { safety: 'external' })),
    ).toEqual({ kind: 'external', url: 'mailto:a@example.vn', mail: true });
    // The engine can only make it stricter.
    expect(
      actionPolicy(link({ kind: 'url', url: 'https://example.vn' }, { safety: 'blocked' })).kind,
    ).toBe('blocked');
    const jumps: SlideLinkAction[] = [
      { kind: 'none' },
      { kind: 'next' },
      { kind: 'lastViewed' },
      { kind: 'end' },
      { kind: 'slide', slide: 2 },
      { kind: 'customShow', id: 0, return: true },
    ];
    for (const a of jumps) {
      expect(actionPolicy(link(a)).kind).toBe('internal');
      expect(actionPolicy(link(a, { safety: 'internal' })).kind).toBe('internal');
      expect(actionPolicy(link(a, { safety: 'blocked' })).kind).toBe('blocked');
    }
  });
});

describe('hot areas of a slide', () => {
  it('joins the click and hover action of one box and numbers the areas of a shape', () => {
    const click = link({ kind: 'next' }, { on: 'click', tooltip: 'Tiếp' });
    const hover = link({ kind: 'none' }, { on: 'hover', highlight: true });
    const text = link(
      { kind: 'url', url: 'https://example.vn' },
      { rect: [12, 24, 30, 12], text: true },
    );
    const other = link({ kind: 'end' }, { shapeId: 9 });
    const areas = hotAreas([click, hover, text, other]);
    expect(areas.map((a) => a.id)).toEqual(['4', '4-1', '9']);
    expect(areas[0]!.click).toBe(click);
    expect(areas[0]!.hover).toBe(hover);
    expect(areas[0]!.title).toBe('Tiếp');
    // A web link without a tooltip shows its address.
    expect(areas[1]!.title).toBe('https://example.vn');
    expect(areas[1]!.hover).toBeUndefined();
    expect(areas[2]!.title).toBeUndefined();
    // Two click links over the same box stay two areas.
    expect(hotAreas([click, link({ kind: 'prev' })])).toHaveLength(2);
  });
});

describe('mail and web addresses', () => {
  it('builds and reads mailto links', () => {
    expect(mailtoUrl(' a@example.vn ', 'Xin chào & hẹn gặp')).toBe(
      'mailto:a@example.vn?subject=Xin%20ch%C3%A0o%20%26%20h%E1%BA%B9n%20g%E1%BA%B7p',
    );
    expect(mailtoUrl('mailto:a@example.vn; b@example.vn')).toBe('mailto:a@example.vn,b@example.vn');
    expect(parseMailto('mailto:a@example.vn?subject=Xin%20ch%C3%A0o')).toEqual({
      address: 'a@example.vn',
      subject: 'Xin chào',
    });
    expect(parseMailto('https://example.vn')).toBeNull();
    expect(isAllowedUrl(mailtoUrl('a@example.vn', 'Hi there'))).toBe(true);
    expect(isMailAddress('a@example.vn')).toBe(true);
    expect(isMailAddress('a@example')).toBe(false);
    expect(isMailAddress('a b@example.vn')).toBe(false);
  });

  it('adds https:// to an address typed without a scheme', () => {
    expect(normalizeUrl(' example.vn/a ')).toBe('https://example.vn/a');
    expect(normalizeUrl('http://example.vn')).toBe('http://example.vn');
    expect(normalizeUrl('file:///C:/x')).toBe('file:///C:/x');
    expect(normalizeUrl('')).toBe('');
  });
});

describe('built-in sounds', () => {
  // source: RIFF WAVE header layout (IBM / Microsoft "Multimedia Programming Interface and
  // Data Specifications 1.0"): fmt chunk of 16 bytes, PCM = 1.
  it('are valid short PCM WAV files', () => {
    for (const id of BUILTIN_SOUNDS) {
      const wav = builtinWav(id);
      const v = new DataView(wav.buffer);
      const ascii = (at: number) => String.fromCharCode(...wav.subarray(at, at + 4));
      expect(ascii(0)).toBe('RIFF');
      expect(ascii(8)).toBe('WAVE');
      expect(ascii(12)).toBe('fmt ');
      expect(ascii(36)).toBe('data');
      expect(v.getUint32(4, true)).toBe(wav.length - 8);
      expect(v.getUint16(20, true)).toBe(1);
      expect(v.getUint16(22, true)).toBe(1);
      expect(v.getUint32(24, true)).toBe(22_050);
      expect(v.getUint16(34, true)).toBe(16);
      expect(v.getUint32(40, true)).toBe(wav.length - 44);
      // Under a second, and not silence.
      const samples = (wav.length - 44) / 2;
      expect(samples).toBeGreaterThan(500);
      expect(samples).toBeLessThan(22_050);
      let peak = 0;
      for (let i = 0; i < samples; i++)
        peak = Math.max(peak, Math.abs(v.getInt16(44 + i * 2, true)));
      expect(peak).toBeGreaterThan(20_000);
      expect(peak).toBeLessThanOrEqual(32_767);
      expect(builtinOf(builtinName(id))).toBe(id);
    }
    expect(builtinOf('chuông.wav')).toBeNull();
    expect(base64Of(new Uint8Array([82, 73, 70, 70]))).toBe('UklGRg==');
  });

  it('every action button has a known default', () => {
    expect(ACTION_BUTTONS).toHaveLength(12);
    expect(new Set(ACTION_BUTTONS.map((b) => b.preset)).size).toBe(12);
    expect(ACTION_BUTTONS.every((b) => b.preset.startsWith('actionButton'))).toBe(true);
  });
});

describe('dialog drafts', () => {
  const out = (target: SlideLinkAction, more: Partial<SlideActionOut> = {}): SlideActionOut => ({
    target,
    highlight: false,
    safety: 'internal',
    locked: false,
    ...more,
  });

  it('reads an action into the action dialog and writes it back', () => {
    expect(specOf(draftOf(null))).toEqual({
      target: { kind: 'none' },
      highlight: false,
      sound: { kind: 'none' },
    });
    const d = draftOf(
      out(
        { kind: 'customShow', id: 4, return: true },
        {
          tooltip: 'Phụ lục',
          highlight: true,
          sound: { stop: false, part: '/ppt/media/audio1.wav', name: 'chuông.wav', loop: false },
        },
      ),
    );
    expect(d).toMatchObject({ mode: 'link', to: 'customShow', show: 4, ret: true, sound: 'keep' });
    expect(specOf(d)).toEqual({
      target: { kind: 'customShow', id: 4, return: true },
      tooltip: 'Phụ lục',
      highlight: true,
      sound: { kind: 'keep' },
    });
    // A built-in sound is sent as its WAV.
    const s = specOf({ ...draftOf(null), sound: 'chime' });
    expect(s?.sound).toMatchObject({ kind: 'file', name: 'chime.wav' });
    const sound = s?.sound;
    expect(sound?.kind === 'file' && atob(sound.data).slice(0, 4)).toBe('RIFF');
    // A picked file that is missing, an address that may not be opened: nothing to send.
    expect(specOf({ ...draftOf(null), sound: 'file' })).toBeNull();
    const web = { ...draftOf(null), mode: 'link' as const, to: 'url' as const };
    expect(targetOf({ ...web, url: 'example.vn' })).toEqual({
      kind: 'url',
      url: 'https://example.vn',
    });
    expect(specOf({ ...web, url: 'javascript:alert(1)' })).toBeNull();
    expect(specOf({ ...web, url: 'file:///C:/calc.exe' })).toBeNull();
    expect(targetOf({ ...web, to: 'file', path: 'bao-cao.docx' })).toEqual({
      kind: 'file',
      path: 'bao-cao.docx',
    });
    expect(targetOf({ ...web, to: 'file', path: 'bai-khac.PPTX' })).toEqual({
      kind: 'presentation',
      path: 'bai-khac.PPTX',
    });
  });

  it('keeps programs, macros, unknown and locked actions untouched', () => {
    const unsafe: SlideLinkAction[] = [
      { kind: 'program', path: 'calc.exe' },
      { kind: 'macro', name: 'M' },
      { kind: 'unknown', action: 'ppaction://x' },
    ];
    for (const target of unsafe) {
      const d = draftOf(out(target, { safety: 'blocked' }));
      expect(d.mode).toBe('keep');
      expect(specOf(d)).toBeNull();
      // Choosing something else replaces it — never with the unsafe action itself.
      expect(specOf({ ...d, mode: 'none' })?.target).toEqual({ kind: 'none' });
    }
    expect(
      draftOf(out({ kind: 'unknown', action: 'ppaction://media' }, { locked: true })).mode,
    ).toBe('keep');
  });

  it('reads and writes the link dialog', () => {
    expect(linkTargetOf(linkDraftOf(null))).toBeNull();
    const web = linkDraftOf(out({ kind: 'url', url: 'https://example.vn' }, { tooltip: 'Web' }));
    expect(web).toMatchObject({ kind: 'url', url: 'https://example.vn', tooltip: 'Web' });
    expect(linkTargetOf(web)).toEqual({ kind: 'url', url: 'https://example.vn' });
    const mail = linkDraftOf(out({ kind: 'url', url: 'mailto:a@example.vn?subject=Ch%C3%A0o' }));
    expect(mail).toMatchObject({ kind: 'mail', address: 'a@example.vn', subject: 'Chào' });
    expect(linkTargetOf(mail)).toEqual({
      kind: 'url',
      url: 'mailto:a@example.vn?subject=Ch%C3%A0o',
    });
    expect(linkTargetOf({ ...mail, address: 'không phải thư' })).toBeNull();
    const place = linkDraftOf(out({ kind: 'slide', slide: 2 }));
    expect(linkTargetOf(place)).toEqual({ kind: 'slide', slide: 2 });
    expect(linkTargetOf(linkDraftOf(out({ kind: 'last' })))).toEqual({ kind: 'last' });
    expect(linkTargetOf(linkDraftOf(out({ kind: 'customShow', id: 3 })))).toEqual({
      kind: 'customShow',
      id: 3,
      return: false,
    });
    // The web tab takes web pages only.
    expect(linkTargetOf({ ...web, url: 'mailto:a@example.vn' })).toBeNull();
    expect(linkTargetOf({ ...web, url: 'file:///C:/calc.exe' })).toBeNull();
  });
});

describe('links in a running show', () => {
  const shows: SlideCustomShow[] = [{ id: 0, name: 'Phụ lục', slides: [4, 5] }];
  const clicks = () => 0;
  const at = (run: ShowRun, i: number): RunPos => ({
    at: i,
    slide: run.order[i]!,
    click: 0,
    enter: 'transition',
  });
  const go = (run: ShowRun, pos: RunPos, action: SlideLinkAction) => {
    const r = runLink(run, pos, action, 6, clicks, shows);
    if (r === 'end') throw new Error('the show ended');
    return r;
  };

  it('keeps the last slide viewed across jumps', () => {
    const run = newRun([0, 1, 2, 3, 4, 5]);
    // Nothing viewed before the first slide: "last viewed" stays put.
    let r = go(run, at(run, 0), { kind: 'lastViewed' });
    expect(r.pos.slide).toBe(0);
    // 0 → slide 3 (link) → back to 0 → and "last viewed" from 0 is 3 again.
    r = go(run, at(run, 0), { kind: 'slide', slide: 3 });
    expect(r.pos.slide).toBe(3);
    r = go(r.run, r.pos, { kind: 'lastViewed' });
    expect(r.pos.slide).toBe(0);
    r = go(r.run, r.pos, { kind: 'lastViewed' });
    expect(r.pos.slide).toBe(3);
    // Plain navigation counts too: 3 → 4 (next), then "last viewed" is 3.
    const n = runStep(r.run, r.pos, 'next', 6, clicks);
    expect(n.pos.slide).toBe(4);
    r = go(n.run, n.pos, { kind: 'lastViewed' });
    expect(r.pos.slide).toBe(3);
    // First / last / previous.
    expect(go(r.run, r.pos, { kind: 'last' }).pos.slide).toBe(5);
    expect(go(r.run, r.pos, { kind: 'first' }).pos.slide).toBe(0);
    expect(go(r.run, r.pos, { kind: 'prev' }).pos.slide).toBe(2);
    expect(runLink(r.run, r.pos, { kind: 'end' }, 6, clicks, shows)).toBe('end');
  });

  it('returns from a custom show to the linking slide, or ends when it does not return', () => {
    const run = newRun([0, 1, 2, 3]);
    let r = go(run, at(run, 2), { kind: 'customShow', id: 0, return: true });
    expect(r.run.order).toEqual([4, 5]);
    expect(r.pos.slide).toBe(4);
    let n = runStep(r.run, r.pos, 'next', 6, clicks);
    expect(n.pos.slide).toBe(5);
    n = runStep(n.run, n.pos, 'next', 6, clicks);
    expect(n.run.order).toEqual([0, 1, 2, 3]);
    expect(n.pos).toMatchObject({ at: 2, slide: 2 });
    // Without "return" the custom show is the show from then on.
    r = go(run, at(run, 2), { kind: 'customShow', id: 0 });
    n = runStep(r.run, r.pos, 'next', 6, clicks);
    n = runStep(n.run, n.pos, 'next', 6, clicks);
    expect(n.pos).toMatchObject({ at: 2, slide: 6 });
    // An unknown show, a slide outside the order: nothing moves.
    expect(go(run, at(run, 1), { kind: 'customShow', id: 9 }).pos.slide).toBe(1);
    expect(go(run, at(run, 1), { kind: 'slide', slide: 5 }).pos.slide).toBe(1);
  });

  it('does not move for addresses, files, programs, macros and unknown actions', () => {
    const run = newRun([0, 1, 2]);
    const pos = at(run, 1);
    const still: SlideLinkAction[] = [
      { kind: 'url', url: 'https://example.vn' },
      { kind: 'file', path: 'a.docx' },
      { kind: 'presentation', path: 'b.pptx' },
      { kind: 'program', path: 'calc.exe' },
      { kind: 'macro', name: 'M' },
      { kind: 'unknown', action: 'ppaction://x' },
      { kind: 'none' },
    ];
    for (const a of still) {
      const r = go(run, pos, a);
      expect(r.pos).toEqual(pos);
      expect(r.run).toBe(run);
    }
  });
});
