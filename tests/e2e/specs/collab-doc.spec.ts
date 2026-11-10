// BlueDoc live collaboration against the real server and mock BlueDrive
// (prompt §6A acceptance): two users edit one .docx from the host page, see
// each other's text, formatting and carets; the host's `save` command
// creates a version listing both editors; revoking B's edit permission
// switches B to view mode; B's offline edits merge after reconnecting.
import { expect, test, type FrameLocator } from '@playwright/test';
import {
  USERS,
  createFromCorpus,
  documentXml,
  fileState,
  folderFiles,
  hostCommand,
  hostEvents,
  notifyPermissions,
  openAs,
  setPerms,
  token,
  urls,
  type Participant,
} from '../collab/drive';

const words = (f: FrameLocator) => f.getByTestId('status-words');

async function wordCount(f: FrameLocator): Promise<number> {
  const t = (await words(f).textContent()) ?? '';
  return Number(/(\d[\d.]*)/.exec(t)?.[1]?.replace(/\./g, '') ?? NaN);
}

/** Waits until the word count differs from `from`; returns it. */
async function changed(f: FrameLocator, from: number): Promise<number> {
  await expect.poll(() => wordCount(f)).not.toBe(from);
  // Settled: the engine may still be applying queued input.
  let n = await wordCount(f);
  for (;;) {
    await new Promise((r) => setTimeout(r, 400));
    const m = await wordCount(f);
    if (m === n) return n;
    n = m;
  }
}

/** Puts the caret at the start of the document. */
async function caretAtStart(p: Participant) {
  const page1 = p.frame.locator('.bo-page[data-page="1"]');
  await expect(page1).toHaveAttribute('data-rendered', 'true');
  const box = (await page1.boundingBox())!;
  // Locator clicks scroll the point into view (the view may follow remote edits).
  await page1.click({ position: { x: box.width * 0.3, y: box.height * 0.12 } });
  await expect(p.frame.getByTestId('caret')).toBeVisible();
  await p.page.keyboard.press('Control+Home');
}

/** Selects the first match of `text` through the find bar. */
async function select(p: Participant, text: string) {
  await p.page.keyboard.press('Control+f');
  const q = p.frame.getByTestId('find-query');
  await q.fill(text);
  await q.press('Enter');
  await expect(p.frame.getByTestId('find-message')).toHaveText(/kết quả/);
}

