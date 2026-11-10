/**
 * BlueDoc side of BlueScript (CORE-SCRIPT-001): the `doc` API translated into
 * engine commands (`bo_engine::edit::Command` JSON, ADR-0008) sent through the
 * editor's own command path, so the view, review state and collaboration follow
 * as for any UI edit. Script offsets are UTF-16 indexes into the paragraph's
 * index text (objects are U+FFFC); the engine's are UTF-8 bytes.
 */
import type {
  Alignment,
  CharFormat,
  EditCommand,
  EditResult,
  FindOptions,
  FormatState,
  ParaFormat,
} from '@blueoffice/platform';
import {
  byteOffset,
  dispatcher,
  hex,
  int,
  num,
  obj,
  rawCommand,
  ScriptApiError,
  str,
  utf16Offset,
  type ScriptHost,
} from '../host';

export type DocRun = (cmd: EditCommand) => Promise<EditResult | null>;

export interface DocScriptHostOptions {
  /** Runs an editing command (the editor's `runCommand`, or the engine directly). */
  run: DocRun;
  /** After the run (the editor redraws, re-reads state). */
  onEnd?: () => void | Promise<void>;
}

const ALIGN: Record<string, Alignment> = {
  left: 'Left',
  center: 'Center',
  right: 'Right',
  justify: 'Justify',
};

const TOGGLES = ['bold', 'italic', 'underline', 'strike', 'superscript', 'subscript'] as const;

