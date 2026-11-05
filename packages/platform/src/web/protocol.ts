/** Messages between `WebEngineHost` (main thread) and `engine.worker.ts`. */
export type EngineRequest =
  | { id: number; method: 'init'; fonts: Uint8Array[] }
  | { id: number; method: 'open'; bytes: Uint8Array; name: string }
  | { id: number; method: 'page'; index: number }
  | { id: number; method: 'glyphs'; face: number; glyphs: number[] }
  | { id: number; method: 'pdf' };

export type EngineResponse =
  | { id: number; ok: true; result: unknown }
  | { id: number; ok: false; error: string };