test('two users edit one .docx: text, bold, carets, save, revoke, offline', async ({ browser }) => {
  const fileId = await createFromCorpus('doc/phase1-sample.docx', 'Báo cáo cộng tác.docx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  await expect(words(a.frame)).toBeVisible();
  await expect(words(b.frame)).toBeVisible();
  const initial = await wordCount(a.frame);
  await expect.poll(() => wordCount(b.frame)).toBe(initial);
  // Both are in the room (presence reaches the host page too).
  await expect
    .poll(async () => {
      const ev = (await hostEvents(a)).filter((e) => e.event === 'collaborators-changed');
      return (ev.at(-1)?.payload as unknown[] | undefined)?.length ?? 0;
    })
    .toBe(2);

  // (a) A types Vietnamese text; B sees it.
  await caretAtStart(a);
  await a.page.keyboard.type('Xin chào ');
  await a.page.keyboard.insertText('Việt Nam cộng tác. ');
  const typed = await changed(a.frame, initial);
  await expect.poll(() => wordCount(b.frame)).toBe(typed);

  // B bolds a word; A sees it bold.
  await caretAtStart(b);
  await select(b, 'soạn thảo bằng');
  await b.frame.locator('[data-command="format.bold"]').click();
  await expect(b.frame.locator('[data-command="format.bold"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await select(a, 'soạn thảo bằng');
  await expect(a.frame.locator('[data-command="format.bold"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );

  // The other user's caret is drawn with their name.
  await expect(a.frame.getByTestId('peer-caret').first()).toBeVisible();
  await expect(a.frame.getByTestId('peer-caret').first()).toContainText(USERS.u2);
  await expect(b.frame.getByTestId('peer-caret').first()).toContainText(USERS.u1);
  // …and a bar in their colour marks the paragraph they are in (CORE-COLLAB-001).
  const bar = a.frame.getByTestId('peer-bar').first();
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute('aria-label', `${USERS.u2} đang sửa đoạn này`);
  await expect(bar).toContainText(USERS.u2);
  const barBox = (await bar.boundingBox())!;
  const caretBox = (await a.frame.getByTestId('peer-caret').first().boundingBox())!;
  expect(barBox.x).toBeLessThan(caretBox.x);
  expect(barBox.y).toBeLessThanOrEqual(caretBox.y + 1);
  expect(barBox.y + barBox.height).toBeGreaterThanOrEqual(caretBox.y + caretBox.height - 1);

  // (c) The host's `save` command creates a version listing both editors.
  const before = (await fileState(fileId)).versions.length;
  await expect(hostCommand(a, 'save')).resolves.toEqual({ saved: true });
  await expect.poll(async () => (await fileState(fileId)).versions.length).toBe(before + 1);
  const saved = (await fileState(fileId)).versions.at(-1)!;
  expect(saved.editors.sort()).toEqual(['u1', 'u2']);
  expect(saved.note).toContain(USERS.u1);
  const xml = documentXml(saved);
  expect(xml).toContain('Xin chào Việt Nam');
  // "soạn thảo bằng" is in a bold run (ECMA-376 Part 1, §17.3.2.1 b).
  expect(xml).toMatch(/<w:r\b(?:(?!<\/w:r>).)*<w:b\/>(?:(?!<\/w:r>).)*soạn thảo bằng/s);
  await expect.poll(async () => (await hostEvents(a)).some((e) => e.event === 'saved')).toBe(true);
  // Ctrl+S inside the editor saves to BlueDrive too (not a download).
  await caretAtStart(b);
  await b.page.keyboard.insertText('Đã lưu ');
  const stored = await changed(b.frame, typed);
  await expect.poll(() => wordCount(a.frame)).toBe(stored);
  await b.page.keyboard.press('Control+s');
  await expect.poll(async () => (await fileState(fileId)).versions.length).toBe(before + 2);
  expect(documentXml((await fileState(fileId)).versions.at(-1)!)).toContain('Đã lưu');
  await expect(b.frame.getByTestId('status-modified')).toHaveCount(0);

  // (e) B goes offline, keeps editing, comes back: the edits merge live.
  await b.context.setOffline(true);
  await expect(b.frame.getByTestId('hosted-banner')).toHaveAttribute('data-state', 'offline');
  await caretAtStart(b);
  await b.page.keyboard.insertText('Ngoại tuyến ');
  const offline = await changed(b.frame, stored);
  // A does not see it while B is offline.
  await a.page.waitForTimeout(1000);
  expect(await wordCount(a.frame)).toBe(stored);
  await b.context.setOffline(false);
  await expect(b.frame.getByTestId('hosted-banner')).toHaveCount(0);
  await expect.poll(() => wordCount(a.frame)).toBe(offline);
  // A's edit after B reconnected reaches B too.
  await caretAtStart(a);
  await a.page.keyboard.insertText('Trực tuyến ');
  const online = await changed(a.frame, offline);
  await expect.poll(() => wordCount(b.frame)).toBe(online);

  // (d) BlueDrive revokes B's edit permission mid-session → B switches to view mode.
  await setPerms(fileId, 'u2', {
    view: true,
    comment: false,
    edit: false,
    share: false,
    download: true,
  });
  const n = await notifyPermissions(fileId, 'u2');
  expect(n.sessions).toBeGreaterThan(0);
  const banner = b.frame.getByTestId('hosted-banner');
  await expect(banner).toHaveAttribute('data-mode', 'view');
  await expect(banner).toHaveText(/quyền sửa .* thu hồi/);
  // B's typing no longer changes the document (locally or for A).
  await caretAtStart(b);
  await b.page.keyboard.insertText('Không được sửa ');
  await b.page.waitForTimeout(1000);
  expect(await wordCount(b.frame)).toBe(online);
  expect(await wordCount(a.frame)).toBe(online);
  // A keeps editing; B (viewer) still sees it live.
  await caretAtStart(a);
  await a.page.keyboard.insertText('Vẫn ');
  const last = await changed(a.frame, online);
  await expect.poll(() => wordCount(b.frame)).toBe(last);
  // B can no longer save (the host command reports the read-only session).
  await expect(hostCommand(b, 'save')).rejects.toThrow(/readOnly/);

  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});

test('stale offline save goes to a conflict copy instead of overwriting', async ({ request }) => {
  // The documented path for edits made without the live session (docs/bohp.md §D):
  // POST …/sync?baseVersion=<old> after the host moved on creates "(xung đột - …)".
  const fileId = await createFromCorpus('doc/phase1-sample.docx', 'Kế hoạch.docx');
  const open = async (user: string) => {
    const res = await request.post(`${urls.editor}/api/sessions`, {
      data: { host: 'bluedrive', fileId, token: await token(user, fileId) },
    });
    expect(res.status()).toBe(201);
    return (await res.json()) as {
      sessionId: string;
      sessionToken: string;
      file: { version: string };
    };
  };
  const s = await open('u2');
  const base = s.file.version;
  // Meanwhile someone saves a newer version through BlueDrive.
  await fetch(`${urls.mock}/_admin/files/${fileId}/contents`, { method: 'PUT', body: 'newer' });
  const res = await request.post(
    `${urls.editor}/api/sessions/${s.sessionId}/sync?baseVersion=${encodeURIComponent(base)}`,
    {
      headers: { 'x-session-token': s.sessionToken, 'content-type': 'application/octet-stream' },
      data: Buffer.from('offline edit'),
    },
  );
  expect(res.ok()).toBe(true);
  const body = (await res.json()) as { outcome: string; conflictCopy?: { name: string } };
  expect(body.outcome).toBe('conflict-copy');
  expect(body.conflictCopy?.name).toMatch(
    /^Kế hoạch \(xung đột - Trần Thị Bình - \d{4}-\d{2}-\d{2} \d{2}-\d{2}\)\.docx$/,
  );
  const names = (await folderFiles()).map((f) => f.name);
  expect(names).toContain(body.conflictCopy!.name);
  // The original keeps the newer version untouched.
  expect((await fileState(fileId)).versions.at(-1)!.note).toBe('External upload');
});
