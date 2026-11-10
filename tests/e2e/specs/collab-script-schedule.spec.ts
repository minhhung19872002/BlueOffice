// CORE-SCRIPT-004: a BlueScript scheduled from the pane of a hosted .docx runs on the
// server (bo-cli script run, headless QuickJS) once the file is closed, its result is
// saved to BlueDrive as a new version noted with the script name, and the run history
// shows in the pane when the file is opened again. The stack enables scheduled runs only
// with a built bo-cli (collab/stack.mjs); without one this spec is skipped.
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type FrameLocator } from '@playwright/test';
import { findBoCli } from '../collab/ports.mjs';
import { createFromCorpus, documentXml, fileState, openAs } from '../collab/drive';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');

async function openPane(frame: FrameLocator) {
  await frame.getByRole('tab', { name: 'Xem', exact: true }).click();
  await frame.locator('[data-command="script.editor"]').click();
  const pane = frame.getByTestId('script-pane');
  await expect(pane).toBeVisible();
  return pane;
}

test('a scheduled script runs on the server and saves a version', async ({ browser }) => {
  test.skip(!findBoCli(root), 'needs a built bo-cli (cargo build -p bo-cli)');
  test.setTimeout(300_000);
  const fileId = await createFromCorpus('doc/phase1-sample.docx', 'Lịch BlueScript.docx');
  const a = await openAs(browser, 'u1', fileId);
  const pane = await openPane(a.frame);
  await pane
    .getByTestId('script-code')
    .fill("await doc.appendParagraph('Dòng do lịch chạy thêm');\nconsole.log('đã chạy');");
  await pane.getByTestId('script-name').fill('Thêm dòng');
  await pane.getByTestId('script-save').click();
  await pane.getByTestId('script-cron').fill('* * * * *');
  await pane.getByTestId('script-schedule').click();
  await expect(pane.getByTestId('script-schedule-item')).toContainText('Thêm dòng');
  // Runs are put off while the file is open for editing: leave it.
  await a.context.close();

  await expect
    .poll(
      async () =>
        (await fileState(fileId)).versions.find((v) => v.note?.startsWith('BlueScript: Thêm dòng'))
          ?.version ?? null,
      { timeout: 200_000, intervals: [2_000] },
    )
    .not.toBeNull();
  const saved = (await fileState(fileId)).versions.find((v) =>
    v.note?.startsWith('BlueScript: Thêm dòng'),
  )!;
  expect(documentXml(saved)).toContain('Dòng do lịch chạy thêm');
  expect(saved.modifiedBy).toBe('u1');

  // The owner sees the run in the pane's history.
  const again = await openAs(browser, 'u1', fileId);
  const pane2 = await openPane(again.frame);
  const item = pane2.getByTestId('script-schedule-item');
  // A run due while the file is open again is put off ("hoãn").
  await expect(item).toContainText(/lần trước: (thành công|hoãn)/);
  await item.getByRole('button', { name: 'Lịch sử' }).click();
  const runs = pane2.getByRole('list', { name: 'Lịch sử chạy “Thêm dòng”' });
  await expect(runs).toContainText('thành công');
  await expect(runs).toContainText(`đã lưu phiên bản ${saved.version}`);
  await expect(runs).toContainText('[log] đã chạy');
  expect(again.errors).toEqual([]);
  await again.context.close();
});
