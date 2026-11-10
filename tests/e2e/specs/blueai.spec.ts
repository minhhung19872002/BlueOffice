import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-AI-001: the BlueAI task pane. The BlueOffice server's /api/ai endpoints are faked by
// routing (as translate.spec does for /api/translate); without routing the static preview has
// no such endpoint, which is the default "BlueAI off" state.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const thumbs = (page: Page) => page.locator('[data-testid^="thumb-"]');

interface Sent {
  task: string;
  text: string;
  instruction: string;
  tone?: string;
  language?: string;
}

/** A fake BlueOffice server with BlueAI enabled; `answer` decides each completion. */
async function fakeAi(page: Page, sent: Sent[], answer: (req: Sent) => string, max = 2000) {
  await page.route('**/api/ai/config**', (route) =>
    route.fulfill({
      json: { enabled: true, host: 'llm.noi-bo.example', maxInputChars: max },
    }),
  );
  await page.route('**/api/ai/complete', async (route) => {
    const body = route.request().postDataJSON() as Sent;
    sent.push(body);
    await route.fulfill({ json: { text: answer(body) } });
  });
}

/** Counts requests to the completion endpoint (none may happen while BlueAI is off). */
function watchCompletions(page: Page) {
  const seen: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/api/ai/complete')) seen.push(r.url());
  });
  return seen;
}

async function openPane(page: Page) {
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'ai.pane').click();
  const pane = page.getByTestId('ai-pane');
  await expect(pane).toBeVisible();
  return pane;
}

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function openDeck(page: Page) {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

test('BlueDoc: BlueAI is off by default — features disabled with an explanation, nothing sent', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const completions = watchCompletions(page);
  await newDocument(page);
  await page.keyboard.insertText('Nội dung mật của hợp đồng');
  await page.keyboard.press('Control+a');
  const pane = await openPane(page);
  await expect(pane).toHaveAttribute('data-state', 'off');
  await expect(pane.getByTestId('ai-off')).toContainText('BlueAI đang tắt');
  for (const id of ['rewrite', 'summarize', 'draft']) {
    const feature = pane.getByTestId(`ai-feature-${id}`);
    await expect(feature).toBeDisabled();
    await expect(feature).toHaveAttribute('title', /BlueAI đang tắt/);
  }
  await expect(pane.getByTestId('ai-run')).toHaveCount(0);
  await expect(pane.getByTestId('ai-consent')).toHaveCount(0);
  // The pane closes from the task pane host.
  await page.getByTestId('ai-close').click();
  await expect(pane).toHaveCount(0);
  expect(completions).toEqual([]);
  expect(errors).toEqual([]);
});

test('BlueSlide: slides from an outline work without AI, as one undo step', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const completions = watchCompletions(page);
  // An explicit "not enabled" answer of a BlueOffice server.
  await page.route('**/api/ai/config**', (route) =>
    route.fulfill({ status: 404, json: { code: 'unavailable' } }),
  );
  await openDeck(page);
  const n = await thumbs(page).count();
  const pane = await openPane(page);
  await expect(pane).toHaveAttribute('data-state', 'off');
  await expect(pane.getByTestId('ai-feature-slidesFromTopic')).toBeDisabled();
  await expect(pane.getByTestId('ai-feature-slidesFromTopic')).toHaveAttribute(
    'title',
    /BlueAI đang tắt/,
  );
  await expect(pane.getByTestId('ai-feature-slidesFromOutline')).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await pane
    .getByTestId('ai-prompt')
    .fill(
      'Kế hoạch quý IV\n  Mục tiêu doanh thu\n    Miền Bắc\n  Nhân sự\nKết luận\n- Việc cần làm',
    );
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-preview')).toHaveText(
    'Kế hoạch quý IV\n  - Mục tiêu doanh thu\n    - Miền Bắc\n  - Nhân sự\nKết luận\n  - Việc cần làm',
  );
  // A preview only: the deck is unchanged until the user confirms.
  await expect(thumbs(page)).toHaveCount(n);
  await pane.getByTestId('ai-apply-createSlides').click();
  await expect(pane.getByTestId('ai-notice')).toHaveText(
    'Đã thêm 2 trang chiếu vào cuối bản trình chiếu.',
  );
  await expect(thumbs(page)).toHaveCount(n + 2);
  // The first new slide is shown.
  await expect(page.getByTestId('slide-status')).toHaveText(`Slide ${n + 1} / ${n + 2}`);
  // The outline view shows the new titles and bullets (the engine created them).
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await page.getByRole('button', { name: 'Dàn ý', exact: true }).click();
  await expect(page.getByTestId('outline-view')).toBeVisible();
  const lines = page.locator('[data-testid^="outline-line-"]');
  await expect(lines.last()).toHaveValue('Việc cần làm');
  const values = await lines.evaluateAll((els) => els.map((e) => (e as HTMLInputElement).value));
  expect(values.slice(-6)).toEqual([
    'Kế hoạch quý IV',
    'Mục tiêu doanh thu',
    'Miền Bắc',
    'Nhân sự',
    'Kết luận',
    'Việc cần làm',
  ]);
  await page.getByRole('button', { name: 'Thường', exact: true }).click();
  // One undo step removes both slides.
  await page.getByTestId('slide-overlay').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Control+z');
  await expect(thumbs(page)).toHaveCount(n);
  expect(completions).toEqual([]);
  expect(errors).toEqual([]);
});

