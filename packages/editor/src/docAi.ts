/**
 * BlueAI in BlueDoc (CORE-AI-001): rewrite the selection in a tone, summarise the selection or
 * the whole document, draft from a request.
 *
 * The material is read from the engine (`docSelectionText`; the whole body through
 * `translationSegments`). A result is only a preview: **Replace** types it over the selection
 * and **Insert** types it after the selection — both a single `insertText` command, hence one
 * undo step, recorded as a tracked change while track changes is on.
 */
import type { EditCommand, EditResult } from '@blueoffice/platform';
import { AiNotice, type AiFeature, type AiMaterial, type AiPreview } from './AiPane';
import { docSelectionText } from './docTranslate';

type Run = (cmd: EditCommand) => Promise<EditResult | null | undefined>;
type T = (key: string, options?: Record<string, unknown>) => string;

const before = (a: { p: number; o: number }, b: { p: number; o: number }) =>
  a.p < b.p || (a.p === b.p && a.o < b.o);

/** Text of the whole body: one line per paragraph (objects, fields and tabs left out). */
export async function docBodyText(run: Run): Promise<string> {
  const r = await run({ cmd: 'translationSegments' });
  const paras = new Map<number, string[]>();
  for (const s of r?.translation ?? []) {
    const p = paras.get(s.p) ?? [];
    p.push(s.text);
    paras.set(s.p, p);
  }
  return [...paras.values()]
    .map((p) => p.join(' ').trim())
    .filter(Boolean)
    .join('\n');
}

/**
 * Types `text` after the selection as one edit: a new paragraph after a selection (the result
 * of rewriting / summarising it), at the caret otherwise.
 */
export async function docInsertResult(run: Run, text: string): Promise<void> {
  const state = await run({ cmd: 'state' });
  if (!state) return;
  const { anchor, focus } = state.selection;
  const collapsed = anchor.p === focus.p && anchor.o === focus.o;
  const end = before(anchor, focus) ? focus : anchor;
  if (!collapsed) await run({ cmd: 'setSelection', selection: { anchor: end, focus: end } });
  await run({ cmd: 'insertText', text: collapsed ? text : `\n${text}` });
}

export interface DocAiDeps {
  t: T;
  run: Run;
  /** Returns the focus to the document after a result was applied. */
  focus?: () => void;
}

export function docAiFeatures({ t, run, focus }: DocAiDeps): AiFeature[] {
  const selection = async (): Promise<AiMaterial | null> => {
    const text = await docSelectionText(run);
    return text.trim() ? { text, label: t('ai.material.selection'), from: 'selection' } : null;
  };
  const selectionOrDocument = async (): Promise<AiMaterial | null> => {
    const sel = await selection();
    if (sel) return sel;
    const text = await docBodyText(run);
    return text.trim() ? { text, label: t('ai.material.document'), from: 'document' } : null;
  };
  /** Insert always; Replace when the result stands for the selection. */
  const preview = (text: string, material: AiMaterial | null): AiPreview => ({
    text,
    actions: [
      ...(material?.from === 'selection'
        ? [
            {
              id: 'replace',
              label: t('ai.actions.replace'),
              apply: async () => {
                // The selection must still be there: never type over nothing by surprise.
                if (!(await docSelectionText(run)).trim())
                  throw new AiNotice(t('ai.doc.selectionLost'));
                await run({ cmd: 'insertText', text });
                focus?.();
              },
            },
          ]
        : []),
      {
        id: 'insert',
        label: t('ai.actions.insert'),
        apply: async () => {
          await docInsertResult(run, text);
          focus?.();
        },
      },
    ],
  });
  return [
    {
      id: 'rewrite',
      label: t('ai.features.rewrite'),
      hint: t('ai.hints.rewrite'),
      ai: true,
      tones: true,
      material: { read: selection, required: true, empty: t('ai.doc.selectFirst') },
      run: async ({ material, tone, language, ask }) =>
        preview(
          await ask({ task: 'rewrite', text: material?.text ?? '', tone, language }),
          material,
        ),
    },
    {
      id: 'summarize',
      label: t('ai.features.summarize'),
      hint: t('ai.hints.summarize'),
      ai: true,
      material: { read: selectionOrDocument, required: true, empty: t('ai.doc.empty') },
      run: async ({ material, language, ask }) => {
        const text = await ask({ task: 'summarize', text: material?.text ?? '', language });
        // A summary never replaces what it summarises.
        return preview(text, material ? { ...material, from: 'summary' } : null);
      },
    },
    {
      id: 'draft',
      label: t('ai.features.draft'),
      hint: t('ai.hints.draft'),
      ai: true,
      prompt: {
        label: t('ai.prompts.draft'),
        placeholder: t('ai.prompts.draftPlaceholder'),
        required: true,
        rows: 4,
      },
      material: { read: selection, required: false, empty: t('ai.doc.noContext') },
      run: async ({ prompt, material, language, ask }) =>
        preview(
          await ask({ task: 'draft', instruction: prompt, text: material?.text ?? '', language }),
          material,
        ),
    },
  ];
}

/** Feature ids → commands of BlueDoc. */
export const DOC_AI_COMMANDS = {
  rewrite: { command: 'ai.rewrite', ai: true },
  summarize: { command: 'ai.summarize', ai: true },
  draft: { command: 'ai.draft', ai: true },
} as const;
