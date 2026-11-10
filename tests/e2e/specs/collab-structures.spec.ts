// Live collaboration of the structures beyond plain body text / cell inputs,
// against the real server and mock BlueDrive: BlueDoc table cells edited by
// two users at once, a BlueSheet row inserted by one user while the other's
// formula points below it, and BlueSlide speaker notes.
import { expect, test, type FrameLocator, type Page } from '@playwright/test';
import {
  createFromCorpus,
  documentXml,
  fileState,
  hostCommand,
  openAs,
  type Participant,
} from '../collab/drive';

async function words(f: FrameLocator): Promise<number> {
  const t = (await f.getByTestId('status-words').textContent()) ?? '';
  // With a selection the bar shows "selected / total" (CORE-UI-007): the total is the last number.
  return Number(t.match(/\d+/g)?.at(-1) ?? NaN);
}

/** Selects the first match of `text` through the find bar and closes it. */
async function findText(p: Participant, text: string) {
  await p.page.keyboard.press('Control+f');
  const q = p.frame.getByTestId('find-query');
  await q.fill(text);
  await q.press('Enter');
  await expect(p.frame.getByTestId('find-message')).toHaveText(/kết quả/);
  await q.press('Escape');
}

/** Clicks a ribbon command, opening the collapsed group that holds it when the ribbon is narrow. */
async function runCommand(f: FrameLocator, id: string) {
  const button = f.locator(`[data-command="${id}"]:visible`).first();
  if (await button.count()) {
    await button.click();
    return;
  }
  const groups = f.locator('[data-group][data-level="collapsed"]');
  for (let i = 0; i < (await groups.count()); i++) {
    await groups.nth(i).getByRole('button').first().click();
    if (await button.count()) {
      await button.click();
      return;
    }
    await groups.nth(i).getByRole('button').first().click();
  }
  throw new Error(`command ${id} not found in the ribbon`);
}

async function clickPage(p: Participant) {
  const page1 = p.frame.locator('.bo-page[data-page="1"]');
  await expect(page1).toHaveAttribute('data-rendered', 'true');
  const box = (await page1.boundingBox())!;
  await page1.click({ position: { x: box.width * 0.3, y: box.height * 0.05 } });
}

test('two users type in different cells of one table at the same time', async ({ browser }) => {
  const fileId = await createFromCorpus('doc/tables-02-merge.docx', 'Bảng gộp.docx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  for (const p of [a, b]) {
    await expect(p.frame.getByTestId('status-words')).toBeVisible();
    await clickPage(p);
  }
  // Each puts the caret at the end of a different cell, then both type.
  await findText(a, 'Gộp ngang');
  await a.page.keyboard.press('End');
  await findText(b, 'Gộp dọc');
  await b.page.keyboard.press('End');
  await Promise.all([a.page.keyboard.type(' của An'), b.page.keyboard.type(' của Bình')]);
  // Both edits survive on both sides (cells merge separately): same word
  // count everywhere, and the saved version has both cells' new text.
  await expect.poll(() => words(a.frame)).toBe(13);
  await expect.poll(() => words(b.frame)).toBe(13);
  await expect(hostCommand(b, 'save')).resolves.toEqual({ saved: true });
  await expect.poll(async () => (await fileState(fileId)).versions.length).toBe(2);
  const xml = documentXml((await fileState(fileId)).versions.at(-1)!);
  // ECMA-376 Part 1, §17.4.66 tc: the text of each table cell.
  const cells = [...xml.matchAll(/<w:tc>[\s\S]*?<\/w:tc>/g)].map((m) =>
    [...m[0].matchAll(/<w:t(?: [^>]*)?>([^<]*)<\/w:t>/g)].map((t) => t[1]).join(''),
  );
  expect(cells).toContain('Gộp ngang của An');
  expect(cells).toContain('Gộp dọc của Bình');
  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});

async function goTo(frame: FrameLocator, ref: string) {
  const box = frame.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(frame.getByLabel('Ô đang nhập')).toBeFocused();
}

async function typeIn(page: Page, frame: FrameLocator, ref: string, text: string) {
  await goTo(frame, ref);
  await page.keyboard.type(text);
  await page.keyboard.press('Enter');
}

test('a row inserted by one user moves the other user’s cells and references', async ({
  browser,
}) => {
  const fileId = await createFromCorpus('sheet/sst-1.xlsx', 'Bảng chèn hàng.xlsx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  for (const p of [a, b]) {
    await expect(p.frame.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  }
  await typeIn(a.page, a.frame, 'J40', '5');
  await typeIn(a.page, a.frame, 'K2', '=J40*2');
  await goTo(b.frame, 'K2');
  await expect(b.frame.getByLabel('Thanh công thức')).toHaveValue('=J40*2');
  // B inserts a row above row 40.
  await goTo(b.frame, 'J40');
  await runCommand(b.frame, 'cells.insertRow');
  // A sees the value one row lower and its formula following it.
  await goTo(a.frame, 'J41');
  await expect(a.frame.getByLabel('Thanh công thức')).toHaveValue('5');
  await goTo(a.frame, 'K2');
  await expect(a.frame.getByLabel('Thanh công thức')).toHaveValue('=J41*2');
  // A bolds a cell, B types into it: value and format both stay.
  await goTo(a.frame, 'L3');
  await a.page.keyboard.press('Control+b');
  await typeIn(b.page, b.frame, 'L3', 'đậm');
  await goTo(a.frame, 'L3');
  await expect(a.frame.getByLabel('Thanh công thức')).toHaveValue('đậm');
  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});

test('speaker notes typed by one user reach the other', async ({ browser }) => {
  const fileId = await createFromCorpus('slide/slide-notes-basic.pptx', 'Ghi chú.pptx');
  const a = await openAs(browser, 'u1', fileId);
  const b = await openAs(browser, 'u2', fileId);
  for (const p of [a, b]) {
    await expect(p.frame.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  }
  const notesA = a.frame.getByTestId('notes-editor');
  const notesB = b.frame.getByTestId('notes-editor');
  await expect(notesB).toHaveValue(/\S/);
  const before = await notesB.inputValue();
  await notesA.click();
  await a.page.keyboard.press('Control+End');
  await a.page.keyboard.type(' — thêm bởi An');
  await notesA.blur();
  await expect(notesB).toHaveValue(`${before} — thêm bởi An`);
  expect(a.errors).toEqual([]);
  expect(b.errors).toEqual([]);
  await a.context.close();
  await b.context.close();
});
