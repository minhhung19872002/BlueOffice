import { describe, expect, it } from 'vitest';
import { notifyMentions } from '@blueoffice/platform';
import { sessionPeople, type FetchLike } from '../src';

const session = () => ({
  sessionId: 'S 1',
  sessionToken: 'secret',
  mode: 'edit' as const,
  file: { id: 'F1', name: 'Kế hoạch.docx', version: '3' },
  allowedOrigins: [],
});

function server(fail = false) {
  const calls: { url: string; method: string; body?: string; headers: Record<string, string> }[] =
    [];
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({
      url,
      method: init?.method ?? 'GET',
      headers: init?.headers ?? {},
      ...(typeof init?.body === 'string' ? { body: init.body } : {}),
    });
    const body = url.includes('/people')
      ? {
          people: [
            { id: 'u2', name: 'Trần Thị Bình', email: 'u2@drive.mock', source: 'host' },
            { id: 'w9', name: 'Khách', email: null, source: 'collaborator' },
            { name: 'no id' },
          ],
        }
      : { recipients: 1, host: 'sent', emails: 1 };
    return {
      ok: !fail,
      status: fail ? 500 : 200,
      json: async () => body,
      arrayBuffer: async () => new ArrayBuffer(0),
    };
  };
  return { fetchImpl, calls };
}

describe('session people directory (CORE-COMMENT-001)', () => {
  it('searches people and posts mention notifications with the session token', async () => {
    const { fetchImpl, calls } = server();
    const people = sessionPeople('https://office', session(), fetchImpl);
    expect(await people.search('Bình')).toEqual([
      {
        id: 'u2',
        name: 'Trần Thị Bình',
        email: 'u2@drive.mock',
        avatarUrl: null,
        source: 'host',
      },
      { id: 'w9', name: 'Khách', email: null, avatarUrl: null, source: 'collaborator' },
    ]);
    expect(calls[0]?.url).toBe('https://office/api/sessions/S%201/people?q=B%C3%ACnh&limit=8');
    expect(calls[0]?.headers['x-session-token']).toBe('secret');

    const r = await notifyMentions(people, {
      commentId: '7',
      text: '@Trần Thị Bình xem',
      mentions: [
        { id: 'u2', name: 'Trần Thị Bình', email: 'u2@drive.mock' },
        { id: '', name: 'X' },
      ],
      assignee: { id: 'u2', name: 'Trần Thị Bình' },
      app: 'doc',
    });
    expect(r).toEqual({ recipients: 1, host: 'sent', emails: 1 });
    const post = calls[1]!;
    expect(post.method).toBe('POST');
    expect(post.url).toBe('https://office/api/sessions/S%201/mentions');
    // Only ids and names go to the server (it looks the e-mail addresses up itself).
    expect(JSON.parse(post.body ?? '')).toEqual({
      commentId: '7',
      text: '@Trần Thị Bình xem',
      app: 'doc',
      mentions: [{ id: 'u2', name: 'Trần Thị Bình' }],
      assignee: { id: 'u2', name: 'Trần Thị Bình' },
    });
  });

  it('never fails the comment: no directory, nobody to tell, or a server error', async () => {
    const { fetchImpl, calls } = server(true);
    const people = sessionPeople('', session(), fetchImpl);
    expect(await people.search('a')).toEqual([]);
    const notice = {
      commentId: '1',
      text: 'x',
      mentions: [{ id: 'u2', name: 'B' }],
      app: 'sheet' as const,
    };
    expect(await notifyMentions(people, notice)).toBeNull();
    expect(await notifyMentions(undefined, notice)).toBeNull();
    const before = calls.length;
    expect(await notifyMentions(people, { ...notice, mentions: [] })).toBeNull();
    expect(calls.length).toBe(before);
  });
});