test('BlueDoc: rewrite the selection — preview, then Replace as one tracked, undoable edit', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const sent: Sent[] = [];
  await fakeAi(page, sent, (req) =>
    req.task === 'rewrite' ? 'Kính đề nghị Quý cơ quan xem xét và phản hồi' : 'Tóm tắt ngắn',
  );
  await newDocument(page);
  await page.keyboard.insertText('Xin hãy xem giúp');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Dòng thứ hai');
  await expect(page.getByTestId('status-words')).toHaveText('7 từ');
  // Select the first paragraph, then turn track changes on.
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('Shift+End');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await expect(cmd(page, 'review.trackChanges')).toHaveAttribute('aria-pressed', 'true');

  await cmd(page, 'ai.pane').click();
  const pane = page.getByTestId('ai-pane');
  await expect(pane).toHaveAttribute('data-state', 'on');
  // Where the text goes, and what would be sent.
  await expect(pane.getByTestId('ai-consent')).toContainText('llm.noi-bo.example');
  await expect(pane.getByTestId('ai-material')).toHaveText('Xin hãy xem giúp');
  expect(sent).toEqual([]);
  await pane.getByTestId('ai-tone').selectOption('formal');
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-preview')).toHaveText(
    'Kính đề nghị Quý cơ quan xem xét và phản hồi',
  );
  expect(sent).toEqual([
    { task: 'rewrite', text: 'Xin hãy xem giúp', instruction: '', tone: 'formal', language: 'vi' },
  ]);
  // Still a preview (4 of the 7 words are selected).
  await expect(page.getByTestId('status-words')).toHaveText('4 / 7 từ');
  await pane.getByTestId('ai-apply-replace').click();
  await expect(pane.getByTestId('ai-preview')).toHaveCount(0);
  // Tracked: the replaced words stay as a deletion beside the inserted ones, and the change
  // shows a change bar.
  await expect(page.getByTestId('change-bar').first()).toBeVisible();
  await expect(cmd(page, 'review.acceptAll')).toBeEnabled();
  // One undo step takes the whole replacement back.
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('change-bar')).toHaveCount(0);
  await expect(page.getByTestId('status-words')).toHaveText(/7 từ$/);
  await expect(cmd(page, 'review.acceptAll')).toBeDisabled();
  expect(errors).toEqual([]);
});

test('BlueDoc: a request over the size limit is refused with the limit, before sending', async ({
  page,
}) => {
  const sent: Sent[] = [];
  await fakeAi(page, sent, () => 'x', 10);
  await newDocument(page);
  await page.keyboard.insertText('Một đoạn văn bản dài hơn giới hạn');
  await page.keyboard.press('Control+a');
  const pane = await openPane(page);
  await pane.getByTestId('ai-feature-summarize').click();
  await expect(pane.getByTestId('ai-too-large')).toHaveText(
    'Nội dung quá dài: 33 ký tự, tối đa 10 ký tự mỗi yêu cầu. Hãy chọn ít nội dung hơn.',
  );
  await expect(pane.getByTestId('ai-run')).toBeDisabled();
  expect(sent).toEqual([]);
});