export function docScriptHost({ run, onEnd }: DocScriptHostOptions): ScriptHost {
  let depth: number | null = null;

  const exec = async (cmd: EditCommand): Promise<EditResult> => {
    const r = await run(cmd);
    if (!r) throw new ScriptApiError(`${cmd.cmd} failed`);
    if (r.error) throw new ScriptApiError(`${cmd.cmd}: ${r.error}`);
    return r;
  };
  const paras = async (from?: number, to?: number) => {
    const r = await exec(
      from === undefined ? { cmd: 'speechText' } : { cmd: 'speechText', from, to: to ?? from },
    );
    return (r.speech?.paras ?? []).map((p) => ({ index: p.p, text: p.text }));
  };
  const paraText = async (i: unknown): Promise<string> => {
    const index = int(i, 'paragraph');
    const p = (await paras(index))[0];
    if (!p || p.index !== index) throw new ScriptApiError(`no paragraph ${index}`);
    return p.text;
  };
  const caret = (p: number, o: number): EditCommand => ({
    cmd: 'setSelection',
    selection: { anchor: { p, o }, focus: { p, o } },
  });
  const selectWhole = async (i: number) => {
    const t = await paraText(i);
    return exec({
      cmd: 'setSelection',
      selection: { anchor: { p: i, o: 0 }, focus: { p: i, o: byteOffset(t, t.length) } },
    });
  };
  /** Character formatting of the selection; toggles only what differs. */
  const charFormat = async (f: unknown, state: FormatState) => {
    const o = obj(f, 'format');
    let fs = state;
    const apply = async (format: CharFormat) => {
      fs = (await exec({ cmd: 'formatText', format })).format;
    };
    for (const k of TOGGLES) {
      if (o[k] === undefined) continue;
      if (typeof o[k] !== 'boolean') throw new ScriptApiError(`format.${k}: expected a boolean`);
      if (fs[k] !== o[k]) await apply({ kind: k });
    }
    if (o.font !== undefined) await apply({ kind: 'font', name: str(o.font, 'format.font') });
    if (o.size !== undefined) {
      await apply({ kind: 'size', halfPoints: Math.round(num(o.size, 'format.size') * 2) });
    }
    if (o.color !== undefined) await apply({ kind: 'color', hex: hex(o.color, 'format.color') });
    if (o.highlight !== undefined) {
      await apply({ kind: 'highlight', hex: hex(o.highlight, 'format.highlight') });
    }
    return fs;
  };
  const paraFormat = async (f: unknown, state: FormatState) => {
    const o = obj(f, 'paragraph format');
    const apply = (format: ParaFormat) => exec({ cmd: 'formatParagraph', format });
    const twips = (k: string) => Math.round(num(o[k], `paragraph format.${k}`) * 20);
    if (o.align !== undefined) {
      const a = ALIGN[str(o.align, 'align')];
      if (!a) throw new ScriptApiError('align: left, center, right or justify');
      await apply({ kind: 'align', align: a });
    }
    for (const k of [
      'spaceBefore',
      'spaceAfter',
      'indentLeft',
      'indentRight',
      'indentFirstLine',
    ] as const) {
      if (o[k] !== undefined) await apply({ kind: k, twips: twips(k) });
    }
    if (o.style !== undefined) await exec({ cmd: 'applyStyle', id: str(o.style, 'style') });
    if (o.bullets !== undefined && !!o.bullets !== state.bullets) await apply({ kind: 'bullets' });
    if (o.numbering !== undefined && !!o.numbering !== state.numbering) {
      await apply({ kind: 'numbering' });
    }
  };
  const insertAfter = async (i: number, text: unknown, style: unknown) => {
    const t = await paraText(i);
    await exec(caret(i, byteOffset(t, t.length)));
    await exec({ cmd: 'splitParagraph' });
    const s = str(text, 'text');
    if (s) await exec({ cmd: 'insertText', text: s });
    if (style !== null && style !== undefined) {
      await exec({ cmd: 'applyStyle', id: str(style, 'style') });
    }
    return i + 1;
  };
  const position = async (p: { p: number; o: number }) => ({
    paragraph: p.p,
    offset: utf16Offset(await paraText(p.p), p.o),
  });

  const call = dispatcher({
    'doc.info': async () => {
      const r = await exec({ cmd: 'state' });
      return {
        name: r.info.name,
        pageCount: r.info.pageCount,
        wordCount: r.info.wordCount,
        modified: r.modified,
      };
    },
    'doc.text': async () => (await paras()).map((p) => p.text).join('\n'),
    'doc.paragraphs': () => paras(),
    'doc.paragraphStyle': async (i) => {
      const t = await paraText(i);
      return (await exec(caret(int(i, 'paragraph'), byteOffset(t, 0)))).format.style;
    },
    'doc.setParagraphText': async (i, text) => {
      const r = await selectWhole(int(i, 'paragraph'));
      const s = str(text, 'text');
      const { anchor, focus } = r.selection;
      if (s) await exec({ cmd: 'insertText', text: s });
      else if (anchor.o !== focus.o) await exec({ cmd: 'deleteBackward' });
    },
    'doc.setParagraphStyle': async (i, id) => {
      await paraText(i);
      await exec(caret(int(i, 'paragraph'), 0));
      await exec({ cmd: 'applyStyle', id: str(id, 'style') });
    },
    'doc.formatParagraph': async (i, f) => {
      const r = await selectWhole(int(i, 'paragraph'));
      await charFormat(f, r.format);
    },
    'doc.setParagraphFormat': async (i, f) => {
      await paraText(i);
      const r = await exec(caret(int(i, 'paragraph'), 0));
      await paraFormat(f, r.format);
    },
    'doc.insertParagraphAfter': (i, text, style) => insertAfter(int(i, 'paragraph'), text, style),
    'doc.deleteParagraph': async (i) => {
      const index = int(i, 'paragraph');
      const t = await paraText(index);
      const next = (await paras(index + 1))[0];
      if (next && next.index === index + 1) {
        await exec({
          cmd: 'setSelection',
          selection: { anchor: { p: index, o: 0 }, focus: { p: index + 1, o: 0 } },
        });
      } else if (index > 0) {
        const prev = await paraText(index - 1);
        await exec({
          cmd: 'setSelection',
          selection: {
            anchor: { p: index - 1, o: byteOffset(prev, prev.length) },
            focus: { p: index, o: byteOffset(t, t.length) },
          },
        });
      } else {
        await selectWhole(index);
      }
      const r = await exec({ cmd: 'state' });
      const { anchor, focus } = r.selection;
      if (anchor.p !== focus.p || anchor.o !== focus.o) await exec({ cmd: 'deleteBackward' });
    },
    'doc.appendParagraph': async (text, style) => {
      const r = await exec({ cmd: 'move', motion: 'docEnd' });
      const at = r.selection.focus;
      if (at.o === 0 && (await paraText(at.p)) === '') {
        const s = str(text, 'text');
        if (s) await exec({ cmd: 'insertText', text: s });
        if (style !== null && style !== undefined) {
          await exec({ cmd: 'applyStyle', id: str(style, 'style') });
        }
        return at.p;
      }
      return insertAfter(at.p, text, style);
    },
    'doc.insertText': async (text) => {
      await exec({ cmd: 'insertText', text: str(text, 'text') });
    },
    'doc.select': async (i, start, end) => {
      const index = int(i, 'paragraph');
      const t = await paraText(index);
      const a = int(start ?? 0, 'start');
      const b = end === null || end === undefined ? a : int(end, 'end');
      await exec({
        cmd: 'setSelection',
        selection: {
          anchor: { p: index, o: byteOffset(t, a) },
          focus: { p: index, o: byteOffset(t, b) },
        },
      });
    },
    'doc.selectAll': async () => {
      await exec({ cmd: 'selectAll' });
    },
    'doc.selection': async () => {
      const r = await exec({ cmd: 'state' });
      return {
        anchor: await position(r.selection.anchor),
        focus: await position(r.selection.focus),
      };
    },
    'doc.format': async (f) => {
      const r = await exec({ cmd: 'state' });
      await charFormat(f, r.format);
    },
    'doc.find': async (query, options) => {
      const r = await exec({
        cmd: 'find',
        query: str(query, 'query'),
        options: findOptions(options),
      });
      const texts = new Map<number, string>();
      const textOf = async (p: number) => {
        if (!texts.has(p)) texts.set(p, await paraText(p));
        return texts.get(p)!;
      };
      const out = [];
      for (const m of r.matches ?? []) {
        const ep = m.ep ?? m.p;
        out.push({
          paragraph: m.p,
          start: utf16Offset(await textOf(m.p), m.start),
          endParagraph: ep,
          end: utf16Offset(await textOf(ep), m.end),
        });
      }
      return out;
    },
    'doc.replace': async (query, w, options) => {
      const r = await exec({
        cmd: 'replaceAll',
        query: str(query, 'query'),
        with: str(w, 'replacement'),
        options: findOptions(options),
      });
      return r.count ?? 0;
    },
    'doc.insertTable': async (rows, cols, values) => {
      const nr = int(rows, 'rows', 1);
      const nc = int(cols, 'cols', 1);
      if (nr * nc > 10_000) throw new ScriptApiError('insertTable: at most 10000 cells');
      await exec({ cmd: 'insertTable', rows: nr, cols: nc });
      if (values === null || values === undefined) return;
      if (!Array.isArray(values)) throw new ScriptApiError('insertTable: values must be rows');
      // The caret is in the first cell; Tab walks the cells row by row.
      for (let r = 0; r < nr; r++) {
        const row: unknown = values[r];
        for (let c = 0; c < nc; c++) {
          const v: unknown = Array.isArray(row) ? row[c] : undefined;
          if (v !== null && v !== undefined && v !== '') {
            await exec({ cmd: 'insertText', text: String(v) });
          }
          if (r < nr - 1 || c < nc - 1) await exec({ cmd: 'tab' });
        }
      }
    },
    'doc.insertPageBreak': async () => {
      await exec({ cmd: 'insertPageBreak' });
    },
    'doc.command': async (cmd) => {
      const r = await exec(rawCommand<EditCommand>(cmd));
      const out: Partial<EditResult> = { ...r };
      delete out.collab;
      delete out.clip;
      return out;
    },
  });

  return {
    app: 'doc',
    call,
    async begin() {
      depth = (await run({ cmd: 'historyLen' }))?.count ?? null;
    },
    async end() {
      try {
        if (depth !== null) await run({ cmd: 'mergeHistory', since: depth });
      } finally {
        depth = null;
        await onEnd?.();
      }
    },
  };
}

function findOptions(v: unknown): FindOptions {
  if (v === null || v === undefined) return {};
  const o = obj(v, 'options');
  const out: FindOptions = {};
  for (const k of ['matchCase', 'wholeWord', 'regex', 'wildcards'] as const) {
    if (o[k] !== undefined) out[k] = !!o[k];
  }
  return out;
}
