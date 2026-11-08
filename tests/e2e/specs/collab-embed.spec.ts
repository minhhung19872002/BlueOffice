// Editor SDK options and events against the real server and mock BlueDrive
// (prompt §6A.2): the host's theme, brand and Back / Share / Close buttons;
// `dirty`, `share-requested` and `closed` events; `print` / `export-pdf` /
// `close` commands; `mode: 'comment'` lets reviewers comment but not edit.
import { expect, test, type FrameLocator } from '@playwright/test';
import { createFromCorpus, fileState, hostCommand, hostEvents, openAs } from '../collab/drive';

const words = (f: FrameLocator) => f.getByTestId('status-words');

async function wordCount(f: FrameLocator): Promise<number> {
  const t = (await words(f).textContent()) ?? '';
  return Number(/(\d[\d.]*)/.exec(t)?.[1]?.replace(/\./g, '') ?? NaN);
}

test('host chrome, theme, dirty / share / close events, print and export-pdf', async ({
  browser,
}) => {
  const fileId = await createFromCorpus('doc/phase1-sample.docx', 'Nhúng SDK.docx');
  const a = await openAs(browser, 'u1', fileId, { theme: 'dark', brand: 'BlueDrive' });
  const f = a.frame;
  await expect(words(f)).toBeVisible();
  // Chrome asked for by the host, in the host's theme.
  await expect(f.getByTestId('hosted-brand')).toHaveText('BlueDrive · BlueDoc');
  await expect(f.getByTestId('hosted-chrome')).toHaveAttribute('data-theme', 'dark');
  await expect(f.locator('.bo-root[data-app="doc"]')).toHaveAttribute('data-theme', 'dark');
  await expect(f.getByTestId('hosted-back')).toBeVisible();
  await expect(f.getByTestId('hosted-unsaved')).toHaveCount(0);

  // Typing makes the document dirty (marker + SDK event).
  const page1 = f.locator('.bo-page[data-page="1"]');
  await expect(page1).toHaveAttribute('data-rendered', 'true');
  const box = (await page1.boundingBox())!;
  await page1.click({ position: { x: box.width * 0.3, y: box.height * 0.12 } });
  await a.page.keyboard.press('Control+Home');
  const before = await wordCount(f);
  await a.page.keyboard.insertText('Nhúng ');
  await expect.poll(() => wordCount(f)).toBe(before + 1);
  await expect(f.getByTestId('hosted-unsaved')).toBeVisible();
  await expect
    .poll(async () => (await hostEvents(a)).filter((e) => e.event === 'dirty').at(-1)?.payload)
    .toEqual({ dirty: true });

  // Share goes to the host, which shows its own sharing UI.
  await f.getByTestId('hosted-share').click();
  await expect
    .poll(async () => (await hostEvents(a)).find((e) => e.event === 'share-requested')?.payload)
    .toEqual({ fileId });

  // export-pdf downloads a PDF; print renders one into the print frame.
  const download = a.page.waitForEvent('download');
  await expect(hostCommand(a, 'export-pdf')).resolves.toBeNull();
  expect((await download).suggestedFilename()).toBe('Nhúng SDK.pdf');
  await expect(hostCommand(a, 'print')).resolves.toBeNull();
  await expect(f.locator('iframe[title="Nhúng SDK.docx"]')).toHaveCount(1);

  // Close saves the pending change as a new host version, then reports `closed`.
  const versions = (await fileState(fileId)).versions.length;
  await f.getByTestId('hosted-close').click();
  await expect(f.getByTestId('hosted-closed')).toBeVisible();
  await expect
    .poll(async () => (await hostEvents(a)).find((e) => e.event === 'closed')?.payload)
    .toEqual({ reason: 'close', saved: true });
  expect((await fileState(fileId)).versions.length).toBe(versions + 1);
  expect((await hostEvents(a)).filter((e) => e.event === 'dirty').at(-1)?.payload).toEqual({
    dirty: false,
  });
  // Closing again does nothing more.
  await expect(hostCommand(a, 'close')).resolves.toEqual({ saved: false });
  expect(a.errors).toEqual([]);
  await a.context.close();
});

test("mode 'comment': comments run, edits do not; no chrome when the host hides it", async ({
  browser,
}) => {
  const fileId = await createFromCorpus('doc/phase1-sample.docx', 'Góp ý.docx');
  // u2 may edit the file; the host opens it for review only.
  const b = await openAs(browser, 'u2', fileId, { mode: 'comment', chrome: '0' });
  const f = b.frame;
  await expect(words(f)).toBeVisible();
  expect((await hostEvents(b)).find((e) => e.event === 'ready')?.payload).toMatchObject({
    mode: 'comment',
  });
  await expect(f.getByTestId('hosted-chrome')).toHaveCount(0);
  await expect(f.getByTestId('hosted-banner')).toHaveAttribute('data-mode', 'comment');

  const page1 = f.locator('.bo-page[data-page="1"]');
  await expect(page1).toHaveAttribute('data-rendered', 'true');
  const box = (await page1.boundingBox())!;
  await page1.click({ position: { x: box.width * 0.3, y: box.height * 0.12 } });
  const before = await wordCount(f);
  await b.page.keyboard.insertText('Không được sửa ');
  await b.page.waitForTimeout(500);
  expect(await wordCount(f)).toBe(before);

  // A comment on a selected word is allowed.
  await b.page.keyboard.press('Control+Home');
  await b.page.keyboard.press('Shift+Control+ArrowRight');
  await f.getByRole('tab', { name: 'Xem lại' }).click();
  await f.locator('[data-command="review.newComment"]').click();
  await f.getByTestId('comment-input').fill('Xem lại số liệu');
  await f.getByTestId('comment-add').click();
  await expect(f.getByTestId('comment-text')).toHaveText('Xem lại số liệu');

  // `close` from the host: nothing to save in comment mode.
  await expect(hostCommand(b, 'close')).resolves.toEqual({ saved: false });
  await expect
    .poll(async () => (await hostEvents(b)).find((e) => e.event === 'closed')?.payload)
    .toEqual({ reason: 'command', saved: false });
  expect(b.errors).toEqual([]);
  await b.context.close();
});
