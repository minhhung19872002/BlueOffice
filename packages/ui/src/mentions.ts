// @mentions in comment text (CORE-COMMENT-001): pure helpers shared by the
// comment composers of the three apps. Shapes match `@blueoffice/platform`
// (`Mention`, `MentionCandidate`) structurally.

/** A person the picker offers. */
export interface MentionSuggestion {
  id: string;
  name: string;
  email?: string | null;
  avatarUrl?: string | null;
  source?: 'host' | 'collaborator';
}

/** "@Name" at `text[start, start + length)` (UTF-16 code units). */
export interface MentionSpan {
  id: string;
  name: string;
  email?: string | null;
  start: number;
  length: number;
}

/** A comment being written: its text and the mentions in it. */
export interface MentionDraft {
  text: string;
  mentions: MentionSpan[];
}

/** Mentions whose "@Name" is still intact in `text`. */
export function liveMentions(text: string, mentions: MentionSpan[]): MentionSpan[] {
  return mentions.filter((m) => text.slice(m.start, m.start + m.length) === `@${m.name}`);
}

/**
 * Re-anchors mentions after `before` became `after` (one contiguous edit, ending
 * at `caret` in `after` when known): mentions before the edit stay, those after
 * it shift, a touched one is dropped.
 */
export function shiftMentions(
  before: string,
  after: string,
  mentions: MentionSpan[],
  caret?: number,
): MentionSpan[] {
  const max = Math.min(before.length, after.length);
  let tail = 0;
  // The edit ends at the caret: the text after it is the unchanged tail.
  const tailMax = caret === undefined ? max : Math.min(max, after.length - caret);
  while (tail < tailMax && before[before.length - 1 - tail] === after[after.length - 1 - tail]) {
    tail++;
  }
  let head = 0;
  const headMax = max - tail;
  while (head < headMax && before[head] === after[head]) head++;
  const endBefore = before.length - tail;
  const delta = after.length - before.length;
  const out: MentionSpan[] = [];
  for (const m of mentions) {
    if (m.start + m.length <= head) out.push(m);
    else if (m.start >= endBefore) out.push({ ...m, start: m.start + delta });
  }
  return liveMentions(after, out);
}

/** The `@query` typed just before `caret` (`@` at a word start, no blank after it), or `null`. */
export function mentionQuery(text: string, caret: number): { at: number; query: string } | null {
  const before = text.slice(0, caret);
  const at = before.lastIndexOf('@');
  if (at < 0) return null;
  if (at > 0 && !/\s/.test(before[at - 1] ?? '')) return null;
  const query = before.slice(at + 1);
  if (/[\s@]/.test(query) || query.length > 40) return null;
  return { at, query };
}

/** Replaces the `@query` at `at…caret` with "@Name " for `person`. */
export function insertMention(
  draft: MentionDraft,
  at: number,
  caret: number,
  person: MentionSuggestion,
): MentionDraft & { caret: number } {
  const label = `@${person.name}`;
  const removed = `${draft.text.slice(0, at)}${draft.text.slice(caret)}`;
  const kept = shiftMentions(draft.text, removed, draft.mentions, at).map((m) =>
    m.start >= at ? { ...m, start: m.start + label.length + 1 } : m,
  );
  const text = `${draft.text.slice(0, at)}${label} ${draft.text.slice(caret)}`;
  const mention: MentionSpan = {
    id: person.id,
    name: person.name,
    ...(person.email ? { email: person.email } : {}),
    start: at,
    length: label.length,
  };
  const mentions = liveMentions(
    text,
    [...kept, mention].sort((a, b) => a.start - b.start),
  );
  return { text, mentions, caret: at + label.length + 1 };
}

/** Distinct people mentioned (first occurrence order). */
export function mentionedPeople(mentions: MentionSpan[]): MentionSuggestion[] {
  const out: MentionSuggestion[] = [];
  for (const m of mentions) {
    const key = m.id || m.email || m.name;
    if (!out.some((p) => (p.id || p.email || p.name) === key)) {
      out.push({ id: m.id, name: m.name, ...(m.email ? { email: m.email } : {}) });
    }
  }
  return out;
}

/** Text split into plain and mention pieces (for highlighting). */
export function mentionPieces(
  text: string,
  mentions: MentionSpan[] | undefined,
): { text: string; mention?: MentionSpan }[] {
  const out: { text: string; mention?: MentionSpan }[] = [];
  let pos = 0;
  const sorted = [...(mentions ?? [])]
    .filter((m) => m.start >= 0 && m.start + m.length <= text.length)
    .sort((a, b) => a.start - b.start);
  for (const m of sorted) {
    if (m.start < pos) continue;
    if (m.start > pos) out.push({ text: text.slice(pos, m.start) });
    out.push({ text: text.slice(m.start, m.start + m.length), mention: m });
    pos = m.start + m.length;
  }
  if (pos < text.length) out.push({ text: text.slice(pos) });
  return out;
}
