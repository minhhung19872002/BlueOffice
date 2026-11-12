import { expect, test, type Page } from '@playwright/test';

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function words(page: Page): Promise<number> {
  const t = (await page.getByTestId('status-words').textContent()) ?? '';
  // The document's count ("2 / 6 từ" while something is selected).
  return Number(/(\d+) từ/.exec(t)?.[1] ?? 'NaN');
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
  // Ctrl+F9 with nothing selected: empty braces in the text, the code typed between them in
  // place (no dialog); F9 then updates the field, which asks its question first.
  await page.keyboard.press('Control+F9');
  await expect(page.getByTestId('field-code-dialog')).toHaveCount(0);
  await expect(page.getByTestId('field-code-inplace')).toBeVisible();
  await expect(page.getByTestId('field-code-input')).toBeFocused();
  await page.keyboard.type('FILLIN "Tên khách?" \\d "Lan"');
  await page.keyboard.press('F9');
  await expect(page.getByTestId('field-code-inplace')).toHaveCount(0);
  await expect(page.getByTestId('field-prompt-dialog')).toBeVisible();
  await expect(page.getByTestId('field-prompt-input')).toHaveValue('Lan');
  await page.getByTestId('field-prompt-ok').click();
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  await page.keyboard.insertText(' và ');
  // Ctrl+F9 over typed text: the selection becomes the code (= 6*7 in words: "bốn mươi hai").
  const code = '= 6*7 \\* CardText';
  await page.keyboard.insertText(code);
  for (let i = 0; i < code.length; i++) await page.keyboard.press('Shift+ArrowLeft');
  await page.keyboard.press('Control+F9');
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');
  // F9 at the caret updates only the field there (no question asked)…
  await page.keyboard.press('F9');
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');
  await expect(page.getByTestId('field-prompt-dialog')).toHaveCount(0);
  // …with everything selected, all of them: the FILLIN question, offering the default answer.
  await page.keyboard.press('Control+A');
  await page.keyboard.press('F9');
  await expect(page.getByTestId('field-prompt-dialog')).toBeVisible();
  await expect(page.getByTestId('field-prompt-text')).toHaveText('Tên khách?');
  await expect(page.getByTestId('field-prompt-input')).toHaveValue('Lan');
  await page.getByTestId('field-prompt-input').fill('Minh Anh');
  await page.getByTestId('field-prompt-ok').click();
  await expect(page.getByTestId('field-prompt-dialog')).toHaveCount(0);
  // Everything is still selected.
  await expect(page.getByTestId('status-words')).toHaveText('6 / 6 từ');
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

test('Shift+F9 shows the code of the field at the caret only', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  // Two formula fields made with Ctrl+F9 over their typed codes: results "42" and "5", codes
  // much wider. The line's width (the caret at its end) tells what is shown.
  const field = async (code: string) => {
    await page.keyboard.insertText(code);
    for (let i = 0; i < code.length; i++) await page.keyboard.press('Shift+ArrowLeft');
    await page.keyboard.press('Control+F9');
    await page.keyboard.press('End');
  };
  await field('= 6*7 + 0');
  await page.keyboard.insertText(' và ');
  await field('= 2+3 + 0');
  await page.keyboard.insertText(' nữa');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  const lineEnd = async () => {
    await page.keyboard.press('End');
    const box = await page.getByTestId('caret').boundingBox();
    if (!box) throw new Error('no caret');
    return box.x;
  };
  /** The caret just after the second field (before " nữa"). */
  const atSecond = async () => {
    await page.keyboard.press('End');
    for (let i = 0; i < ' nữa'.length; i++) await page.keyboard.press('ArrowLeft');
  };
  const results = await lineEnd();
  // The caret at the first field: Shift+F9 shows its code only.
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('Shift+F9');
  await expect.poll(lineEnd).toBeGreaterThan(results + 15);
  const one = await lineEnd();
  // The caret at the second field: Shift+F9 shows its code too — wider still.
  await atSecond();
  await page.keyboard.press('Shift+F9');
  await expect.poll(lineEnd).toBeGreaterThan(one + 15);
  // Shift+F9 again on each field: the results come back one at a time.
  await atSecond();
  await page.keyboard.press('Shift+F9');
  await expect.poll(lineEnd).toBeLessThan(one + 2);
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('Shift+F9');
  await expect.poll(lineEnd).toBeLessThan(results + 2);
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
  // "−" deletes the item at the caret; the last item of a section stays.
  await page.getByTestId('control-delete-item').click();
  await expect.poll(() => words(page)).toBe(before);
  await page.getByTestId('control-delete-item').click();
  await expect.poll(() => words(page)).toBe(before);
  // A drop-down list: Alt+Down goes to its list.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('dropDown');
  await page.getByTestId('control-items').fill('Hà Nội\nHuế');
  await page.getByTestId('control-ok').click();
  // The list opens on the control itself (its arrow at the right end of the frame).
  await expect(page.getByTestId('control-drop')).toBeVisible();
  await page.keyboard.press('Alt+ArrowDown');
  await expect(page.getByTestId('control-list')).toBeFocused();
  const listed = await words(page);
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  // "Chọn một mục." (3 words) became "Huế".
  await expect.poll(() => words(page)).toBe(listed - 2);
  await expect(page.getByTestId('control-list')).toHaveCount(0);
  expect(errors).toEqual([]);
});

