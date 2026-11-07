// BlueSheet and BlueSlide live collaboration against the real server and mock
// BlueDrive: a cell formula typed by one user reaches the other, and a shape
// moved on one screen moves on the other (with the mover's selection shown).
import { expect, test, type FrameLocator, type Page } from '@playwright/test';
import { createFromCorpus, fileState, hostCommand, openAs, zipEntry } from '../collab/drive';

async function goTo(page: Page, frame: FrameLocator, ref: string) {
  const box = frame.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(frame.getByLabel('Ô đang nhập')).toBeFocused();
  void page;
}

test('two users edit one .xlsx: cell formulas sync live and save', async ({ browser }) => {
  const fileId = await createFromCorpus('sheet/sst-1.xlsx', 'Bảng cộng tác.xlsx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  for (const p of [a, b]) {
    await expect(p.frame.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  }

  // A types a formula; B sees it in the formula bar.
  await goTo(a.page, a.frame, 'H20');
  await a.page.keyboard.type('=2*21');
  await a.page.keyboard.press('Enter');
  await goTo(b.page, b.frame, 'H20');
  await expect(b.frame.getByLabel('Thanh công thức')).toHaveValue('=2*21');

  // B answers with Vietnamese text; A sees it.
  await goTo(b.page, b.frame, 'H21');
  await b.page.keyboard.type('Xin chào Việt Nam');
  await b.page.keyboard.press('Enter');
  await goTo(a.page, a.frame, 'H21');
  await expect(a.frame.getByLabel('Thanh công thức')).toHaveValue('Xin chào Việt Nam');
  // Re-select H20 on B: the formula bar still shows the formula (not a value).
  await goTo(b.page, b.frame, 'H20');
  await expect(b.frame.getByLabel('Thanh công thức')).toHaveValue('=2*21');

  // Saving from the host writes one version with both editors.
  await expect(hostCommand(b, 'save')).resolves.toEqual({ saved: true });
  await expect.poll(async () => (await fileState(fileId)).versions.length).toBe(2);
  const v = (await fileState(fileId)).versions.at(-1)!;
  expect(v.editors.sort()).toEqual(['u1', 'u2']);
  const sheet = zipEntry(Buffer.from(v.content, 'base64'), 'xl/worksheets/sheet1.xml')!.toString();
  // ECMA-376 Part 1, §18.3.1.40 f (cell formula).
  expect(sheet).toMatch(/<c r="H20"[^>]*>\s*<f>2\*21<\/f>/);

  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});

test('two users edit one .pptx: a moved shape moves for the other user', async ({ browser }) => {
  const fileId = await createFromCorpus('slide/slide-text-bullets.pptx', 'Trình chiếu.pptx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  for (const p of [a, b]) {
    await expect(p.frame.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
    await expect(p.frame.getByTestId('slide-overlay')).toBeVisible();
  }

  // A selects the title placeholder; B sees A's selection.
  const overlay = a.frame.getByTestId('slide-overlay');
  const o = (await overlay.boundingBox())!;
  await a.page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3);
  await expect(a.frame.getByTestId('selection-box')).toHaveCount(1);
  const peer = b.frame.getByTestId('peer-selection');
  await expect(peer).toHaveCount(1);
  const before = (await peer.boundingBox())!;

  // A drags it down-right; on B the shape (and A's selection frame) moves too.
  const sel = (await a.frame.getByTestId('selection-box').first().boundingBox())!;
  const sx = sel.x + sel.width / 2;
  const sy = sel.y + sel.height / 2;
  await a.page.mouse.move(sx, sy);
  await a.page.mouse.down();
  await a.page.mouse.move(sx + 40, sy + 30, { steps: 4 });
  await a.page.mouse.move(sx + 60, sy + 70, { steps: 4 });
  await a.page.mouse.up();
  await expect(a.frame.getByTestId('status-modified')).toBeVisible();
  await expect
    .poll(async () => {
      const now = (await peer.boundingBox())!;
      return [Math.round(now.x - before.x) > 20, Math.round(now.y - before.y) > 20];
    })
    .toEqual([true, true]);

  // B selects the moved shape at its new place.
  const moved = (await peer.boundingBox())!;
  await b.page.mouse.click(moved.x + moved.width / 2, moved.y + moved.height / 2);
  await expect(b.frame.getByTestId('selection-box')).toHaveCount(1);
  // Same window size: B's selection outline sits where A's is.
  const bSel = (await b.frame.getByTestId('selection-box').first().boundingBox())!;
  const aSel = (await a.frame.getByTestId('selection-box').first().boundingBox())!;
  for (const k of ['x', 'y', 'width', 'height'] as const) {
    expect(Math.abs(bSel[k] - aSel[k])).toBeLessThan(2);
  }

  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});
