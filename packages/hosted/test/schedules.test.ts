/** Server schedules of BlueScript runs through the session API (CORE-SCRIPT-002/004). */
import { describe, expect, it } from 'vitest';
import { sessionScriptSchedules, type FetchLike, type HostedSession } from '../src/session';

const session: HostedSession = {
  sessionId: 'abc',
  sessionToken: 'tok',
  mode: 'edit',
  file: { id: 'f1', name: 'Báo cáo.docx' },
  allowedOrigins: [],
};

const json = (status: number, body: unknown) => ({
  ok: status >= 200 && status < 300,
  status,
  json: async () => body,
  arrayBuffer: async () => new ArrayBuffer(0),
});

describe('sessionScriptSchedules', () => {
  it('lists runs and enables a schedule with the session token', async () => {
    const seen: { url: string; method: string; token?: string; body?: unknown }[] = [];
    const fetchImpl: FetchLike = async (url, init) => {
      seen.push({
        url,
        method: init?.method ?? 'GET',
        token: init?.headers?.['x-session-token'],
        body: typeof init?.body === 'string' ? JSON.parse(init.body) : undefined,
      });
      if (url.endsWith('/runs')) {
        return json(200, {
          runs: [
            {
              startedAt: '2026-11-11T07:00:00Z',
              durationMs: 12,
              status: 'succeeded',
              error: null,
              logs: '[log] ok\n',
              version: '3',
            },
          ],
        });
      }
      return json(200, {
        id: 's 1',
        name: 'A',
        cron: '0 7 * * *',
        enabled: true,
        nextRunAt: '2026-11-12T07:00:00Z',
        lastRunAt: null,
        lastStatus: 'failed',
        lastError: 'x',
        consecutiveFailures: 0,
        disabledReason: null,
      });
    };
    const s = sessionScriptSchedules('https://bo', session, fetchImpl);
    const runs = await s.runs!('s 1');
    expect(runs).toEqual([
      {
        startedAt: '2026-11-11T07:00:00Z',
        durationMs: 12,
        status: 'succeeded',
        error: null,
        logs: '[log] ok\n',
        version: '3',
      },
    ]);
    const updated = await s.setEnabled!('s 1', true);
    expect(updated.enabled).toBe(true);
    expect(updated.consecutiveFailures).toBe(0);
    expect(updated.disabledReason).toBeNull();
    expect(seen).toEqual([
      { url: 'https://bo/api/sessions/abc/schedules/s%201/runs', method: 'GET', token: 'tok' },
      {
        url: 'https://bo/api/sessions/abc/schedules/s%201',
        method: 'PATCH',
        token: 'tok',
        body: { enabled: true },
      },
    ]);
  });

  it('reports failures', async () => {
    const s = sessionScriptSchedules('', session, async () => json(404, {}));
    await expect(s.runs!('x')).rejects.toThrow('session.scheduleRuns 404');
    await expect(s.setEnabled!('x', false)).rejects.toThrow('session.scheduleEnable 404');
  });
});
