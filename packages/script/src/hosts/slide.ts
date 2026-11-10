/**
 * BlueSlide side of BlueScript (CORE-SCRIPT-001): `presentation`, slides and
 * shapes translated into slide commands (`Engine::slide_command_json`), sent
 * through the editor's command path. Coordinates are points, as in the engine.
 */
import type {
  SlideChartData,
  SlideCommand,
  SlideEditResult,
  SlideRect,
  SlideShapeInfo,
  SlideTextFormat,
} from '@blueoffice/platform';
import {
  dispatcher,
  hex,
  int,
  num,
  obj,
  rawCommand,
  ScriptApiError,
  str,
  type ScriptHost,
} from '../host';

export type SlideRun = (cmd: SlideCommand) => Promise<SlideEditResult | null>;

export interface SlideScriptHostOptions {
  run: SlideRun;
  onEnd?: () => void | Promise<void>;
}

/** A shape as scripts see it (top-left box, points). */
export interface ScriptShape {
  id: number;
  name: string;
  kind: SlideShapeInfo['kind'];
  text: string | null;
  rect: { x: number; y: number; w: number; h: number; rotation: number };
}

const shapeOf = (s: SlideShapeInfo): ScriptShape => ({
  id: s.id,
  name: s.name,
  kind: s.kind,
  text: s.text,
  rect: {
    x: s.rect.cx - s.rect.w / 2,
    y: s.rect.cy - s.rect.h / 2,
    w: s.rect.w,
    h: s.rect.h,
    rotation: s.rect.rot,
  },
});

const rectOf = (v: unknown): SlideRect | undefined => {
  if (v === null || v === undefined) return undefined;
  const o = obj(v, 'rect');
  return {
    x: num(o.x, 'rect.x'),
    y: num(o.y, 'rect.y'),
    w: num(o.w, 'rect.w'),
    h: num(o.h, 'rect.h'),
  };
};

