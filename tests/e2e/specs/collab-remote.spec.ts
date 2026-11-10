// SLIDE-REMOTE-001 against the real BlueOffice server (collab stack): a phone (second browser
// context, phone viewport) paired through the QR link drives the presenter's slide show, and a
// viewer of the online presentation follows the presenter's slide and animation step.
import { expect, test, type Browser, type FrameLocator } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { corpus, createFromCorpus, openAs, zipEntry } from '../collab/drive';

/** Text of the notes page of slide `n` (1-based), read from the package itself. */
function notesOf(path: string, n: number): string {
  const xml = zipEntry(readFileSync(corpus(path)), `ppt/notesSlides/notesSlide${n}.xml`);
  const decode = (s: string) =>
    s
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/&amp;/g, '&');
  // Paragraphs (ECMA-376 §21.1.2.2.6 p) of the notes body; the slide image placeholder has none.
  return (xml?.toString('utf8') ?? '')
    .split('</a:p>')
    .map((p) => [...p.matchAll(/<a:t>([^<]*)<\/a:t>/g)].map((m) => decode(m[1]!)).join(''))
    .filter((p) => p.trim())
    .join(' ');
}

async function command(frame: FrameLocator, id: string) {
  await frame.locator(`[data-command="${id}"]`).first().click();
}

async function phone(browser: Browser) {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  return { context, page, errors };
}

