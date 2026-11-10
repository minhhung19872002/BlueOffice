import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-ADDIN-001: organisation add-ins. The BlueOffice server's catalogue (/api/addins) is faked
// by routing (as blueai.spec does for /api/ai); the sample add-in page (tests/e2e/fixtures/addin)
// is served from its own origin, https://addins.test, and runs in a sandboxed frame of the task
// pane, talking to the editor only through postMessage.

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', 'fixtures', 'addin');
const ORIGIN = 'https://addins.test';
const sdk = join(here, '..', '..', '..', 'apps', 'web', 'dist', 'addin-sdk', 'v1.js');

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

function manifest(id: string, permissions: string[], extra: Record<string, unknown> = {}) {
  return {
    manifestVersion: 1,
    id,
    name: { vi: `Mẫu ${id.split('.').pop()}`, en: `Sample ${id}` },
    version: '1.0.0',
    publisher: 'Phòng CNTT',
    startUrl: `${ORIGIN}/sample/index.html`,
    allowedOrigins: [ORIGIN],
    permissions,
    apps: ['doc', 'sheet'],
    ...extra,
  };
}

const FULL = manifest(
  'vn.test.sample',
  ['selection', 'read-document', 'write-document', 'ui:ribbon', 'storage'],
  {
    ribbon: {
      tab: 'insert',
      commands: [
        { id: 'greet', label: { vi: 'Chào', en: 'Greet' }, action: 'event', apps: ['doc'] },
      ],
    },
  },
);
const LIMITED = manifest('vn.test.limited', ['selection']);

/** Fake catalogue + add-in origin; returns the install requests seen. */
async function fakeServer(page: Page, manifests: object[]) {
  const installs = new Map<string, string[]>();
  const requests: string[] = [];
  await page.route('**/api/addins**', async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    requests.push(`${req.method()} ${url.pathname}`);
    const m = /^\/api\/addins\/([^/]+)\/install$/.exec(url.pathname);
    if (m && req.method() === 'POST') {
      installs.set(
        decodeURIComponent(m[1]!),
        (req.postDataJSON() as { permissions: string[] }).permissions,
      );
      return route.fulfill({ json: { installed: true } });
    }
    if (m && req.method() === 'DELETE') {
      installs.delete(decodeURIComponent(m[1]!));
      return route.fulfill({ status: 204 });
    }
    return route.fulfill({
      json: {
        addins: manifests.map((manifest) => {
          const id = (manifest as { id: string }).id;
          return {
            manifest,
            trusted: false,
            installed: installs.has(id),
            granted: installs.get(id) ?? [],
          };
        }),
      },
    });
  });
  await page.route(`${ORIGIN}/**`, async (route) => {
    const path = new URL(route.request().url()).pathname;
    // The add-in hosts its own copy of the SDK built by the web app (`addin-sdk/v1.js`): a
    // public origin may not load scripts from the loopback editor (Private Network Access).
    const file = path.endsWith('/app.js')
      ? join(fixtures, 'app.js')
      : path.endsWith('/addin-sdk.js')
        ? sdk
        : path.endsWith('.html')
          ? join(fixtures, 'index.html')
          : null;
    if (!file) return route.fulfill({ status: 404 });
    return route.fulfill({
      body: readFileSync(file, 'utf8'),
      contentType: file.endsWith('.js') ? 'text/javascript' : 'text/html; charset=utf-8',
      // CSP Embedded Enforcement: the page accepts the policy the editor's frame requires.
      headers: { 'Allow-CSP-From': '*' },
    });
  });
  return { installs, requests };
}

async function install(page: Page, id: string, expectPermissions: string[]) {
  await page.getByRole('tab', { name: 'Add-in', exact: true }).click();
  await cmd(page, 'addins.manage').click();
  const pane = page.getByTestId('addins-pane');
  await expect(pane).toBeVisible();
  await pane.locator(`[data-addin="${id}"]`).getByTestId('addin-install').click();
  const consent = page.getByTestId('addin-consent');
  await expect(consent).toBeVisible();
  const shown = await consent
    .locator('[data-permission]')
    .evaluateAll((els) => els.map((e) => e.getAttribute('data-permission')));
  expect(shown).toEqual(expectPermissions);
  await consent.getByTestId('addin-consent-accept').click();
  await expect(consent).toBeHidden();
  const frame = page.frameLocator(`[data-testid="addin-frame"][data-addin="${id}"]`);
  await expect(frame.locator('body')).toHaveAttribute('data-ready', 'true', { timeout: 20_000 });
  return frame;
}