export function slideScriptHost({ run, onEnd }: SlideScriptHostOptions): ScriptHost {
  let depth: number | null = null;

  const exec = async (cmd: SlideCommand): Promise<SlideEditResult> => {
    const r = await run(cmd);
    if (!r) throw new ScriptApiError(`${cmd.cmd} failed`);
    if (r.error) throw new ScriptApiError(`${cmd.cmd}: ${r.error}`);
    return r;
  };
  const goTo = async (i: unknown) => {
    const slide = int(i, 'slide');
    const r = await exec({ cmd: 'state' });
    if (slide >= r.info.pageCount) throw new ScriptApiError(`no slide ${slide}`);
    return r.slide === slide ? r : exec({ cmd: 'goTo', slide });
  };
  /** Selects shape `id` of slide `i`; the result describes it. */
  const pick = async (i: unknown, id: unknown) => {
    await goTo(i);
    const shape = int(id, 'shape');
    const r = await exec({ cmd: 'select', ids: [shape] });
    if (!r.selection.some((s) => s.id === shape)) throw new ScriptApiError(`no shape ${shape}`);
    return r;
  };
  const inserted = (r: SlideEditResult) => {
    const s = r.selection[0];
    if (!s) throw new ScriptApiError('nothing was inserted');
    return shapeOf(s);
  };

  const call = dispatcher({
    'slide.info': async () => {
      const r = await exec({ cmd: 'state' });
      return { name: r.info.name, slideCount: r.info.pageCount, current: r.slide };
    },
    'slide.shapes': async (i) => {
      await goTo(i);
      const r = await exec({ cmd: 'selectAll' });
      await exec({ cmd: 'select', ids: [] });
      return r.selection.map(shapeOf);
    },
    'slide.addSlide': async (after, layout) => {
      const r = await exec({
        cmd: 'addSlide',
        ...(after === null || after === undefined ? {} : { after: int(after, 'after') }),
        ...(layout === null || layout === undefined ? {} : { layout: int(layout, 'layout') }),
      });
      return r.slide;
    },
    'slide.deleteSlide': async (i) => {
      await exec({ cmd: 'deleteSlide', index: int(i, 'slide') });
    },
    'slide.duplicateSlide': async (i) =>
      (await exec({ cmd: 'duplicateSlide', index: int(i, 'slide') })).slide,
    'slide.getNotes': async (i) => (await goTo(i)).notes,
    'slide.setNotes': async (i, text) => {
      await exec({ cmd: 'setNotes', slide: int(i, 'slide'), text: str(text, 'text') });
    },
    'slide.addTextBox': async (i, text, rect) => {
      await goTo(i);
      const r = rectOf(rect);
      return inserted(
        await exec({ cmd: 'insertTextBox', text: str(text, 'text'), ...(r ? { rect: r } : {}) }),
      );
    },
    'slide.addShape': async (i, preset, rect, text) => {
      await goTo(i);
      const r = rectOf(rect);
      const shape = inserted(
        await exec({
          cmd: 'insertShape',
          preset: str(preset, 'preset'),
          ...(r ? { rect: r } : {}),
        }),
      );
      if (text === null || text === undefined) return shape;
      const t = await exec({ cmd: 'setText', id: shape.id, text: str(text, 'text') });
      await exec({ cmd: 'textExit' });
      return { ...shape, text: t.selection.find((s) => s.id === shape.id)?.text ?? String(text) };
    },
    'slide.addTable': async (i, rows, cols, rect) => {
      await goTo(i);
      const r = rectOf(rect);
      return inserted(
        await exec({
          cmd: 'insertTable',
          rows: int(rows, 'rows', 1),
          cols: int(cols, 'cols', 1),
          ...(r ? { rect: r } : {}),
        }),
      );
    },
    'slide.addChart': async (i, kind, data, options) => {
      await goTo(i);
      const o = options === null || options === undefined ? {} : obj(options, 'options');
      const d = data === null || data === undefined ? undefined : chartData(data);
      const r = rectOf(o.rect);
      return inserted(
        await exec({
          cmd: 'insertChart',
          kind: str(kind, 'kind') as Extract<SlideCommand, { cmd: 'insertChart' }>['kind'],
          ...(o.title === undefined ? {} : { title: str(o.title, 'title') }),
          ...(d ? { data: d } : {}),
          ...(r ? { rect: r } : {}),
        }),
      );
    },
    'slide.setText': async (i, id, text) => {
      await pick(i, id);
      await exec({ cmd: 'setText', id: int(id, 'shape'), text: str(text, 'text') });
      await exec({ cmd: 'textExit' });
    },
    'slide.formatText': async (i, id, f) => {
      await pick(i, id);
      const o = obj(f, 'format');
      const apply = (format: SlideTextFormat) => exec({ cmd: 'formatText', format });
      for (const k of ['bold', 'italic', 'underline', 'strike'] as const) {
        if (o[k] !== undefined) await apply({ kind: k, value: !!o[k] });
      }
      if (o.size !== undefined) await apply({ kind: 'size', points: num(o.size, 'size') });
      if (o.color !== undefined) await apply({ kind: 'color', hex: hex(o.color, 'color') });
      if (o.font !== undefined) await apply({ kind: 'font', typeface: str(o.font, 'font') });
    },
    'slide.setFill': async (i, id, color) => {
      await pick(i, id);
      await exec({ cmd: 'setFill', color: hex(color, 'color') });
    },
    'slide.setLine': async (i, id, color, width) => {
      await pick(i, id);
      await exec({
        cmd: 'setLine',
        color: hex(color, 'color'),
        ...(width === null || width === undefined ? {} : { width: num(width, 'width') }),
      });
    },
    'slide.move': async (i, id, x, y) => {
      const r = await pick(i, id);
      const s = r.selection.find((sh) => sh.id === id)!;
      const at = shapeOf(s).rect;
      await exec({
        cmd: 'transform',
        transform: { kind: 'move', dx: num(x, 'x') - at.x, dy: num(y, 'y') - at.y },
      });
    },
    'slide.deleteShape': async (i, id) => {
      await pick(i, id);
      await exec({ cmd: 'delete' });
    },
    'slide.command': async (cmd) => {
      const r = await exec(rawCommand<SlideCommand>(cmd));
      const out: Partial<SlideEditResult> = { ...r };
      delete out.collab;
      return out;
    },
  });

  return {
    app: 'slide',
    call,
    async begin() {
      depth = (await run({ cmd: 'historyLen' }))?.history ?? null;
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

function chartData(v: unknown): SlideChartData {
  const o = obj(v, 'data');
  if (!Array.isArray(o.categories) || !Array.isArray(o.series)) {
    throw new ScriptApiError('data: { categories: string[], series: { name, values }[] }');
  }
  return {
    categories: o.categories.map((c) => String(c)),
    series: o.series.map((s: unknown) => {
      const so = obj(s, 'series');
      if (!Array.isArray(so.values)) throw new ScriptApiError('series.values: expected numbers');
      return {
        name: String(so.name ?? ''),
        values: so.values.map((x: unknown) => (typeof x === 'number' ? x : null)),
      };
    }),
  };
}