test('BlueSheet: a suggested formula is parsed by the engine and inserted only on request', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const sent: Sent[] = [];
  let next = '=SUM(A2:A4';
  await fakeAi(page, sent, (req) => (req.task === 'formula' ? next : 'Tổng là 60; cao nhất 30.'));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  for (const v of ['Doanh thu', '30', '10', '20']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  const box = page.getByLabel('Hộp tên');
  const goTo = async (ref: string) => {
    await box.click();
    await box.fill(ref);
    await box.press('Enter');
    await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
  };
  await goTo('A1:A4');
  const pane = await openPane(page);
  await expect(pane.getByTestId('ai-consent')).toContainText('llm.noi-bo.example');
  // Only what the pane shows is sent: the headers and first rows of the selection.
  await expect(pane.getByTestId('ai-material')).toHaveText(
    'Selection: A1:A4\nA1 = Doanh thu\nA2 = 30\nA3 = 10\nA4 = 20',
  );
  await pane.getByTestId('ai-prompt').fill('Tổng doanh thu');
  // An answer that does not parse is dropped by the engine check.
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-notice')).toContainText('không hợp lệ');
  await expect(pane.getByTestId('ai-preview')).toHaveCount(0);
  // So is one calling a function the engine does not have.
  next = '=TONGDOANHTHU(A2:A4)';
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-notice')).toContainText('TONGDOANHTHU');
  await expect(pane.getByTestId('ai-preview')).toHaveCount(0);
  // A valid one is offered, not applied.
  next = 'Công thức:\n=SUM(A2:A4)';
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-preview')).toHaveText('=SUM(A2:A4)');
  expect(sent.at(-1)).toMatchObject({ task: 'formula', instruction: 'Tổng doanh thu' });
  expect(sent.at(-1)?.text).toBe('Selection: A1:A4\nA1 = Doanh thu\nA2 = 30\nA3 = 10\nA4 = 20');
  await goTo('A5');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
  await pane.getByTestId('ai-apply-insertFormula').click();
  await expect(pane.getByTestId('ai-notice')).toHaveText('Đã chèn công thức vào ô A5.');
  await goTo('A5');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('=SUM(A2:A4)');
  await goTo('A1:A5');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 120');

  // Analyse data: a short text that can only be copied or discarded.
  await goTo('A1:A4');
  await pane.getByTestId('ai-feature-analyze').click();
  await expect(pane.getByTestId('ai-material')).toHaveText('Doanh thu\n30\n10\n20');
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-preview')).toHaveText('Tổng là 60; cao nhất 30.');
  await expect(pane.locator('[data-testid^="ai-apply-"]')).toHaveCount(0);
  await pane.getByTestId('ai-discard').click();
  await expect(pane.getByTestId('ai-preview')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSlide: an outline from the provider is schema-checked, previewed, then created', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const sent: Sent[] = [];
  let next = '{"slides":[{"title":"A"}],"commands":[{"cmd":"deleteSlide","index":0}]}';
  await fakeAi(page, sent, () => next);
  await openDeck(page);
  const n = await thumbs(page).count();
  const pane = await openPane(page);
  await pane.getByTestId('ai-feature-slidesFromTopic').click();
  await pane.getByTestId('ai-prompt').fill('Chuyển đổi số 2027');
  // An answer outside the schema (extra keys) is dropped; the deck is untouched.
  await pane.getByTestId('ai-run').click();
  await expect(pane.getByTestId('ai-notice')).toContainText('không đúng dạng');
  await expect(pane.getByTestId('ai-preview')).toHaveCount(0);
  await expect(thumbs(page)).toHaveCount(n);
  next =
    '```json\n{"slides":[{"title":"Hiện trạng","bullets":["Hạ tầng","<b>Dữ liệu</b>"]},{"title":"Lộ trình","bullets":[]}]}\n```';
  await pane.getByTestId('ai-run').click();
  // Markup in an answer stays text.
  await expect(pane.getByTestId('ai-preview')).toHaveText(
    'Hiện trạng\n  - Hạ tầng\n  - <b>Dữ liệu</b>\nLộ trình',
  );
  expect(sent.at(-1)).toMatchObject({ task: 'outline', instruction: 'Chuyển đổi số 2027' });
  await expect(thumbs(page)).toHaveCount(n);
  await pane.getByTestId('ai-apply-createSlides').click();
  await expect(thumbs(page)).toHaveCount(n + 2);
  expect(errors).toEqual([]);
});
