/**
 * BlueScript prelude (CORE-SCRIPT-001): JavaScript evaluated inside QuickJS before
 * the user script. It builds the object API typed by `bluescript.d.ts` on top of
 * one native function, `__bo_call(method, argsJson)`, and removes the native
 * hooks from the global object afterwards.
 *
 * Every API method is exactly one host call `method(...args)` with JSON arguments
 * (see `API_METHODS`), so a recorder (CORE-SCRIPT-002) can turn the calls it
 * observes back into script source with {@link callToSource}.
 */

/** Host method names, by application. Keep in step with the hosts in `hosts/`. */
export const API_METHODS = {
  doc: [
    'doc.info',
    'doc.text',
    'doc.paragraphs',
    'doc.paragraphStyle',
    'doc.setParagraphText',
    'doc.setParagraphStyle',
    'doc.formatParagraph',
    'doc.setParagraphFormat',
    'doc.insertParagraphAfter',
    'doc.deleteParagraph',
    'doc.appendParagraph',
    'doc.insertText',
    'doc.select',
    'doc.selectAll',
    'doc.selection',
    'doc.format',
    'doc.find',
    'doc.replace',
    'doc.insertTable',
    'doc.insertPageBreak',
    'doc.command',
  ],
  sheet: [
    'sheet.list',
    'sheet.active',
    'sheet.add',
    'sheet.rename',
    'sheet.getValues',
    'sheet.setValues',
    'sheet.getFormulas',
    'sheet.setFormulas',
    'sheet.fill',
    'sheet.format',
    'sheet.clear',
    'sheet.merge',
    'sheet.borders',
    'sheet.addChart',
    'sheet.charts',
    'sheet.command',
  ],
  slide: [
    'slide.info',
    'slide.shapes',
    'slide.addSlide',
    'slide.deleteSlide',
    'slide.duplicateSlide',
    'slide.getNotes',
    'slide.setNotes',
    'slide.addTextBox',
    'slide.addShape',
    'slide.addTable',
    'slide.addChart',
    'slide.setText',
    'slide.formatText',
    'slide.setFill',
    'slide.setLine',
    'slide.move',
    'slide.deleteShape',
    'slide.command',
  ],
} as const;

export type ApiMethod = (typeof API_METHODS)[keyof typeof API_METHODS][number];

