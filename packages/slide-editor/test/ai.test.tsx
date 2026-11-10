// @vitest-environment happy-dom
import { useBlueAi, type AiFeature, type AiRunContext, type BlueAi } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  AiError,
  FakeAiProvider,
  aiComplete,
  type SlideCommand,
  type SlideEditResult,
  type SlideOutlineEdit,
  type SlideOutlineSlide,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { SLIDE_AI_COMMANDS, appendOutlineSlides, slideAiFeatures } from '../src/slideAi';

afterEach(() => {
  cleanup();
});

const i18n = createI18n('vi-VN');
const t = (key: string, options?: Record<string, unknown>) => i18n.t(key, options ?? {});

/** A fake deck: its outline, and `applyOutline` as the engine does it (new slides appended). */
function fakeDeck(initial: { title: string; bullets?: string[] }[]) {
  let outline: SlideOutlineSlide[] = initial.map((s) => ({
    title: s.title,
    bullets: (s.bullets ?? []).map((text) => ({ text, level: 0 })),
    hasTitle: true,
    hasBody: true,
  }));
  const log: SlideCommand[] = [];
  let current = 0;
  let refuse: string | null = null;
  const run = async (c: SlideCommand): Promise<SlideEditResult> => {
    log.push(c);
    const r = {} as SlideEditResult;
    if (c.cmd === 'applyOutline') {
      if (refuse) return { ...r, error: refuse };
      outline = c.slides.map((e: SlideOutlineEdit) => ({
        title: e.title,
        bullets: e.bullets,
        hasTitle: true,
        hasBody: true,
      }));
      current = c.current ?? current;
    }
    if (c.cmd === 'outline' || c.cmd === 'applyOutline') r.outline = outline;
    return r;
  };
  return {
    run,
    log,
    outline: () => outline,
    current: () => current,
    refuse: (why: string) => (refuse = why),
  };
}

const ctx = (provider: FakeAiProvider | undefined, prompt: string): AiRunContext => ({
  prompt,
  tone: 'formal',
  language: 'vi',
  material: null,
  signal: new AbortController().signal,
  ask: (request) => aiComplete(provider, request),
});

const feature = (features: AiFeature[], id: string) => features.find((f) => f.id === id)!;

describe('slides from an outline', () => {
  it('appends slides after the last one as a single applyOutline, existing slides untouched', async () => {
    const deck = fakeDeck([{ title: 'Trang bìa', bullets: ['Công ty A'] }]);
    const n = await appendOutlineSlides(deck.run, [
      {
        title: 'Mục tiêu',
        bullets: [
          { text: 'Doanh thu', level: 0 },
          { text: 'Miền Bắc', level: 1 },
        ],
      },
      { title: 'Kết luận', bullets: [] },
    ]);
    expect(n).toBe(2);
    const applied = deck.log.filter((c) => c.cmd === 'applyOutline');
    expect(applied).toHaveLength(1);
    expect(applied[0]).toEqual({
      cmd: 'applyOutline',
      slides: [
        { slide: 0, title: 'Trang bìa', bullets: [{ text: 'Công ty A', level: 0 }] },
        {
          title: 'Mục tiêu',
          bullets: [
            { text: 'Doanh thu', level: 0 },
            { text: 'Miền Bắc', level: 1 },
          ],
        },
        { title: 'Kết luận', bullets: [] },
      ],
      current: 1,
    });
    expect(deck.outline().map((s) => s.title)).toEqual(['Trang bìa', 'Mục tiêu', 'Kết luận']);
    // The first new slide is shown.
    expect(deck.current()).toBe(1);
  });

  it('does nothing for an empty outline and reports a refusal', async () => {
    const deck = fakeDeck([{ title: 'A' }]);
    expect(await appendOutlineSlides(deck.run, [])).toBe(0);
    expect(deck.log).toEqual([]);
    deck.refuse('readOnly');
    await expect(
      appendOutlineSlides(deck.run, [{ title: 'B', bullets: [] }]),
    ).rejects.toBeInstanceOf(AiError);
  });

  it('rule-based: previews the outline, creates slides only when applied, needs no provider', async () => {
    const deck = fakeDeck([{ title: 'Trang bìa' }]);
    const f = feature(slideAiFeatures({ t, run: deck.run }), 'slidesFromOutline');
    expect(f.ai).toBe(false);
    const preview = await f.run(ctx(undefined, 'Giới thiệu\n  Lịch sử\n  Sản phẩm\nLiên hệ'));
    expect(preview.text).toBe('Giới thiệu\n  - Lịch sử\n  - Sản phẩm\nLiên hệ');
    expect(preview.note).toBe('2 trang chiếu');
    expect(deck.log).toEqual([]);
    expect(preview.actions.map((a) => a.id)).toEqual(['createSlides']);
    expect(await preview.actions[0]!.apply()).toBe(
      'Đã thêm 2 trang chiếu vào cuối bản trình chiếu.',
    );
    expect(deck.outline()).toEqual([
      { title: 'Trang bìa', bullets: [], hasTitle: true, hasBody: true },
      {
        title: 'Giới thiệu',
        bullets: [
          { text: 'Lịch sử', level: 0 },
          { text: 'Sản phẩm', level: 0 },
        ],
        hasTitle: true,
        hasBody: true,
      },
      { title: 'Liên hệ', bullets: [], hasTitle: true, hasBody: true },
    ]);
    await expect(f.run(ctx(undefined, '   '))).rejects.toMatchObject({ code: 'empty' });
  });

  it('from a topic: the provider outline is schema-checked before it is previewed', async () => {
    const deck = fakeDeck([{ title: 'Trang bìa' }]);
    const fake = new FakeAiProvider();
    const f = feature(slideAiFeatures({ t, run: deck.run }), 'slidesFromTopic');
    expect(f.ai).toBe(true);
    const preview = await f.run(ctx(fake, 'Mở đầu: chào, mục tiêu; Kết luận'));
    expect(fake.calls).toEqual([
      { task: 'outline', instruction: 'Mở đầu: chào, mục tiêu; Kết luận', language: 'vi' },
    ]);
    expect(preview.text).toBe('Mở đầu\n  - chào\n  - mục tiêu\nKết luận');
    expect(deck.log).toEqual([]);
    await preview.actions[0]!.apply();
    expect(deck.outline().map((s) => s.title)).toEqual(['Trang bìa', 'Mở đầu', 'Kết luận']);

    // Answers that are not the schema are dropped: nothing is previewed, nothing created.
    const before = deck.log.length;
    for (const answer of [
      'Xin lỗi, tôi không làm được.',
      '{"slides":[{"title":"a","bullets":[{"cmd":"deleteSlide"}]}]}',
      '{"slides":[{"title":"a"}],"commands":[{"cmd":"deleteSlide","index":0}]}',
    ]) {
      fake.answers.outline = answer;
      await expect(f.run(ctx(fake, 'chủ đề'))).rejects.toMatchObject({ code: 'badAnswer' });
    }
    expect(deck.log.length).toBe(before);
    // Off: the AI feature cannot run.
    fake.off();
    await expect(f.run(ctx(fake, 'chủ đề'))).rejects.toMatchObject({ code: 'unavailable' });
  });

  it('names its commands', () => {
    expect(SLIDE_AI_COMMANDS).toEqual({
      slidesFromOutline: { command: 'ai.slidesFromOutline', ai: false },
      slidesFromTopic: { command: 'ai.slidesFromTopic', ai: true },
    });
  });
});