test('BlueDoc: an installed add-in reads the selection, inserts text and gets ribbon commands', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const server = await fakeServer(page, [FULL]);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Xin chào thế giới');
  // Select "thế giới".
  await page.keyboard.press('Shift+Control+ArrowLeft');
  await page.keyboard.press('Shift+Control+ArrowLeft');

  const frame = await install(page, FULL.id, FULL.permissions);
  expect(server.installs.get(FULL.id)).toEqual(FULL.permissions);
  // The frame is sandboxed without its own origin and under the editor's CSP.
  const el = page.locator(`[data-testid="addin-frame"][data-addin="${FULL.id}"]`);
  await expect(el).toHaveAttribute('sandbox', 'allow-scripts allow-forms');
  await expect(el).toHaveAttribute('csp', /connect-src 'none'/);
  await expect(frame.locator('#status')).toHaveText(
    'ready doc selection,read-document,write-document,ui:ribbon,storage',
  );

  await frame.locator('#read-selection').click();
  await expect(frame.locator('#out')).toHaveText('selected: thế giới');
  await frame.locator('#insert').click();
  await expect(frame.locator('#out')).toHaveText('inserted');
  await frame.locator('#read-document').click();
  await expect(frame.locator('#out')).toHaveText('document: "Xin chào Từ add-in"');

  // The selection changing in the editor reaches the add-in as an event.
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Home');
  await expect(frame.locator('#events')).toContainText('selection;');

  // A ribbon command of the manifest (Insert tab, action "event") reaches the add-in.
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await cmd(page, `addin.${FULL.id}.greet`).click();
  await expect(frame.locator('#events')).toContainText('command:greet;');
  await expect(frame.locator('#out')).toHaveText('inserted');
  await frame.locator('#read-document').click();
  await expect(frame.locator('#out')).toHaveText('document: "Xin chào!Xin chào Từ add-in"');

  // The add-in's own storage.
  await frame.locator('#store').click();
  await expect(frame.locator('#out')).toHaveText('stored 1');
  const key = `blueoffice.addin.storage.${FULL.id}`;
  expect(await page.evaluate((k) => localStorage.getItem(k), key)).toBe('{"n":1}');

  // Uninstall: removed from the catalogue, the pane closes, its storage is deleted.
  await page.getByRole('tab', { name: 'Add-in', exact: true }).click();
  await cmd(page, 'addins.manage').click();
  // The catalogue and the add-in share the right dock: bring the catalogue's tab forward.
  await page.locator('#taskpane-tab-addins').click();
  await page.getByTestId('addins-pane').getByTestId('addin-uninstall').click();
  await expect(el).toHaveCount(0);
  expect(server.installs.has(FULL.id)).toBe(false);
  expect(await page.evaluate((k) => localStorage.getItem(k), key)).toBeNull();
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await expect(cmd(page, `addin.${FULL.id}.greet`)).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSheet: an add-in writes a cell', async ({ page }) => {
  await fakeServer(page, [FULL]);
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const box = page.getByLabel('Hộp tên');
  await box.fill('B2');
  await box.press('Enter');

  const frame = await install(page, FULL.id, FULL.permissions);
  await frame.locator('#read-selection').click();
  await expect(frame.locator('#out')).toContainText('"address":"B2"');
  await frame.locator('#insert').click();
  await expect(frame.locator('#out')).toHaveText('inserted');
  await box.fill('B2');
  await box.press('Enter');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Từ add-in');
  // A sheet has no "greet" command (the manifest limits it to BlueDoc).
  await expect(cmd(page, `addin.${FULL.id}.greet`)).toHaveCount(0);
});

test('A call without the permission is refused; nothing changes', async ({ page }) => {
  await fakeServer(page, [LIMITED]);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Bí mật');
  await page.keyboard.press('Shift+Home');

  const frame = await install(page, LIMITED.id, LIMITED.permissions);
  await expect(frame.locator('#status')).toHaveText('ready doc selection');
  // Allowed: the selection.
  await frame.locator('#read-selection').click();
  await expect(frame.locator('#out')).toHaveText('selected: Bí mật');
  // Refused: the whole document, writing, storage.
  await frame.locator('#read-document').click();
  await expect(frame.locator('#out')).toHaveText(
    'error -32001: doc.text needs the read-document permission [read-document]',
  );
  await frame.locator('#insert').click();
  await expect(frame.locator('#out')).toHaveText(
    'error -32001: doc.insertText needs the write-document permission [write-document]',
  );
  await frame.locator('#store').click();
  await expect(frame.locator('#out')).toHaveText(
    'error -32001: storage.set needs the storage permission [storage]',
  );
  // The document is unchanged: the word count still counts two words.
  await page.getByTestId('document-view').click();
  await page.keyboard.press('Control+a');
  await expect(page.getByTestId('status-words')).toContainText('2');
});

test('Add-ins are off without a catalogue: no Add-in tab', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Chèn', exact: true })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Add-in', exact: true })).toHaveCount(0);
});
