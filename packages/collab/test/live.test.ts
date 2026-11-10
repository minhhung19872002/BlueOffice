/**
 * Two real WASM engines, each with its own `bo-collab` replica, behind a fake
 * relay: BlueDoc text and BlueSlide objects typed in one editor reach the
 * other; concurrent edits converge; frames arriving while the engine is busy
 * are held until it is done.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync, WasmCollab, WasmEngine } from '@blueoffice/engine-wasm';
import {
  collabDocHost,
  collabSlideHost,
  colorFor,
  isEmptyChange,
  type DocResultLike,
  type SlideResultLike,
} from '../src';

const root = join(__dirname, '..', '..', '..');
let fonts: Uint8Array[] = [];

beforeAll(() => {
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
  const dir = join(root, 'assets', 'fonts');
  const manifest = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8')) as {
    fonts: string[];
  };
  fonts = manifest.fonts.map((f) => readFileSync(join(dir, f)));
});

/** Engine calls resolve on a later task, like the Web Worker host. */
const later = () => new Promise((r) => setTimeout(r, 1));

interface Result extends DocResultLike {
  selection: { anchor: { p: number; o: number }; focus: { p: number; o: number } };
  matches?: unknown[];
}
interface SlideResult extends SlideResultLike {
  selection: { id: number }[];
}

function hostOf(e: WasmEngine) {
  const parse = <T>(s: string) => JSON.parse(s) as T;
  return {
    open: async (b: Uint8Array, n: string) => {
      await later();
      return parse<unknown>(e.open(b, n, undefined));
    },
    newDocument: async (n: string) => parse<unknown>(e.newDocument(n)),
    command: async (c: { cmd: string }) => {
      await later();
      return parse<Result>(e.command(JSON.stringify(c)));
    },
    insertImage: async (b: Uint8Array, n: string) => parse<Result>(e.insertImage(b, n)),
    collabStart: async (seed: boolean, id?: number) => parse<object>(e.collabStart(seed, id)),
    collabApply: async (j: string) => {
      await later();
      return parse<Result>(e.collabApply(j));
    },
    collabPeers: async (s: [number, number][]) =>
      parse<{ caret: { page: number } | null; rects: unknown[] }[]>(
        e.collabPeers(JSON.stringify(s)),
      ),
    slideCommand: async (c: { cmd: string }) => {
      await later();
      return parse<SlideResult>(e.slideCommand(JSON.stringify(c)));
    },
    slideInsertPicture: async (b: Uint8Array, n: string) =>
      parse<SlideResult>(e.slideInsertPicture(b, n)),
    slideCollabStart: async (seed: boolean, id: number) =>
      parse<object>(e.slideCollabStart(seed, id)),
    slideCollabApply: async (j: string) => {
      await later();
      return parse<SlideResult>(e.slideCollabApply(j));
    },
    slideCollabPeers: async (k: string[]) =>
      parse<({ slide: number; rect: { cx: number; cy: number } } | null)[]>(
        e.slideCollabPeers(JSON.stringify(k)),
      ),
  };
}

function peer(id: number, name: string) {
  const replica = new WasmCollab(id);
  const engine = new WasmEngine();
  for (const f of fonts) engine.addFont(f);
  const outbox: Uint8Array[] = [];
  const user = { userId: `u${id}`, name, color: colorFor(id) };
  const host = hostOf(engine);
  const doc = collabDocHost(host, replica, () => outbox.push(...doc.engine.drainOutbox()), user);
  const slides = collabSlideHost(
    host,
    replica,
    () => outbox.push(...slides.engine.drainOutbox()),
    user,
  );
  return { replica, engine, doc, slides, outbox };
}

type Peer = ReturnType<typeof peer>;
type Side = 'doc' | 'slides';

/** Delivers `from`'s queued frames to `to` (replies go back). */
function send(from: Peer, to: Peer, side: Side) {
  for (const f of from.outbox.splice(0)) {
    for (const r of to[side].engine.receive(f)) from[side].engine.receive(r);
  }
}

async function settle(a: Peer, b: Peer, side: Side) {
  for (let i = 0; i < 4; i++) {
    send(a, b, side);
    send(b, a, side);
    await a[side].idle();
    await b[side].idle();
  }
}

function connect(a: Peer, b: Peer, side: Side) {
  for (const r of b[side].engine.receive(a[side].engine.start())) a[side].engine.receive(r);
  for (const r of a[side].engine.receive(b[side].engine.start())) b[side].engine.receive(r);
}

const enc = new TextEncoder();
const sel = (p: number, o: number) => ({
  cmd: 'setSelection',
  selection: { anchor: { p, o }, focus: { p, o } },
});

