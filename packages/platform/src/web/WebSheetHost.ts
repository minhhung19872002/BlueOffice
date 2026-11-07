import { RpcSheetHost, type SheetMethod } from '../sheet';
import type { FontSource } from '../types';

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };
type Reply = { id: number; ok: true; result: unknown } | { id: number; ok: false; error: string };

/** Creates the {@link RpcSheetHost} backed by `sheet.worker.ts`. */
export function createWebSheetHost(
  fonts: FontSource,
  locale: () => string = () => 'vi-VN',
  createWorker: () => Worker = () =>
    new Worker(new URL('./sheet.worker.ts', import.meta.url), { type: 'module' }),
): RpcSheetHost {
  let worker: Worker | null = null;
  let seq = 0;
  const pending = new Map<number, Pending>();

  const post = (body: Record<string, unknown>, transfer: Transferable[] = []): Promise<unknown> => {
    if (!worker) {
      worker = createWorker();
      worker.onmessage = (ev: MessageEvent<Reply>) => {
        const r = ev.data;
        const p = pending.get(r.id);
        if (!p) return;
        pending.delete(r.id);
        if (r.ok) p.resolve(r.result);
        else p.reject(new Error(r.error));
      };
      worker.onerror = (ev) => {
        const err = new Error(ev.message || 'engine.workerCrashed');
        for (const p of pending.values()) p.reject(err);
        pending.clear();
      };
    }
    const id = ++seq;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      worker!.postMessage({ ...body, id }, transfer);
    });
  };

  let ready: Promise<void> | null = null;
  const init = (): Promise<void> => {
    ready ??= (async () => {
      const list = await fonts.list();
      const data = await Promise.all(list.map((f) => fonts.load(f.file)));
      await post(
        { method: 'init', fonts: data, locale: locale() },
        data.map((d) => d.buffer as ArrayBuffer),
      );
    })();
    return ready;
  };

  return new RpcSheetHost((method: SheetMethod, args: unknown[]) => post({ method, args }), init);
}