/** Source of the prelude (plain ES2020, no imports). */
export const PRELUDE = String.raw`
(function () {
  'use strict';
  const g = globalThis;
  const native = g.__bo_call;
  const nativeLog = g.__bo_log;
  const app = g.__bo_app;
  delete g.__bo_call;
  delete g.__bo_log;
  delete g.__bo_app;
  const rpc = async (method, ...args) => {
    const json = await native(method, JSON.stringify(args));
    return JSON.parse(json);
  };
  const show = (v) => {
    if (typeof v === 'string') return v;
    if (v === undefined) return 'undefined';
    if (v instanceof Error) return v.name + ': ' + v.message;
    try { return JSON.stringify(v); } catch (e) { return String(v); }
  };
  const logger = (level) => (...a) => nativeLog(level, a.map(show).join(' '));
  const define = (name, value) =>
    Object.defineProperty(g, name, { value, enumerable: false, writable: false, configurable: false });
  define('console', Object.freeze({
    log: logger('log'), info: logger('info'), warn: logger('warn'), error: logger('error'),
  }));

  // ------------------------------------------------------------- BlueDoc
  if (app === 'doc') {
    class Paragraph {
      constructor(d) { this.index = d.index; this.text = d.text; Object.freeze(this); }
      getStyle() { return rpc('doc.paragraphStyle', this.index); }
      setText(text) { return rpc('doc.setParagraphText', this.index, String(text)); }
      setStyle(id) { return rpc('doc.setParagraphStyle', this.index, String(id)); }
      format(f) { return rpc('doc.formatParagraph', this.index, f); }
      setFormat(f) { return rpc('doc.setParagraphFormat', this.index, f); }
      select(start, end) { return rpc('doc.select', this.index, start ?? 0, end ?? null); }
      insertAfter(text, style) { return rpc('doc.insertParagraphAfter', this.index, String(text), style ?? null); }
      delete() { return rpc('doc.deleteParagraph', this.index); }
    }
    const paras = (list) => list.map((d) => new Paragraph(d));
    define('doc', Object.freeze({
      get info() { return rpc('doc.info'); },
      get text() { return rpc('doc.text'); },
      get paragraphs() { return rpc('doc.paragraphs').then(paras); },
      async paragraph(i) {
        const list = await rpc('doc.paragraphs');
        const d = list.find((p) => p.index === i);
        if (!d) throw new RangeError('no paragraph ' + i);
        return new Paragraph(d);
      },
      get selection() { return rpc('doc.selection'); },
      appendParagraph(text, style) { return rpc('doc.appendParagraph', String(text), style ?? null); },
      insertText(text) { return rpc('doc.insertText', String(text)); },
      select(paragraph, start, end) { return rpc('doc.select', paragraph, start ?? 0, end ?? null); },
      selectAll() { return rpc('doc.selectAll'); },
      format(f) { return rpc('doc.format', f); },
      find(query, options) { return rpc('doc.find', String(query), options ?? {}); },
      replace(query, replacement, options) {
        return rpc('doc.replace', String(query), String(replacement), options ?? {});
      },
      insertTable(rows, cols, values) { return rpc('doc.insertTable', rows, cols, values ?? null); },
      insertPageBreak() { return rpc('doc.insertPageBreak'); },
      command(cmd) { return rpc('doc.command', cmd); },
    }));
  }

  // ------------------------------------------------------------- BlueSheet
  if (app === 'sheet') {
    class Range {
      constructor(sheet, address) { this.sheet = sheet; this.address = String(address); Object.freeze(this); }
      get values() { return rpc('sheet.getValues', this.sheet, this.address); }
      get formulas() { return rpc('sheet.getFormulas', this.sheet, this.address); }
      setValues(v) { return rpc('sheet.setValues', this.sheet, this.address, v); }
      setFormulas(f) { return rpc('sheet.setFormulas', this.sheet, this.address, f); }
      setValue(v) { return rpc('sheet.fill', this.sheet, this.address, v); }
      format(patch) { return rpc('sheet.format', this.sheet, this.address, patch); }
      clear(what) { return rpc('sheet.clear', this.sheet, this.address, what ?? 'all'); }
      merge(center) { return rpc('sheet.merge', this.sheet, this.address, !!center); }
      borders(preset, style, color) {
        return rpc('sheet.borders', this.sheet, this.address, preset, style ?? null, color ?? null);
      }
    }
    class Sheet {
      constructor(d) { this.index = d.index; this.name = d.name; Object.freeze(this); }
      range(address) { return new Range(this.index, address); }
      cell(row, col) {
        let c = col + 1, name = '';
        while (c > 0) { const m = (c - 1) % 26; name = String.fromCharCode(65 + m) + name; c = Math.floor((c - 1) / 26); }
        return new Range(this.index, name + (row + 1));
      }
      rename(name) { return rpc('sheet.rename', this.index, String(name)); }
      addChart(kind, address, options) {
        return rpc('sheet.addChart', this.index, String(address), String(kind), options ?? {});
      }
      get charts() { return rpc('sheet.charts', this.index); }
    }
    define('workbook', Object.freeze({
      get sheets() { return rpc('sheet.list').then((l) => l.map((d) => new Sheet(d))); },
      get activeSheet() { return rpc('sheet.active').then((d) => new Sheet(d)); },
      async sheet(which) {
        const l = await rpc('sheet.list');
        const d = typeof which === 'number' ? l[which] : l.find((s) => s.name === which);
        if (!d) throw new RangeError('no sheet ' + String(which));
        return new Sheet(d);
      },
      async addSheet(name) { return new Sheet(await rpc('sheet.add', name ?? null)); },
      command(cmd) { return rpc('sheet.command', cmd); },
    }));
    // The active sheet when the script started (index -1: resolved by the host).
    define('sheet', new Sheet({ index: -1, name: '' }));
  }

  // ------------------------------------------------------------- BlueSlide
  if (app === 'slide') {
    class Shape {
      constructor(slide, d) {
        this.slide = slide; this.id = d.id; this.name = d.name; this.kind = d.kind;
        this.text = d.text; this.rect = d.rect; Object.freeze(this);
      }
      setText(text) { return rpc('slide.setText', this.slide, this.id, String(text)); }
      format(f) { return rpc('slide.formatText', this.slide, this.id, f); }
      setFill(color) { return rpc('slide.setFill', this.slide, this.id, color ?? null); }
      setLine(color, width) { return rpc('slide.setLine', this.slide, this.id, color ?? null, width ?? null); }
      move(x, y) { return rpc('slide.move', this.slide, this.id, x, y); }
      delete() { return rpc('slide.deleteShape', this.slide, this.id); }
    }
    const shape = (slide) => (d) => new Shape(slide, d);
    class Slide {
      constructor(index) { this.index = index; Object.freeze(this); }
      get shapes() { return rpc('slide.shapes', this.index).then((l) => l.map(shape(this.index))); }
      get notes() { return rpc('slide.getNotes', this.index); }
      setNotes(text) { return rpc('slide.setNotes', this.index, String(text)); }
      async addTextBox(text, rect) {
        return shape(this.index)(await rpc('slide.addTextBox', this.index, String(text), rect ?? null));
      }
      async addShape(preset, rect, text) {
        return shape(this.index)(await rpc('slide.addShape', this.index, String(preset), rect ?? null, text ?? null));
      }
      async addTable(rows, cols, rect) {
        return shape(this.index)(await rpc('slide.addTable', this.index, rows, cols, rect ?? null));
      }
      async addChart(kind, data, options) {
        return shape(this.index)(await rpc('slide.addChart', this.index, String(kind), data, options ?? {}));
      }
      delete() { return rpc('slide.deleteSlide', this.index); }
      async duplicate() { return new Slide(await rpc('slide.duplicateSlide', this.index)); }
    }
    define('presentation', Object.freeze({
      get info() { return rpc('slide.info'); },
      get slides() {
        return rpc('slide.info').then((i) => Array.from({ length: i.slideCount }, (_, k) => new Slide(k)));
      },
      slide(i) { return new Slide(i); },
      async addSlide(after, layout) { return new Slide(await rpc('slide.addSlide', after ?? null, layout ?? null)); },
      command(cmd) { return rpc('slide.command', cmd); },
    }));
  }
})();
`;