describe('BlueDoc collaboration over bo-collab', () => {
  it('seeds, shares typing and converges under concurrent edits', async () => {
    const a = peer(31, 'An');
    const b = peer(32, 'Bình');
    await a.doc.seed();
    await a.doc.open(enc.encode('Xin chào\nViệt Nam'), 'chung.txt');
    connect(a, b, 'doc');
    await b.doc.open(enc.encode('bản cũ của Bình'), 'chung.txt');
    await settle(a, b, 'doc');
    expect(b.engine.collabText()).toBe('Xin chào\nViệt Nam\n');

    let external = 0;
    b.doc.onExternalChange(() => external++);
    await a.doc.command(sel(0, enc.encode('Xin chào').length));
    await a.doc.command({ cmd: 'insertText', text: ' các bạn' } as { cmd: string });
    // B types while A's frames arrive mid-command: they are held, then applied.
    const typing = b.doc
      .command(sel(1, 0))
      .then(() => b.doc.command({ cmd: 'insertText', text: 'Nước ' } as { cmd: string }));
    send(a, b, 'doc');
    await typing;
    await settle(a, b, 'doc');
    expect(external).toBeGreaterThan(0);
    expect(a.engine.collabText()).toBe('Xin chào các bạn\nNước Việt Nam\n');
    expect(b.engine.collabText()).toBe(a.engine.collabText());
    expect(a.replica.fullState()).toBe(b.replica.fullState());
    // The engine's model has the text (find goes through the model).
    const found = (await b.doc.command({
      cmd: 'find',
      query: 'các bạn',
    } as { cmd: string })) as Result;
    expect(found.matches).toHaveLength(1);
  });

  it('a late joiner typing right away (caret untouched) keeps and shares the text', async () => {
    const docx = readFileSync(join(root, 'tests', 'corpus', 'doc', 'phase1-sample.docx'));
    const a = peer(61, 'An');
    const b = peer(62, 'Bình');
    await a.doc.seed();
    await a.doc.open(docx, 'mẫu.docx');
    // B's editor opens the file before the room's state arrives (the real order).
    await b.doc.open(docx, 'mẫu.docx');
    connect(a, b, 'doc');
    await settle(a, b, 'doc');
    expect(b.engine.collabText()).toBe(a.engine.collabText());
    // B types at the caret the shared body left it at, then A types at the top.
    await b.doc.command({ cmd: 'insertText', text: 'Bình gõ ngay. ' } as { cmd: string });
    await settle(a, b, 'doc');
    await a.doc.command(sel(0, 0));
    await a.doc.command({ cmd: 'insertText', text: 'An ' } as { cmd: string });
    await settle(a, b, 'doc');
    expect(a.engine.collabText().startsWith('An ')).toBe(true);
    expect(a.engine.collabText()).toContain('Bình gõ ngay. ');
    expect(b.engine.collabText()).toBe(a.engine.collabText());
    expect(a.replica.fullState()).toBe(b.replica.fullState());
  });

  it('undoes only the own edits and shows collaborators carets', async () => {
    const a = peer(41, 'An');
    const b = peer(42, 'Bình');
    await a.doc.seed();
    await a.doc.open(enc.encode('abc'), 'x.txt');
    connect(a, b, 'doc');
    await b.doc.open(enc.encode('abc'), 'x.txt');
    await settle(a, b, 'doc');

    const peers: { name: string; color: string; caret: unknown }[][] = [];
    b.doc.onPeers((p) => peers.push(p));
    await a.doc.command(sel(0, 3));
    await a.doc.command({ cmd: 'insertText', text: 'A' } as { cmd: string });
    await settle(a, b, 'doc');
    await b.doc.command(sel(0, 0));
    await b.doc.command({ cmd: 'insertText', text: 'B' } as { cmd: string });
    await settle(a, b, 'doc');
    expect(a.engine.collabText()).toBe('BabcA\n');

    const r = (await a.doc.command({ cmd: 'undo' })) as Result;
    expect(r.format.canRedo).toBe(true);
    await settle(a, b, 'doc');
    expect(b.engine.collabText()).toBe('Babc\n');
    expect(a.engine.collabText()).toBe('Babc\n');

    const last = peers.at(-1) ?? [];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ name: 'An', color: colorFor(41) });
    expect(last[0]?.caret).not.toBeNull();
  });
});

