import { expect, test, type Page } from '@playwright/test';

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function words(page: Page): Promise<number> {
  const t = (await page.getByTestId('status-words').textContent()) ?? '';
  return Number(/\d+/.exec(t)?.[0] ?? 'NaN');
}

/** Click the middle of the frame of the content control around the caret. */
async function clickInFrame(page: Page, dx = 0.5) {
  const box = await page.getByTestId('control-frame').first().boundingBox();
  if (!box) throw new Error('no control frame');
  await page.mouse.click(box.x + box.width * dx, box.y + box.height / 2);
}

test('Ctrl+F9, F11 / Shift+F11 and the questions of ASK / FILLIN fields on F9', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  // Ctrl+F9 with nothing selected: the code is typed between the braces.
  await page.keyboard.press('Control+F9');
  await expect(page.getByTestId('field-code-dialog')).toBeVisible();
  await page.getByTestId('field-code-input').fill('FILLIN "Tên khách?" \\d "Lan"');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('field-code-dialog')).toHaveCount(0);
  // The default answer shows until the field asks.
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  await page.keyboard.insertText(' và ');
  // Ctrl+F9 over typed text: the selection becomes the code (= 6*7 in words: "bốn mươi hai").
  const code = '= 6*7 \\* CardText';
  await page.keyboard.insertText(code);
  for (let i = 0; i < code.length; i++) await page.keyboard.press('Shift+ArrowLeft');
  await page.keyboard.press('Control+F9');
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');
  // F9 asks the FILLIN question, offering the default answer.
  await page.keyboard.press('F9');
  await expect(page.getByTestId('field-prompt-dialog')).toBeVisible();
  await expect(page.getByTestId('field-prompt-text')).toHaveText('Tên khách?');
  await expect(page.getByTestId('field-prompt-input')).toHaveValue('Lan');
  await page.getByTestId('field-prompt-input').fill('Minh Anh');
  await page.getByTestId('field-prompt-ok').click();
  await expect(page.getByTestId('field-prompt-dialog')).toHaveCount(0);
  await expect(page.getByTestId('status-words')).toHaveText('6 từ');
  // F11 from the start selects the first field: typing replaces it.
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('F11');
  await page.keyboard.insertText('Khách');
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');
  // Shift+F11 from the end selects the last field.
  await page.keyboard.press('Control+End');
  await page.keyboard.press('Shift+F11');
  await page.keyboard.insertText('42');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  // No field left: F11 selects nothing, typing just adds text.
  await page.keyboard.press('F11');
  await page.keyboard.insertText('!');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  expect(errors).toEqual([]);
});

test('content controls: placeholder click, check box by Space and mouse, repeating "+" and properties', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  // A plain text control: its frame shows around it; a click in the placeholder selects it all.
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-ok').click();
  await expect(page.getByTestId('control-frame')).toBeVisible();
  await page.keyboard.press('End');
  await clickInFrame(page, 0.4);
  await page.keyboard.insertText('Lan');
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  // Properties: the title shows in the frame's tab.
  await page.getByTestId('control-properties').click();
  await expect(page.getByTestId('control-props-dialog')).toBeVisible();
  await page.getByTestId('control-props-title').fill('Họ tên');
  await page.getByTestId('control-props-ok').click();
  await expect(page.getByTestId('control-frame-title')).toHaveText('Họ tên');
  await expect(page.getByTestId('control-kind-label')).toHaveText('Văn bản thuần · Họ tên');
  // A check box: Space toggles it while selected, a click on its mark toggles it back.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('checkbox');
  await page.getByTestId('control-ok').click();
  await expect(page.getByTestId('control-toggle')).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Space');
  await expect(page.getByTestId('control-toggle')).toHaveAttribute('aria-pressed', 'true');
  await clickInFrame(page);
  await expect(page.getByTestId('control-toggle')).toHaveAttribute('aria-pressed', 'false');
  // A repeating section around a new paragraph: "+" adds a copy of its item.
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Thành viên');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('repeatingSection');
  await page.getByTestId('control-placeholder').fill('Danh sách');
  await page.getByTestId('control-ok').click();
  await expect(page.getByTestId('control-kind-label')).toHaveText('Phần lặp lại · Danh sách');
  await expect(page.getByTestId('control-frame-title')).toHaveText('Danh sách');
  const before = await words(page);
  await page.getByTestId('control-add-item').click();
  await expect.poll(() => words(page)).toBe(before + 2);
  // The new item is in the same section: "+" again adds a third.
  await expect(page.getByTestId('control-add-item')).toBeVisible();
  await page.getByTestId('control-add-item').click();
  await expect.poll(() => words(page)).toBe(before + 4);
  // Undo removes one item at a time.
  await page.keyboard.press('Control+Z');
  await expect.poll(() => words(page)).toBe(before + 2);
  // A drop-down list: Alt+Down goes to its list.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('dropDown');
  await page.getByTestId('control-items').fill('Hà Nội\nHuế');
  await page.getByTestId('control-ok').click();
  await page.keyboard.press('Alt+ArrowDown');
  await expect(page.getByTestId('control-choose')).toBeFocused();
  const listed = await words(page);
  await page.getByTestId('control-choose').selectOption('Huế');
  // "Chọn một mục." (3 words) became "Huế".
  await expect.poll(() => words(page)).toBe(listed - 2);
  // Close the list the picker opened.
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});
