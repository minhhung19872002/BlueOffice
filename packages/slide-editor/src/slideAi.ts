/**
 * BlueAI in BlueSlide (CORE-AI-001): slides from an outline.
 *
 * - **From an outline** (rule-based, no AI, works while BlueAI is off): the typed lines become
 *   slides — a line that is neither indented nor a list item is a title, the others its
 *   bullets (`outlineFromText`).
 * - **From a topic** (needs BlueAI): the provider answers a JSON outline, which is checked
 *   against the schema (`parseAiOutline`) before anything is shown.
 *
 * Either way the outline is previewed first; **Create slides** appends the slides after the
 * last one with the engine's own `applyOutline` command (title + content layout, one undo
 * step). Existing slides are passed through unchanged.
 */
import {
  AiError,
  outlineFromText,
  outlineText,
  parseAiOutline,
  type AiOutlineSlide,
} from '@blueoffice/platform';
import type { AiFeature, AiPreview } from '@blueoffice/editor';
import { editsOf, linesOf, type OutlineLine } from './outlineModel';
import type { SlideRun } from './translate';

type T = (key: string, options?: Record<string, unknown>) => string;

/** Appends `slides` after the deck's last slide (one undo step); returns how many were added. */
export async function appendOutlineSlides(
  run: SlideRun,
  slides: readonly AiOutlineSlide[],
): Promise<number> {
  if (!slides.length) return 0;
  const current = await run({ cmd: 'outline' });
  if (!current?.outline) throw new AiError('badAnswer', 'no outline');
  const existing = linesOf(current.outline);
  const added: OutlineLine[] = slides.flatMap((s) => [
    { kind: 'title' as const, level: 0, text: s.title, slide: null },
    ...s.bullets.map((b): OutlineLine => ({
      kind: 'bullet',
      level: b.level,
      text: b.text,
      slide: null,
    })),
  ]);
  const r = await run({
    cmd: 'applyOutline',
    slides: editsOf([...existing, ...added]),
    // Show the first new slide.
    current: current.outline.length,
  });
  if (!r || r.error) throw new AiError('badAnswer', r?.error ?? 'refused');
  return slides.length;
}

export interface SlideAiDeps {
  t: T;
  run: SlideRun;
}

export function slideAiFeatures({ t, run }: SlideAiDeps): AiFeature[] {
  const preview = (slides: AiOutlineSlide[]): AiPreview => {
    if (!slides.length) throw new AiError('empty');
    return {
      text: outlineText(slides),
      note: t('ai.slide.count', { count: slides.length }),
      actions: [
        {
          id: 'createSlides',
          label: t('ai.actions.createSlides'),
          apply: async () =>
            t('ai.slide.created', { count: await appendOutlineSlides(run, slides) }),
        },
      ],
    };
  };
  return [
    {
      id: 'slidesFromOutline',
      label: t('ai.features.slidesFromOutline'),
      hint: t('ai.hints.slidesFromOutline'),
      ai: false,
      prompt: {
        label: t('ai.prompts.outline'),
        placeholder: t('ai.prompts.outlinePlaceholder'),
        required: true,
        rows: 10,
      },
      run: async ({ prompt }) => preview(outlineFromText(prompt)),
    },
    {
      id: 'slidesFromTopic',
      label: t('ai.features.slidesFromTopic'),
      hint: t('ai.hints.slidesFromTopic'),
      ai: true,
      prompt: {
        label: t('ai.prompts.topic'),
        placeholder: t('ai.prompts.topicPlaceholder'),
        required: true,
        rows: 4,
      },
      run: async ({ prompt, language, ask }) =>
        preview(parseAiOutline(await ask({ task: 'outline', instruction: prompt, language }))),
    },
  ];
}

/** Feature ids → commands of BlueSlide. */
export const SLIDE_AI_COMMANDS = {
  slidesFromOutline: { command: 'ai.slidesFromOutline', ai: false },
  slidesFromTopic: { command: 'ai.slidesFromTopic', ai: true },
} as const;