describe('BlueSlide collaboration over bo-collab', () => {
  it('a move by one user and a text edit by another converge', async () => {
    const deck = readFileSync(join(root, 'tests', 'corpus', 'slide', 'slide-edit-arrange.pptx'));
    const a = peer(51, 'An');
    const b = peer(52, 'Bình');
    await a.slides.seed();
    await a.slides.open(deck, 'deck.pptx');
    connect(a, b, 'slides');
    await b.slides.open(deck, 'deck.pptx');
    await settle(a, b, 'slides');
    expect(isEmptyChange(JSON.parse(a.replica.fullState()) as object)).toBe(false);

    const all = (await a.slides.slideCommand({ cmd: 'selectAll' })) as SlideResult;
    const [first, second] = all.selection.map((s) => s.id);
    expect(second).toBeDefined();
    await a.slides.slideCommand({ cmd: 'select', ids: [first] } as { cmd: string });
    await a.slides.slideCommand({
      cmd: 'transform',
      transform: { kind: 'move', dx: 30, dy: 0 },
      snap: false,
    } as { cmd: string });
    await b.slides.slideCommand({ cmd: 'setText', id: second, text: 'Chào' } as {
      cmd: string;
    });
    await settle(a, b, 'slides');
    expect(a.replica.fullState()).toBe(b.replica.fullState());

    const slideId = Object.keys(
      (JSON.parse(a.replica.fullState()) as { slides: Record<string, string> }).slides,
    ).sort((x, y) => {
      const pos = (k: string) =>
        (
          JSON.parse(
            (JSON.parse(a.replica.fullState()) as { slides: Record<string, string> }).slides[k]!,
          ) as { pos: number }
        ).pos;
      return pos(x) - pos(y);
    })[0];
    const key = `${slideId}/${first}`;
    const [ra] = await a.slides.slideCollabPeers([key]);
    const [rb] = await b.slides.slideCollabPeers([key]);
    expect(ra).toBeTruthy();
    expect(rb).toEqual(ra);

    // Presence: B sees A's selected object.
    const seen: { name: string; slide: number }[][] = [];
    b.slides.onSlidePeers((p) => seen.push(p));
    await a.slides.slideCommand({ cmd: 'select', ids: [second] } as { cmd: string });
    await settle(a, b, 'slides');
    expect(seen.at(-1)?.[0]).toMatchObject({ name: 'An', slide: 0 });
  });

  // SLIDE-ALBUM-001: a photo album is many commands joined by `historyLen` … `mergeHistory`.
  it('commands joined into one undo step are one collaborative undo step', async () => {
    const deck = readFileSync(join(root, 'tests', 'corpus', 'slide', 'slide-edit-arrange.pptx'));
    const a = peer(61, 'An');
    const b = peer(62, 'Bình');
    await a.slides.seed();
    await a.slides.open(deck, 'deck.pptx');
    connect(a, b, 'slides');
    await b.slides.open(deck, 'deck.pptx');
    await settle(a, b, 'slides');
    const count = (p: Peer) => (JSON.parse(p.engine.info()) as { pageCount: number }).pageCount;
    const n = count(a);
    // The replica groups undo steps by time: let a second pass at every look at the clock,
    // and let a frame of the other user arrive between the commands.
    let now = Date.now();
    const clock = vi.spyOn(Date, 'now').mockImplementation(() => (now += 1000));
    const between = async () => {
      await b.slides.slideCommand({ cmd: 'selectNext' });
      await settle(a, b, 'slides');
    };
    const box = { cmd: 'insertTextBox', text: 'Ảnh', rect: { x: 10, y: 10, w: 100, h: 30 } };
    try {
      // Separate commands: each is its own undo step.
      await a.slides.slideCommand({ cmd: 'addSlide' });
      await between();
      await a.slides.slideCommand(box);
      await between();
      expect(count(a)).toBe(n + 1);
      await a.slides.slideCommand({ cmd: 'undo' });
      expect(count(a)).toBe(n + 1);
      await a.slides.slideCommand({ cmd: 'undo' });
      expect(count(a)).toBe(n);
      await settle(a, b, 'slides');
      // The same commands as a batch: one undo takes all of it back, on both sides.
      const start = (await a.slides.slideCommand({ cmd: 'historyLen' })) as SlideResult & {
        history: number;
      };
      await a.slides.slideCommand({ cmd: 'addSlide' });
      await between();
      await a.slides.slideCommand(box);
      await between();
      await a.slides.slideCommand({ cmd: 'mergeHistory', since: start.history } as {
        cmd: string;
      });
      await settle(a, b, 'slides');
      expect([count(a), count(b)]).toEqual([n + 1, n + 1]);
      await a.slides.slideCommand({ cmd: 'undo' });
      await settle(a, b, 'slides');
      expect([count(a), count(b)]).toEqual([n, n]);
      // An edit after the batch is a step of its own again.
      await a.slides.slideCommand({ cmd: 'redo' });
      await between();
      await a.slides.slideCommand({ cmd: 'addSlide' });
      await between();
      expect(count(a)).toBe(n + 2);
      await a.slides.slideCommand({ cmd: 'undo' });
      expect(count(a)).toBe(n + 1);
    } finally {
      clock.mockRestore();
    }
  });
});