/** One observed API call. */
export interface ApiCall {
  method: string;
  args: unknown[];
}

/**
 * Script source for a list of host calls (the recorder of CORE-SCRIPT-002 emits
 * these): each becomes `await __call("method", ...args)` through the public API
 * where a direct mapping exists, else the app's `command()` escape hatch.
 */
export function callToSource(call: ApiCall): string {
  const a = call.args.map((x) => JSON.stringify(x ?? null));
  const [ns, name] = call.method.split('.') as [string, string];
  switch (call.method) {
    case 'doc.insertText':
      return `await doc.insertText(${a[0]});`;
    case 'doc.format':
      return `await doc.format(${a[0]});`;
    case 'doc.replace':
      return `await doc.replace(${a.slice(0, 3).join(', ')});`;
    case 'doc.appendParagraph':
      return `await doc.appendParagraph(${a.slice(0, 2).join(', ')});`;
    case 'doc.select':
      return `await doc.select(${a.slice(0, 3).join(', ')});`;
    case 'doc.insertTable':
      return `await doc.insertTable(${a.slice(0, 3).join(', ')});`;
    case 'doc.insertPageBreak':
      return 'await doc.insertPageBreak();';
    case 'doc.command':
      return `await doc.command(${a[0]});`;
    case 'sheet.setValues':
    case 'sheet.setFormulas':
    case 'sheet.format': {
      const fn = { setValues: 'setValues', setFormulas: 'setFormulas', format: 'format' }[
        name as 'setValues'
      ];
      return `await (await workbook.sheet(${a[0]})).range(${a[1]}).${fn}(${a[2]});`;
    }
    case 'sheet.command':
      return `await workbook.command(${a[0]});`;
    case 'slide.setText':
      return `await (await presentation.slide(${a[0]}).shapes).find((s) => s.id === ${a[1]})?.setText(${a[2]});`;
    case 'slide.command':
      return `await presentation.command(${a[0]});`;
    default:
      return `// ${ns}.${name}(${a.join(', ')})`;
  }
}
