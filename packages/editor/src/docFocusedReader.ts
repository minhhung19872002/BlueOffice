/**
 * BlueDoc text for the focused reader (CORE-LANG-004): the body paragraphs the read-aloud
 * feature speaks (`speechText`: index text, language, hidden / deleted ranges) with the heading
 * levels of the outline (`outline`). Hidden and deleted text and object placeholders become
 * spaces, so a word's offset in a block is its offset in the read-aloud unit.
 */
import type { EngineHost } from '@blueoffice/platform';
import { docUnits } from './docReadAloud';
import type { ReaderBlock } from './FocusedReader';
import { speakable } from './readAloud';

export async function docReaderBlocks(
  engine: EngineHost,
  fallbackLang: string,
): Promise<ReaderBlock[]> {
  const [speech, outline] = await Promise.all([
    engine.command({ cmd: 'speechText' }),
    engine.command({ cmd: 'outline' }),
  ]);
  const levels = new Map((outline.outline ?? []).map((o) => [o.para, o.level]));
  const { units, paras } = docUnits(speech.speech?.paras ?? [], fallbackLang);
  const blocks: ReaderBlock[] = [];
  units.forEach((u, i) => {
    const para = paras[i];
    const text = speakable(u);
    if (!para || !text.trim()) return;
    blocks.push({ p: para.p, text, level: levels.get(para.p) ?? null, lang: u.lang });
  });
  return blocks;
}

/** Block to open the reader at: the paragraph of the caret, else the next one with text. */
export function readerStart(blocks: readonly ReaderBlock[], caretPara: number | undefined): number {
  if (caretPara === undefined) return 0;
  const i = blocks.findIndex((b) => b.p >= caretPara);
  return i < 0 ? Math.max(0, blocks.length - 1) : i;
}