describe('BlueAI pane in BlueSlide', () => {
  function Harness({
    provider,
    onReady,
  }: {
    provider: FakeAiProvider | undefined;
    onReady: (ai: BlueAi) => void;
  }) {
    const ai = useBlueAi(provider, { writeText: async () => {} });
    useEffect(() => {
      onReady(ai);
    });
    return <>{ai.content}</>;
  }

  it('keeps the rule-based outline working while BlueAI is off', async () => {
    const deck = fakeDeck([{ title: 'Trang bìa' }]);
    const fake = new FakeAiProvider(null);
    let hook: BlueAi | null = null;
    render(
      <I18nextProvider i18n={i18n}>
        <Harness provider={fake} onReady={(h) => (hook = h)} />
      </I18nextProvider>,
    );
    // Asked for the AI feature: the pane falls back to the one that needs no AI.
    hook!.open({ features: slideAiFeatures({ t, run: deck.run }), feature: 'slidesFromTopic' });
    await waitFor(() => expect(screen.getByTestId('ai-off')).toBeTruthy());
    const topic = screen.getByTestId('ai-feature-slidesFromTopic') as HTMLButtonElement;
    expect(topic.disabled).toBe(true);
    expect(topic.title).toContain('BlueAI đang tắt');
    await waitFor(() =>
      expect(screen.getByTestId('ai-feature-slidesFromOutline').getAttribute('aria-checked')).toBe(
        'true',
      ),
    );
    fireEvent.change(screen.getByTestId('ai-prompt'), {
      target: { value: 'Phần một\n- Ý A\n- Ý B\nPhần hai' },
    });
    fireEvent.click(screen.getByTestId('ai-run'));
    await waitFor(() =>
      expect(screen.getByTestId('ai-preview').textContent).toBe(
        'Phần một\n  - Ý A\n  - Ý B\nPhần hai',
      ),
    );
    fireEvent.click(screen.getByTestId('ai-apply-createSlides'));
    await waitFor(() =>
      expect(screen.getByTestId('ai-notice').textContent).toBe(
        'Đã thêm 2 trang chiếu vào cuối bản trình chiếu.',
      ),
    );
    expect(deck.outline().map((s) => s.title)).toEqual(['Trang bìa', 'Phần một', 'Phần hai']);
    expect(fake.calls).toHaveLength(0);
  });
});
