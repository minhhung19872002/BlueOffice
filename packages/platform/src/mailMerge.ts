// Extended mail merge (DOC-MERGE-001): engine command / result shapes (mirroring
// `bo_engine::merge_cmd` and `bo_edit::merge::{recipients, rules, sheets}`) and the
// host provider interfaces for recipient data (SQL through the server) and e-mail.

/** Comparison of a recipient filter condition. */
export type FilterOp =
  'eq' | 'ne' | 'lt' | 'le' | 'gt' | 'ge' | 'empty' | 'notEmpty' | 'contains' | 'notContains';

/** One filter condition; `or` joins it to the previous one with OR (else AND). */
export interface FilterRule {
  field: string;
  op: FilterOp;
  value?: string;
  or?: boolean;
}

export interface SortKey {
  field: string;
  descending?: boolean;
}

/** Filter, sort (up to 3 keys) and excluded data rows of the recipient list. */
export interface RecipientOptions {
  filter?: FilterRule[];
  sort?: SortKey[];
  excluded?: number[];
}

/** Comparison operators of IF / SKIPIF / NEXTIF rules (ECMA-376 Part 1, §17.16.5.24). */
export type RuleOp = '=' | '<>' | '<' | '<=' | '>' | '>=';

/** A merge rule field the Rules menu inserts. */
export type MergeRule =
  | { kind: 'if'; field: string; op: RuleOp; value: string; then: string; else?: string }
  | { kind: 'skipIf'; field: string; op: RuleOp; value: string }
  | { kind: 'nextIf'; field: string; op: RuleOp; value: string }
  | { kind: 'next' }
  | { kind: 'mergeRec' }
  | { kind: 'mergeSeq' }
  | { kind: 'ask'; name: string; prompt: string; default?: string; once?: boolean }
  | { kind: 'fillIn'; prompt: string; default?: string; once?: boolean }
  | { kind: 'set'; name: string; value: string };

/** A prompt of an ASK / FILLIN field. */
export interface MergePrompt {
  /** `ASK:<bookmark>` or `FILLIN:<prompt>`. */
  key: string;
  kind: 'ask' | 'fillin';
  name?: string;
  prompt: string;
  default: string;
  /** Asked once for the whole merge (`\o`). */
  once: boolean;
}

/** Prompt key → one answer for every record, or one per record of the recipient list. */
export type MergeAnswers = Record<string, string[]>;

/** Label sheet geometry, millimetres. */
export interface LabelSheet {
  id: string;
  pageWidth: number;
  pageHeight: number;
  rows: number;
  cols: number;
  top: number;
  left: number;
  labelWidth: number;
  labelHeight: number;
  hPitch: number;
  vPitch: number;
}

/** Envelope size, millimetres. */
export interface EnvelopeSize {
  id: string;
  width: number;
  height: number;
}

export type LabelMode = 'perRecord' | 'same';

export type MailAttach = 'none' | 'pdf' | 'docx';

/** Extended mail-merge commands of `EditCommand`. */
export type MailMergeCommand =
  | { cmd: 'mergeTable'; name: string; fields: string[]; rows: string[][] }
  | { cmd: 'mergeSetRecipients'; options: RecipientOptions }
  | { cmd: 'mergeRecords'; offset?: number; limit?: number }
  | { cmd: 'mergeDuplicates'; fields?: string[] }
  | { cmd: 'mergeInsertRule'; rule: MergeRule }
  | { cmd: 'mergePrompts' }
  | { cmd: 'mergeSetAnswers'; answers: MergeAnswers }
  | { cmd: 'mergeToPdfs'; pattern?: string }
  | {
      cmd: 'mergeToEmails';
      to: string;
      subject: string;
      attach?: MailAttach;
      attachmentName?: string;
    }
  | {
      cmd: 'mergeEnvelope';
      size: EnvelopeSize;
      delivery: string;
      returnAddress?: string;
      /** Delivery address position from the page's top-left corner, mm. */
      at?: [number, number] | null;
    }
  | { cmd: 'mergeLabels'; sheet: LabelSheet; text: string; mode: LabelMode }
  | { cmd: 'mergePresets' };

export interface RecordRow {
  index: number;
  values: string[];
  included: boolean;
  /** Position in the recipient list, absent when filtered out or excluded. */
  position?: number;
}

export interface MergeRecords {
  fields: string[];
  total: number;
  active: number;
  rows: RecordRow[];
}

export interface MailAttachment {
  name: string;
  contentType: string;
  base64: string;
}

/** A merged e-mail. */
export interface MailMessage {
  to: string;
  subject: string;
  /** HTML body (the merged document). */
  html: string;
  attachment?: MailAttachment;
}

/** Results of the extended mail-merge commands (`EditResult.mergeOut`). */
export interface MergeOut {
  records?: MergeRecords;
  duplicates?: number[][];
  prompts?: MergePrompt[];
  files?: { name: string; pages: number }[];
  /** Zip of the per-recipient PDFs, base64. */
  zip?: string;
  mails?: MailMessage[];
  skipped?: number;
  presets?: { labels: LabelSheet[]; envelopes: EnvelopeSize[] };
}

/** A table of recipients a data provider returns (display text). */
export interface MergeTable {
  name: string;
  fields: string[];
  rows: string[][];
}

/** A data source a provider offers (e.g. a saved query on the server). */
export interface MergeSourceInfo {
  id: string;
  /** Display name (from the host, not translated by the editor). */
  label: string;
  /** The source takes a query text (e.g. SQL run by the server). */
  query?: boolean;
}

/**
 * Recipient data from the host — e.g. SQL run by the server against a database the
 * administrator configured. The client never holds database drivers or credentials.
 */
export interface MergeDataProvider {
  sources(): Promise<MergeSourceInfo[]>;
  load(sourceId: string, query?: string): Promise<MergeTable>;
}

export interface MailSendResult {
  sent: number;
  /** Per-message failures (index into the batch, message from the host). */
  failed: { index: number; error: string }[];
}

/** Sends merged e-mails (server SMTP relay, desktop mail client, …). */
export interface MailProvider {
  send(messages: MailMessage[]): Promise<MailSendResult>;
}
