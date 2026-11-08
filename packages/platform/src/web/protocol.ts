/** Messages between `WebEngineHost` (main thread) and `engine.worker.ts`. */
export type EngineRequest =
  | { id: number; method: 'init'; fonts: Uint8Array[] }
  | { id: number; method: 'loadDictionary'; aff: Uint8Array; dic: Uint8Array }
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
  | { id: number; method: 'slideInsertPicture'; bytes: Uint8Array; name: string }
  | { id: number; method: 'collabStart'; seed: boolean; clientId?: number }
  | { id: number; method: 'collabApply'; json: string }
  | { id: number; method: 'collabPeers'; json: string }
  | { id: number; method: 'collabStop' }
  | { id: number; method: 'slideCollabStart'; seed: boolean; clientId: number }
  | { id: number; method: 'slideCollabApply'; json: string }
  | { id: number; method: 'slideCollabPeers'; json: string }
  | { id: number; method: 'signPrepare'; bytes: Uint8Array; certsPem: string; options: string }
  | {
      id: number;
      method: 'signTimestampRequest';
      session: number;
      signature: Uint8Array;
      nonce: Uint8Array;
    }
  | {
      id: number;
      method: 'signFinish';
      session: number;
      signature: Uint8Array;
      tsResponse?: Uint8Array;
    }
  | {
      id: number;
      method: 'verifySignatures';
      bytes: Uint8Array;
      anchorsPem: string;
      nowSecs: number;
      /** Base64 CRLs / OCSP responses fetched for `revocationRequests`. */
      revocation: string[];
    }
  | {
      id: number;
      method: 'revocationRequests';
      bytes: Uint8Array;
      anchorsPem: string;
      nowSecs: number;
    };

export type EngineResponse =
  { id: number; ok: true; result: unknown } | { id: number; ok: false; error: string };
