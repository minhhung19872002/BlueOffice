import { expect, test, type Page } from '@playwright/test';

// CORE-SCRIPT-002: record typing + bold in BlueDoc, stop, save the script with a Quick Access
// Toolbar button, then run it on a new document → same text and formatting.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function runScript(page: Page, code: string) {
  const pane = page.getByTestId('script-pane');
  await pane.getByTestId('script-code').fill(code);
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-run')).toBeEnabled({ timeout: 20_000 });
  return pane.getByTestId('script-console');
}

const CHECK = `const text = await doc.text;
await doc.select(0, 0, 1);
const plain = (await doc.command({ cmd: 'state' })).format.bold;
await doc.select(0, 9, 10);
const bold = (await doc.command({ cmd: 'state' })).format.bold;
console.log('check=[' + text + '] ' + plain + '/' + bold);`;

test('BlueDoc: a recorded macro replays on a new document', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.editor').click();
  const pane = page.getByTestId('script-pane');
  await expect(pane).toBeVisible();

  // Record: type, Ctrl+B, type.
  await pane.getByTestId('script-record').click();
  await expect(pane.getByTestId('script-recording')).toBeVisible();
  await expect(cmd(page, 'script.record')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('document-view').click();
  await page.keyboard.type('Xin chào ');
  await page.keyboard.press('Control+b');
  await page.keyboard.type('BlueScript');
  await pane.getByTestId('script-record').click();
  await expect(pane.getByTestId('script-recording')).toHaveCount(0);
  const code = pane.getByTestId('script-code');
  await expect(code).toHaveValue(/await doc\.insertText\("Xin chào "\);/);
  await expect(code).toHaveValue(/await doc\.format\(\{"bold":true\}\);/);
  await expect(code).toHaveValue(/await doc\.insertText\("BlueScript"\);/);

  // Save it with a toolbar button and a shortcut.
  await pane.getByTestId('script-name').fill('Lời chào');
  await pane.getByTestId('script-save').click();
  await pane.getByTestId('script-qat').check();
  await pane.getByTestId('script-shortcut').press('Control+b');
  await expect(pane.getByTestId('script-bind-error')).toBeVisible();
  await pane.getByTestId('script-shortcut').press('Control+Alt+j');
  await expect(pane.getByTestId('script-shortcut')).toHaveValue('Ctrl+Alt+J');
  const qat = page.locator('[data-command^="qat:script.run."]');
  await expect(qat).toHaveCount(1);

  // The recording reproduces the document.
  let out = await runScript(page, CHECK);
  await expect(out).toContainText('check=[Xin chào BlueScript] false/true');

  // New document; the toolbar button runs the saved script.
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Control+n');
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');
  await expect(qat).toBeEnabled();
  const clear = pane.getByRole('button', { name: 'Xóa kết quả' });
  await clear.click();
  out = await runScript(page, CHECK);
  await expect(out).toContainText('check=[]');
  await qat.click();
  await expect
    .poll(async () => {
      await clear.click();
      out = await runScript(page, CHECK);
      return out.textContent();
    })
    .toContain('check=[Xin chào BlueScript] false/true');

  // A third document: the shortcut runs it too.
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Control+n');
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Control+Alt+j');
  await expect
    .poll(async () => {
      await clear.click();
      out = await runScript(page, CHECK);
      return out.textContent();
    })
    .toContain('check=[Xin chào BlueScript] false/true');
  expect(errors).toEqual([]);
});
