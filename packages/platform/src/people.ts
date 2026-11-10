/**
 * @mentions and assigned tasks in comments (CORE-COMMENT-001). Shapes mirror
 * `bo_model::mention`; the people directory and the notifications go through
 * the server of a hosted session (`GET /api/sessions/{id}/people`,
 * `POST …/mentions`; docs/bohp.md §A3).
 */

/** A person who can be mentioned (mirrors `bo_model::mention::MentionPerson`). */
export interface MentionPerson {
  /** Id at the host; empty for people read from a file. */
  id: string;
  name: string;
  email?: string | null;
}

/** "@Name" at `text[start, start + length)` (UTF-16 code units). */
export interface Mention extends MentionPerson {
  start: number;
  length: number;
}

/** A task assigned in a comment; complete when its thread is resolved. */
export interface CommentTask {
  assignee: MentionPerson;
}

/** One comment of a sheet / slide thread (engine `CommentView` / `CommentOut`). */
export interface ThreadCommentInfo {
  id: string;
  author: string;
  date: string;
  text: string;
  mentions?: Mention[];
  task?: CommentTask | null;
}

/** A person offered by the picker. */
export interface MentionCandidate extends MentionPerson {
  avatarUrl?: string | null;
  /** `host` directory or a current `collaborator`. */
  source: 'host' | 'collaborator';
}

/** A posted comment with new mentions (`POST …/mentions`). */
export interface CommentNotice {
  commentId: string;
  text: string;
  mentions: MentionPerson[];
  assignee?: MentionPerson | null;
  app: 'doc' | 'sheet' | 'slide';
}

/** What the server sent for a {@link CommentNotice}. */
export interface CommentNoticeResult {
  recipients: number;
  host: 'sent' | 'unsupported' | 'failed' | 'none';
  emails: number;
}

/** People to mention + notifications (hosted sessions only). */
export interface PeopleDirectory {
  search(query: string): Promise<MentionCandidate[]>;
  notify(notice: CommentNotice): Promise<CommentNoticeResult>;
}

/**
 * Tells the people newly mentioned in a posted comment (and its assignee) —
 * through the hosted session's server. Best effort: never fails the comment.
 */
export async function notifyMentions(
  people: PeopleDirectory | undefined,
  notice: CommentNotice,
): Promise<CommentNoticeResult | null> {
  const targets = notice.mentions.filter((m) => m.id);
  if (!people || (targets.length === 0 && !notice.assignee?.id)) return null;
  try {
    return await people.notify({ ...notice, mentions: targets });
  } catch {
    return null;
  }
}

/** Engine fields of a comment with mentions (`mentions`, `task`). */
export function mentionFields(
  mentions: Mention[],
  assignee: MentionPerson | null | undefined,
): { mentions?: Mention[]; task?: CommentTask } {
  const person = (p: MentionPerson): MentionPerson => ({
    id: p.id,
    name: p.name,
    ...(p.email ? { email: p.email } : {}),
  });
  return {
    ...(mentions.length
      ? { mentions: mentions.map((m) => ({ ...person(m), start: m.start, length: m.length })) }
      : {}),
    ...(assignee ? { task: { assignee: person(assignee) } } : {}),
  };
}

/** Up to two initials of a display name ("Trần Thị Bình" gives "TB"); "?" when empty (presence badges). */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/u).filter(Boolean);
  if (!words.length) return '?';
  const first = [...(words[0] ?? '')][0] ?? '';
  const last = words.length > 1 ? ([...(words[words.length - 1] ?? '')][0] ?? '') : '';
  return (first + last).toLocaleUpperCase();
}
