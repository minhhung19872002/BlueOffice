import { expect, test, type Page } from '@playwright/test';

// CORE-SCRIPT-003: "Convert macro VBA to BlueScript" — pasted VBA becomes a BlueScript
// draft (a new unsaved script) with the list of what could not be converted. The draft
// is not run until the user presses Run; VBA itself is never executed.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

const VBA = `Sub Fill()
    Dim i As Long, total As Long
    Application.ScreenUpdating = False
    For i = 1 To 3
        Cells(i, 1).Value = i * 10
        total = total + Cells(i, 1).Value
    Next i
    Range("B1").Value = "Tổng " & total
    Range("B1").Select
    MsgBox "Xong: " & total
End Sub`;

test('BlueSheet: macro VBA is converted to a draft that the user reviews and runs', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.convertVba').click();

  const pane = page.getByTestId('script-pane');
  await expect(pane).toBeVisible();
  await pane.getByTestId('script-vba-source').fill(VBA);
  await pane.getByTestId('script-vba-convert').click();

  // The draft is in the editor as a new unsaved script, with its warnings listed.
  const code = pane.getByTestId('script-code');
  await expect(code).toHaveValue(/await sheet\.cell\(i - 1, 0\)\.setValue\(i \* 10\);/);
  await expect(code).toHaveValue(/\/\/ TODO\(VBA\): Range\("B1"\)\.Select/);
  await expect(pane.getByTestId('script-list')).toHaveValue('');
  await expect(pane.getByTestId('script-name')).toHaveValue('Bản nháp từ macro VBA');
  const warnings = pane.getByTestId('script-vba-warnings').locator('li');
  await expect(warnings).toHaveCount(2);
  await expect(warnings.nth(0)).toContainText('Dòng 3: Application.ScreenUpdating');
  await expect(warnings.nth(1)).toContainText('Dòng 9: range.Select');

  // Nothing ran: the cells are still empty.
  const out = pane.getByTestId('script-console');
  await expect(out).toContainText('Bản nháp chưa được chạy');
  await expect(out).not.toContainText('Xong: 60');

  // The user runs the draft: it writes the cells through the engine.
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-run')).toBeEnabled({ timeout: 20_000 });
  await expect(out).toContainText('Xong: 60');
  await code.fill(`console.log(JSON.stringify(await sheet.range('A1:B3').values));`);
  await pane.getByTestId('script-run').click();
  await expect(out).toContainText('[[10,"Tổng 60"],[20,null],[30,null]]');
  expect(errors).toEqual([]);
});

test('BlueDoc: the command opens the converter and a document macro is drafted', async ({
  page,
}) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.convertVba').click();
  const pane = page.getByTestId('script-pane');
  await pane
    .getByTestId('script-vba-source')
    .fill('Sub Hello()\n  Selection.TypeText "Xin chào"\n  Selection.Font.Bold = True\nEnd Sub');
  await pane.getByTestId('script-vba-convert').click();
  await expect(pane.getByTestId('script-code')).toHaveValue(
    /await doc\.insertText\("Xin chào"\);\s+await doc\.format\(\{ bold: true \}\);/,
  );
  await expect(pane.getByTestId('script-vba-report')).toContainText('Đã chuyển được toàn bộ');
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-run')).toBeEnabled({ timeout: 20_000 });
  await pane.getByTestId('script-code').fill(`console.log('text=[' + (await doc.text) + ']');`);
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-console')).toContainText('text=[Xin chào]');
});
