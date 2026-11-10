import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** A self-made GLB (glTF 2.0 §4.4): a unit cube, positions + indices, one material. */
function cubeGlb(): Buffer {
  const p = [
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
  ];
  const idx = [
    0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 4, 0, 3, 4, 3, 7, 3, 2, 6, 3, 6, 7, 4, 5,
    1, 4, 1, 0,
  ];
  const bin = Buffer.alloc(p.length * 12 + idx.length * 4);
  p.flat().forEach((v, i) => bin.writeFloatLE(v, i * 4));
  idx.forEach((v, i) => bin.writeUInt32LE(v, p.length * 12 + i * 4));
  const json = JSON.stringify({
    asset: { version: '2.0' },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, material: 0 }] }],
    materials: [
      { pbrMetallicRoughness: { baseColorFactor: [0.2, 0.5, 0.85, 1], metallicFactor: 0 } },
    ],
    accessors: [
      {
        bufferView: 0,
        componentType: 5126,
        count: 8,
        type: 'VEC3',
        min: [-0.5, -0.5, -0.5],
        max: [0.5, 0.5, 0.5],
      },
      { bufferView: 1, componentType: 5125, count: idx.length, type: 'SCALAR' },
    ],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: p.length * 12 },
      { buffer: 0, byteOffset: p.length * 12, byteLength: idx.length * 4 },
    ],
    buffers: [{ byteLength: bin.length }],
  });
  const js = Buffer.from(json.padEnd(Math.ceil(json.length / 4) * 4, ' '));
  const head = Buffer.alloc(12);
  head.write('glTF', 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + js.length + 8 + bin.length, 8);
  const chunk = (data: Buffer, type: number) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.writeUInt32LE(type, 4);
    return Buffer.concat([h, data]);
  };
  return Buffer.concat([head, chunk(js, 0x4e4f534a), chunk(bin, 0x004e4942)]);
}

test('inserts a 3D model, turns it, saves and reopens with the view kept', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);

  // Insert › 3D model.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mô hình 3D', exact: true }).click();
  await (
    await chooser
  ).setFiles({ name: 'khoi.glb', mimeType: 'model/gltf-binary', buffer: cubeGlb() });
  const layer = page.getByTestId('model3d-layer');
  await expect(layer).toBeVisible();
  await expect(layer).toHaveAttribute('data-rot', '0,0,0');
  // The contextual tab of the model is shown.
  await expect(page.getByRole('tab', { name: 'Mô hình 3D' })).toBeVisible();

  // Drag the rotation handle 60 px right and 20 px down: ay = 30°, ax = 10°.
  const h = (await page.getByTestId('model3d-rotate').boundingBox())!;
  await page.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
  await page.mouse.down();
  for (let i = 1; i <= 6; i++) {
    await page.mouse.move(h.x + h.width / 2 + i * 10, h.y + h.height / 2 + (i * 20) / 6);
  }
  await page.mouse.up();
  await expect(layer).toHaveAttribute('data-rot', '600000,1800000,0');

  // A preset view from the ribbon, then undo back to the dragged view.
  await page.getByRole('tab', { name: 'Mô hình 3D' }).click();
  await page.getByRole('button', { name: 'Từ trên xuống', exact: true }).click();
  await expect(layer).toHaveAttribute('data-rot', '5400000,0,0');
  await page.keyboard.press('Control+z');
  await expect(layer).toHaveAttribute('data-rot', '600000,1800000,0');
  const viewport = await layer.getAttribute('data-viewport');

  // Save, reopen, select the model again: same view.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  const copy = join(tmpdir(), `bo-e2e-${Date.now()}-model3d.pptx`);
  writeFileSync(copy, readFileSync(saved));
  await openDeck(page, copy);
  await expect(layer).toHaveCount(0);
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width / 2 + 20, o.y + o.height / 2 + 20);
  await expect(layer).toBeVisible();
  await expect(layer).toHaveAttribute('data-rot', '600000,1800000,0');
  await expect(layer).toHaveAttribute('data-viewport', viewport ?? '');
  expect(errors).toEqual([]);
});