async function insertControl(page: Page, kind: string) {
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption(kind);
  await page.getByTestId('control-ok').click();
}

test('date picker calendar and gallery on the control, properties, temporary, picture', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  // A date picker: its calendar opens from the button on the control; a day is clicked.
  await insertControl(page, 'date');
  const placeholder = await words(page);
  await page.getByTestId('control-drop').click();
  await expect(page.getByTestId('control-calendar')).toBeVisible();
  const day = page.locator('[data-testid="control-calendar"] td[data-date$="-15"]');
  const iso = (await day.getAttribute('data-date')) ?? '';
  await day.click();
  await expect(page.getByTestId('control-calendar')).toHaveCount(0);
  // "Bấm vào đây để nhập ngày." became one date, dd/MM/yyyy.
  await expect.poll(() => words(page)).toBe(placeholder - 5);
  const [y, m] = iso.split('-');
  await expect(page.getByTestId('doc-a11y-mirror')).toContainText(`15/${m}/${y}`);
  // From the keyboard: Alt+Down, → (the 16th), Enter.
  await page.keyboard.press('Alt+ArrowDown');
  await expect(page.getByTestId('control-calendar')).toBeVisible();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('doc-a11y-mirror')).toContainText(`16/${m}/${y}`);
  // Properties: English long dates.
  await page.getByTestId('control-properties').click();
  await page.getByTestId('control-props-lid').selectOption('en-US');
  await page.getByTestId('control-props-date-format').fill('MMMM d, yyyy');
  await page.getByTestId('control-props-ok').click();
  await expect(page.getByTestId('doc-a11y-mirror')).toContainText(`16, ${y}`);

  // A drop-down list whose items are edited in its properties: added, moved up.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('dropDown');
  await page.getByTestId('control-items').fill('Huế');
  await page.getByTestId('control-ok').click();
  await page.getByTestId('control-properties').click();
  await page.getByTestId('control-props-item-display').fill('Hà Nội');
  await page.getByTestId('control-props-item-value').fill('HN');
  await page.getByTestId('control-props-item-add').click();
  await page.getByTestId('control-props-item-up').click();
  await expect(page.getByTestId('control-props-items').locator('option')).toHaveText([
    'Hà Nội (HN)',
    'Huế',
  ]);
  // Shown with start / end tags in a colour instead of the box.
  await page.getByTestId('control-props-appearance').selectOption('tags');
  await page.getByTestId('control-props-ok').click();
  await expect(page.getByTestId('control-frame')).toHaveCount(0);
  await expect(page.getByTestId('control-tag-start')).toBeVisible();
  await expect(page.getByTestId('control-tag-end')).toBeVisible();
  await page.getByTestId('control-drop').click();
  await expect(page.getByTestId('control-list').locator('li')).toHaveText(['Hà Nội', 'Huế']);
  await page.getByTestId('control-list').locator('li').first().click();
  await expect(page.getByTestId('doc-a11y-mirror')).toContainText('Hà Nội');
  // Show as none: no frame, no tags.
  await page.getByTestId('control-properties').click();
  await page.getByTestId('control-props-appearance').selectOption('hidden');
  await page.getByTestId('control-props-ok').click();
  await expect(page.getByTestId('control-tag-start')).toHaveCount(0);
  await expect(page.getByTestId('control-frame')).toHaveCount(0);

  // "Remove content control when contents are edited": typing over the placeholder leaves
  // plain text.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  await insertControl(page, 'text');
  await page.getByTestId('control-properties').click();
  await page.getByTestId('control-props-temporary').check();
  await page.getByTestId('control-props-ok').click();
  await expect(page.getByTestId('control-bar')).toBeVisible();
  // The placeholder is still selected: typing replaces it.
  await page.keyboard.insertText('Lan');
  await expect(page.getByTestId('control-bar')).toHaveCount(0);

  // A picture control shows a placeholder picture (no text added).
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ');
  const before = await words(page);
  await insertControl(page, 'picture');
  await expect(page.getByTestId('control-kind-label')).toHaveText('Ảnh');
  await expect(page.getByTestId('control-picture')).toBeVisible();
  // No placeholder text: the placeholder is a picture (at most one object counted).
  await expect(page.getByTestId('doc-a11y-mirror')).not.toContainText('Bấm vào đây để chèn ảnh');
  expect(await words(page)).toBeLessThanOrEqual(before + 1);

  // A building block gallery: its list offers the Quick Parts blocks; one replaces the
  // placeholder.
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await insertControl(page, 'buildingBlockGallery');
  await expect(page.getByTestId('control-kind-label')).toHaveText('Thư viện khối dựng sẵn');
  const galleryWords = await words(page);
  await page.getByTestId('control-drop').click();
  const first = page.getByTestId('control-list').locator('li').first();
  await expect(first).toBeVisible();
  await first.click();
  await expect.poll(() => words(page)).not.toBe(galleryWords);
  expect(errors).toEqual([]);
});
