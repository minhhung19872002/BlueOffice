/**
 * Signature lines (CORE-SIGLINE-001): mirrors `bo_edit::sigline` and
 * `bo_engine::sigline` (ECMA-376 Part 4 `o:signatureline`; the signature that
 * signs a line carries [MS-OFFCRYPTO] `SignatureInfoV1` with its id).
 */

/** What the insert dialog asks for (`bo_edit::sigline::SignatureLineSpec`). */
export interface SignatureLineSpec {
  suggestedSigner: string;
  suggestedSigner2: string;
  suggestedSignerEmail: string;
  signingInstructions: string;
  allowComments: boolean;
  showSignDate: boolean;
}

/** The signed state read from the package signatures (display only). */
export interface SignedLine {
  text: string;
  /** `YYYY-MM-DD`, empty when unknown. */
  date: string;
  /** Verification result pushed by the host; `null` = not verified. */
  valid: boolean | null;
}

/** A line's attributes (`bo_model::SignatureLine`). */
export interface SignatureLine {
  id: string;
  providerId: string;
  isSignatureLine: boolean;
  suggestedSigner: string;
  suggestedSigner2: string;
  suggestedSignerEmail: string;
  signingInstructions: string;
  allowComments: boolean;
  showSignDate: boolean;
  signed?: SignedLine;
}

/** A line of the body with its address (`bo_edit::sigline::SignatureLineInfo`). */
export interface SignatureLineInfo {
  para: number;
  at: number;
  line: SignatureLine;
}

/** Pictures of a line as signed / invalid, PNG base64 (`SignatureLineImages`). */
export interface SignatureLineImages {
  id: string;
  valid: string;
  invalid: string;
}

/** Signature line commands (a type alias so the union keeps an index signature). */
export type SignatureLineCommand =
  /** Insert a line at the caret; `id` is a braced GUID (derived when absent). */
  | { cmd: 'insertSignatureLine'; spec: SignatureLineSpec; id?: string }
  /** The document's lines → `signatureLines`. */
  | { cmd: 'signatureLines' }
  /** Pictures for signing line `id` → `signatureLineImages`. */
  | {
      cmd: 'signatureLineImages';
      id: string;
      text: string;
      imageBase64?: string | null;
      date: string;
    }
  /** Show verification results on the signed lines (display only). */
  | { cmd: 'setSignatureLineStatus'; lines: { id: string; valid: boolean | null }[] };