test('a phone paired by QR link drives the presenter’s show', async ({ browser }) => {
  const deck = 'slide/slide-notes-multi.pptx';
  const fileId = await createFromCorpus(deck, 'Bài giảng có ghi chú.pptx');
  const a = await openAs(browser, 'u1', fileId);
  const f = a.frame;
  await expect(f.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);

  await f.getByRole('tab', { name: 'Trình chiếu' }).click();
  await command(f, 'show.remote');
  const dialog = f.getByTestId('remote-dialog');
  await expect(dialog).toBeVisible();
  await expect(f.getByTestId('remote-qr')).toBeVisible();
  const url = (await f.getByTestId('remote-url').textContent())!;
  expect(url).toMatch(/\/remote\/[A-Za-z0-9_-]{22}$/);
  await expect(f.getByTestId('remote-status')).toHaveText('Đang chờ điện thoại quét mã…');

  const p = await phone(browser);
  await p.page.goto(url);
  await expect(f.getByTestId('remote-status')).toHaveText(
    'Điện thoại đã kết nối và đang điều khiển.',
  );
  await expect(f.getByTestId('remote-qr')).toHaveCount(0);
  // Not presenting yet: the phone shows where the presenter is and offers to start.
  await expect(p.page.getByTestId('remote-position')).toHaveText(
    /^Chưa trình chiếu — đang ở trang 1\//,
  );
  await expect(p.page.getByTestId('remote-notes')).toHaveText(notesOf(deck, 1));

  // The code was used: another phone cannot pair with it.
  const other = await phone(browser);
  await other.page.goto(url);
  await expect(other.page.getByTestId('remote-page')).toHaveAttribute('data-phase', 'invalid');
  await other.context.close();

  await f.getByTestId('remote-close').click();
  await p.page.getByTestId('remote-start').tap();
  const stage = f.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await expect(p.page.getByTestId('remote-current')).toHaveAttribute('src', /^data:image\/jpeg/);
  await expect(p.page.getByTestId('remote-next-thumb')).toHaveAttribute('src', /^data:image\/jpeg/);
  await expect(p.page.getByTestId('remote-timer')).toBeVisible();

  // Next / previous reach the presenter; the notes follow on the phone.
  await p.page.getByTestId('remote-next').tap();
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(p.page.getByTestId('remote-notes')).toHaveText(notesOf(deck, 2));
  await expect(p.page.getByTestId('remote-position')).toHaveText(/^Trang chiếu 2 \(2\/\d+\)/);
  await p.page.getByTestId('remote-next').tap();
  await expect(stage).toHaveAttribute('data-slide', '2');
  await p.page.getByTestId('remote-prev').tap();
  await expect(stage).toHaveAttribute('data-slide', '1');

  // Black screen on and off.
  await p.page.getByTestId('remote-black').tap();
  await expect(f.getByTestId('show-surface')).toHaveAttribute('data-cover', 'black');
  await expect(p.page.getByTestId('remote-black')).toHaveAttribute('aria-pressed', 'true');
  await p.page.getByTestId('remote-black').tap();
  await expect(f.getByTestId('show-surface')).toHaveAttribute('data-cover', 'none');

  // End from the phone.
  await p.page.getByTestId('remote-end').tap();
  await expect(stage).toHaveCount(0);
  await expect(f.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  await expect(p.page.getByTestId('remote-start')).toBeVisible();

  // The presenter revokes the phone.
  await command(f, 'show.remote');
  await f.getByTestId('remote-revoke').click();
  await expect(p.page.getByTestId('remote-page')).toHaveAttribute('data-phase', 'revoked');
  await expect(p.page.getByTestId('remote-next')).toHaveCount(0);
  await expect(p.page.getByTestId('remote-start')).toHaveCount(0);

  expect(a.errors).toEqual([]);
  expect(p.errors).toEqual([]);
  await p.context.close();
  await a.context.close();
});

test('viewers of the online presentation follow the slide and animation step', async ({
  browser,
}) => {
  const fileId = await createFromCorpus('slide/slide-anim-fade-appear.pptx', 'Hiệu ứng.pptx');
  const a = await openAs(browser, 'u1', fileId);
  const f = a.frame;
  await expect(f.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await f.getByRole('tab', { name: 'Trình chiếu' }).click();
  await command(f, 'show.online');
  await f.getByTestId('online-start').click();
  const link = f.getByTestId('online-url');
  await expect(link).toHaveValue(/\/watch\/[A-Za-z0-9_-]{22}$/);
  const url = await link.inputValue();

  const vctx = await browser.newContext({ viewport: { width: 1024, height: 640 } });
  const viewer = await vctx.newPage();
  const verrors: string[] = [];
  viewer.on('pageerror', (e) => verrors.push(e.message));
  await viewer.goto(url);
  await expect(viewer.getByTestId('audience-waiting')).toHaveText(
    'Người trình bày chưa bắt đầu trình chiếu. Trang sẽ tự hiện khi bắt đầu.',
  );
  await expect(f.getByTestId('online-viewers')).toHaveText('Người đang xem: 1');
  await f.getByTestId('online-close').click();

  // The presenter starts the show and plays the first effect.
  await command(f, 'show.fromStart');
  const stage = f.getByTestId('show-stage');
  const vstage = viewer.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await expect(vstage).toHaveAttribute('data-slide', '0');
  await expect(vstage).toHaveAttribute('data-click', '0');
  const clicks = Number(await stage.getAttribute('data-clicks'));
  expect(clicks).toBeGreaterThan(0);
  await stage.click();
  await expect(stage).toHaveAttribute('data-click', '1');
  await expect(vstage).toHaveAttribute('data-click', '1');
  await expect(vstage).toHaveAttribute('data-clicks', String(clicks));

  // The viewer cannot move ahead.
  await viewer.keyboard.press('ArrowRight');
  await viewer.keyboard.press('PageDown');
  await vstage.click();
  await expect(vstage).toHaveAttribute('data-click', '1');
  await expect(vstage).toHaveAttribute('data-slide', '0');
  await expect(stage).toHaveAttribute('data-click', '1');

  // The rest of the slide's steps, then the next slide.
  for (let c = 2; c <= clicks; c++) {
    await stage.click();
    await expect(vstage).toHaveAttribute('data-click', String(c));
  }
  await stage.click();
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(vstage).toHaveAttribute('data-slide', '1');
  await expect(vstage).toHaveAttribute('data-click', '0');

  // The presenter ends the show, then the online presentation.
  await a.page.keyboard.press('Escape');
  await expect(stage).toHaveCount(0);
  await expect(viewer.getByTestId('audience-waiting')).toBeVisible();
  await command(f, 'show.online');
  await f.getByTestId('online-end').click();
  await expect(viewer.getByTestId('watch-message')).toHaveText(
    'Buổi trình chiếu trực tuyến đã kết thúc.',
  );
  // The link does not work any more.
  await viewer.reload();
  await expect(viewer.getByTestId('watch-message')).toHaveText(
    'Liên kết không hợp lệ hoặc buổi trình chiếu đã kết thúc.',
  );

  expect(a.errors).toEqual([]);
  expect(verrors).toEqual([]);
  await vctx.close();
  await a.context.close();
});
