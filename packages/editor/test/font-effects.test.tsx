// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { CharAdvanced, EditCommand, EditResult, RubySpec } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  advancedPatch,
  CombineCharsDialog,
  FontEffectsDialog,
  PhoneticGuideDialog,
  PLAIN,
  rubyDefaults,
} from '../src/FontEffectsDialog';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

/** A `run` that answers `state` with `advanced` / `ruby` and records other commands. */
function engine(advanced: Partial<CharAdvanced> = {}, ruby?: RubySpec) {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    if (c.cmd === 'state') {
      return {
        format: { advanced: { ...PLAIN, ...advanced }, sizeHalfPoints: 28, lang: 'vi-VN' },
        ruby,
      } as unknown as EditResult;
    }
    sent.push(c);
    return {} as EditResult;
  });
  return { run, sent };
}

describe('advancedPatch', () => {
  it('holds only the changed fields', () => {
    expect(advancedPatch(PLAIN, PLAIN)).toEqual({});
    const next = {
      ...PLAIN,
      emboss: true,
      spacing: 40,
      glow: { color: 'FF0000', radius: 5, transparency: 40 },
    };
    expect(advancedPatch(PLAIN, next)).toEqual({
      emboss: true,
      spacing: 40,
      glow: { color: 'FF0000', radius: 5, transparency: 40 },
    });
    // Turning an effect off sends null (the engine removes it).
    expect(advancedPatch(next, { ...next, glow: null })).toEqual({ glow: null });
  });
});

describe('FontEffectsDialog', () => {
  it('sends the effects and spacing the user changed', async () => {
    const { run, sent } = engine({ shadow: true });
    wrap(<FontEffectsDialog run={run} onClose={vi.fn()} />);
    const shadow = (await screen.findByTestId('fx-shadow')) as HTMLInputElement;
    expect(shadow.checked).toBe(true);
    fireEvent.click(screen.getByTestId('fx-emboss'));
    fireEvent.click(screen.getByTestId('fx-glow'));
    fireEvent.change(screen.getByTestId('fx-glow-radius'), { target: { value: '8' } });
    fireEvent.click(screen.getByTestId('font-effects-tab-spacing'));
    // 1.5 pt of spacing = 30 twips; raised 3 pt = 6 half-points.
    fireEvent.change(screen.getByTestId('fx-spacing'), { target: { value: '1.5' } });
    fireEvent.change(screen.getByTestId('fx-position'), { target: { value: '3' } });
    fireEvent.change(screen.getByTestId('fx-ligatures'), { target: { value: 'none' } });
    fireEvent.click(screen.getByTestId('font-effects-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'formatText',
      format: {
        kind: 'advanced',
        patch: {
          emboss: true,
          glow: { color: 'FFC000', radius: 8, transparency: 40 },
          spacing: 30,
          position: 6,
          ligatures: 'none',
        },
      },
    });
  });

  it('raised and engraved exclude each other', async () => {
    const { run } = engine({ emboss: true });
    wrap(<FontEffectsDialog run={run} onClose={vi.fn()} />);
    fireEvent.click(await screen.findByTestId('fx-imprint'));
    expect((screen.getByTestId('fx-emboss') as HTMLInputElement).checked).toBe(false);
    expect((screen.getByTestId('fx-imprint') as HTMLInputElement).checked).toBe(true);
  });

  it('sends nothing when nothing changed', async () => {
    const { run, sent } = engine();
    const onClose = vi.fn();
    wrap(<FontEffectsDialog run={run} onClose={onClose} />);
    fireEvent.click(await screen.findByTestId('font-effects-ok'));
    expect(onClose).toHaveBeenCalled();
    expect(sent).toHaveLength(0);
  });
});

describe('PhoneticGuideDialog', () => {
  it('defaults: half size, one base size above', () => {
    expect(rubyDefaults(28, 'ja-JP')).toEqual({
      base: '',
      guide: '',
      align: 'center',
      size: 14,
      raise: 28,
      baseSize: 28,
      lang: 'ja-JP',
    });
  });

  it('inserts a new guide', async () => {
    const { run, sent } = engine();
    wrap(<PhoneticGuideDialog run={run} onClose={vi.fn()} />);
    fireEvent.change(await screen.findByTestId('ruby-base'), { target: { value: 'Huế' } });
    fireEvent.change(screen.getByTestId('ruby-guide'), { target: { value: 'cố đô' } });
    fireEvent.change(screen.getByTestId('ruby-align'), { target: { value: 'left' } });
    expect(screen.getByTestId('ruby-preview').textContent).toContain('cố đô');
    fireEvent.click(screen.getByTestId('ruby-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'insertRuby',
      ruby: {
        base: 'Huế',
        guide: 'cố đô',
        align: 'left',
        size: 14,
        raise: 28,
        baseSize: 28,
        lang: 'vi-VN',
      },
      replace: false,
    });
  });

  it('edits the selected guide in place', async () => {
    const ruby: RubySpec = {
      base: 'Hà Nội',
      guide: 'thủ đô',
      align: 'center',
      size: 12,
      raise: 26,
      baseSize: 26,
      lang: 'vi-VN',
    };
    const { run, sent } = engine({}, ruby);
    wrap(<PhoneticGuideDialog run={run} onClose={vi.fn()} />);
    const guide = (await screen.findByTestId('ruby-guide')) as HTMLInputElement;
    expect(guide.value).toBe('thủ đô');
    fireEvent.change(guide, { target: { value: 'kinh đô' } });
    fireEvent.click(screen.getByTestId('ruby-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'insertRuby',
      ruby: { ...ruby, guide: 'kinh đô' },
      replace: true,
    });
  });
});

describe('CombineCharsDialog', () => {
  it('turns two lines in one on with brackets', async () => {
    const { run, sent } = engine();
    wrap(<CombineCharsDialog run={run} onClose={vi.fn()} />);
    fireEvent.click(await screen.findByTestId('combine-on'));
    fireEvent.change(screen.getByTestId('combine-brackets'), { target: { value: 'square' } });
    fireEvent.click(screen.getByTestId('combine-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'formatText',
      format: { kind: 'advanced', patch: { combine: true, combineBrackets: 'square' } },
    });
  });
});
