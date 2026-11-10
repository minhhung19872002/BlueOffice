import { expect, test, type Page } from '@playwright/test';

// CORE-SCRIPT-001: BlueScript pane — a script runs in the QuickJS worker, edits the
// document through the engine, is one undo step, and a runaway script is stopped.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function runScript(page: Page, code: string) {
  const pane = page.getByTestId('script-pane');
  await pane.getByTestId('script-code').fill(code);
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-run')).toBeEnabled({ timeout: 20_000 });
  return pane.getByTestId('script-console');
}

test('BlueDoc: a script edits the document as one undo step', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.editor').click();
  await expect(page.getByTestId('script-pane')).toBeVisible();

  let out = await runScript(
    page,
    `await doc.appendParagraph('Báo cáo', 'Heading1');
await doc.appendParagraph('Nội dung từ BlueScript');
console.log('paras=' + (await doc.paragraphs).length);
console.log('fetch=' + typeof fetch + ' postMessage=' + typeof postMessage);`,
  );
  await expect(out).toContainText('paras=2');
  await expect(out).toContainText('fetch=undefined postMessage=undefined');
  await expect(out).toContainText('Xong');

  // One Ctrl+Z takes the whole run back.
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Control+z');
  out = await runScript(page, `console.log('text=[' + (await doc.text) + ']');`);
  await expect(out).toContainText('text=[]');

  // A runaway script is stopped with the Stop button.
  const pane = page.getByTestId('script-pane');
  await pane.getByTestId('script-code').fill('while (true) {}');
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-stop')).toBeEnabled();
  await pane.getByTestId('script-stop').click();
  await expect(pane.getByTestId('script-console')).toContainText('Đã dừng.');
  await expect(pane.getByTestId('script-run')).toBeEnabled();
  expect(errors).toEqual([]);
});

test('BlueSheet: a script writes and reads cells', async ({ page }) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.editor').click();
  const out = await runScript(
    page,
    `await sheet.range('A1:B2').setValues([['Tên', 'Điểm'], ['An', 9]]);
await sheet.range('B3').setFormulas([['=B2*2']]);
console.log(JSON.stringify(await sheet.range('A1:B3').values));`,
  );
  await expect(out).toContainText('[["Tên","Điểm"],["An",9],[null,18]]');
});
