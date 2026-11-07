/** Messages between `WebEngineHost` (main thread) and `engine.worker.ts`. */
export type EngineRequest =
  | { id: number; method: 'init'; fonts: Uint8Array[] }
  | { id: number; method: 'open'; bytes: Uint8Array; name: string; password?: string }
  | { id: number; method: 'page'; index: number }
  | { id: number; method: 'glyphs'; face: number; glyphs: number[] }
  | { id: number; method: 'pdf' }
  | { id: number; method: 'new'; name: string }
  | { id: number; method: 'command'; json: string }
  | { id: number; method: 'insertImage'; bytes: Uint8Array; name: string }
  | { id: number; method: 'media'; name: string }
  | { id: number; method: 'saveDocx' }
  | { id: number; method: 'markSaved' }
  | { id: number; method: 'notes'; index: number }
  | { id: number; method: 'savePptx' }
  | { id: number; method: 'slideCommand'; json: string }
  | { id: number; method: 'slideInsertPicture'; bytes: Uint8Array; name: string };

export type EngineResponse =
  { id: number; ok: true; result: unknown } | { id: number; ok: false; error: string };
