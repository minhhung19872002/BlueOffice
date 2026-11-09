// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  LUM_PRESETS,
  RECOLOR_PRESETS,
  Ribbon,
  SATURATION_PRESETS,
  SHARPNESS_PRESETS,
  TRANSPARENCY_PRESETS,
  formatBytes,
  mergeFx,
  toBase64,
  usePictureTools,
  PICTURE_COMMANDS,
  PictureActionsHub,
  type PictureTabLabels,
  type PictureToolsCommand,
  type PictureToolsInfo,
} from '../src';

afterEach(cleanup);

const labels: PictureTabLabels = {
  tab: 'Định dạng ảnh',
  groupAdjust: 'Chỉnh ảnh',
  groupSize: 'Cắt',
  change: 'Thay ảnh',
  resetPicture: 'Khôi phục',
  resetSize: 'Khôi phục cả cỡ',
  compress: 'Nén',
  toShapes: 'Thành hình',
  adjust: 'Tinh chỉnh',
  corrections: 'Sáng và nét',
  sharpness: 'Độ nét',
  brightnessContrast: 'Sáng/tương phản',
  color: 'Màu',
  saturation: 'Bão hòa',
  recolor: 'Phối màu',
  transparency: 'Trong suốt',
  crop: 'Cắt ảnh',
  cropAspect: 'Theo tỷ lệ',
  cropShape: 'Theo hình',
  fill: 'Phủ kín',
  fit: 'Vừa khung',
  customCrop: 'Tùy chỉnh',
  options: 'Tinh chỉnh…',
  loading: 'Đang tải',
  preset: (id) => `preset ${id}`,
  shape: (s) => `hình ${s}`,
  ok: 'OK',
  cancel: 'Hủy',
  apply: 'Áp dụng',
  adjustTitle: 'Tinh chỉnh ảnh',
  bright: 'Sáng',
  contrast: 'Tương phản',
  sharpen: 'Nét',
  alpha: 'Trong suốt',
  reset: 'Mặc định',
  cropTitle: 'Cắt tùy chỉnh',
  left: 'Trái',
  top: 'Trên',
  right: 'Phải',
  bottom: 'Dưới',
  compressTitle: 'Nén ảnh',
  resolution: 'Độ phân giải',
  keepResolution: 'Giữ nguyên',
  ppi: (n) => `${n} ppi`,
  dropCropped: 'Bỏ phần cắt',
  jpegQuality: 'JPEG',
  scopeAll: 'Mọi ảnh',
  current: (i) => `${i.pixels} @ ${i.ppi} (${i.size})`,
  saved: (b) => `giảm ${b}`,
  nothingSaved: 'không giảm',
};

const info: PictureToolsInfo = {
  fx: { bright: 20000 },
  shape: 'rect',
  pixelWidth: 400,
  pixelHeight: 300,
  bytes: 2048,
  ppi: 150,
  svg: true,
};

describe('picture presets', () => {
  it('values are hand-derived percentages (1/1000 %)', () => {
    expect(SHARPNESS_PRESETS.map((p) => p.id)).toEqual([
      'sharp.soften50',
      'sharp.soften25',
      'sharp.none',
      'sharp.sharpen25',
      'sharp.sharpen50',
    ]);
    expect(SHARPNESS_PRESETS[0]!.fx).toEqual({ sharpen: -50000 });
    expect(LUM_PRESETS).toHaveLength(25);
    expect(LUM_PRESETS.find((p) => p.id === 'lum.20.-40')!.fx).toEqual({
      bright: 20000,
      contrast: -40000,
    });
    expect(SATURATION_PRESETS.find((p) => p.id === 'sat.0')!.fx).toEqual({ saturation: 0 });
    // 30 % see-through = 70 % opacity; 0 % removes the transform.
    expect(TRANSPARENCY_PRESETS.find((p) => p.id === 'alpha.30')!.fx).toEqual({ alpha: 70000 });
    expect(TRANSPARENCY_PRESETS[0]!.fx).toEqual({ alpha: null });
    const ids = RECOLOR_PRESETS.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    // Recolour presets replace each other.
    const gray = mergeFx({ duotone: [1, 2], bright: 5 }, RECOLOR_PRESETS[1]!.fx);
    expect(gray).toEqual({ duotone: null, bright: 5, grayscale: true, bilevel: null });
  });

  it('formats sizes and base64', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(3 * 1024 * 1024)).toBe('3.00 MB');
    expect(toBase64(new Uint8Array([104, 105]))).toBe('aGk=');
  });
});

function Harness({
  run,
  hub,
}: {
  run: (c: PictureToolsCommand) => Promise<{ picture?: PictureToolsInfo | null } | null>;
  hub: PictureActionsHub;
}) {
  const p = usePictureTools({
    labels,
    enabled: true,
    selectionKey: 'a',
    run,
    exec: (id) => hub.get()?.[id as (typeof PICTURE_COMMANDS)[number]](),
    pickFile: async () => ({ bytes: new Uint8Array([1, 2, 3]), name: 'b.png' }),
    shapes: true,
    hub,
  });
  return (
    <div className="bo-root">
      {p.tab && <Ribbon tabs={[p.tab]} />}
      {p.dialogs}
    </div>
  );
}

describe('usePictureTools', () => {
  it('reads the picture, crops, changes and compresses through commands', async () => {
    const sent: PictureToolsCommand[] = [];
    const run = vi.fn(async (c: PictureToolsCommand) => {
      sent.push(c);
      if (c.cmd === 'pictureCompress') {
        return { picture: { ...info, compressed: { pictures: 1, before: 4096, after: 1024 } } };
      }
      if (c.cmd === 'picturePreviews') return { picture: { ...info, previews: [] } };
      return { picture: info };
    });
    const hub = new PictureActionsHub();
    render(<Harness run={run} hub={hub} />);
    await waitFor(() => expect(sent[0]).toEqual({ cmd: 'pictureInfo' }));
    // SVG pictures offer "turn into shapes".
    expect(await screen.findByText('Thành hình')).toBeTruthy();
    // Crop menu → 16:9.
    fireEvent.click(screen.getByTestId('picture-crop'));
    fireEvent.click(await screen.findByTestId('picture-crop-ratio-16x9'));
    await waitFor(() => expect(sent).toContainEqual({ cmd: 'pictureCropAspect', ratio: '16:9' }));
    // Crop to a shape.
    fireEvent.click(screen.getByTestId('picture-crop'));
    fireEvent.click(await screen.findByTestId('picture-crop-shape-ellipse'));
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: 'pictureCropShape', preset: 'ellipse' }),
    );
    // Change picture: the file goes as base64.
    await act(async () => hub.get()!['picture.change']());
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: 'pictureChange', data: 'AQID', name: 'b.png' }),
    );
    // Compress: settings, then the saved size.
    act(() => hub.get()!['picture.compress']());
    expect((await screen.findByTestId('picture-compress-current')).textContent).toBe(
      '400 × 300 @ 150 (2.0 KB)',
    );
    fireEvent.click(screen.getByTestId('picture-compress-ppi-96'));
    fireEvent.click(screen.getByTestId('picture-compress-ok'));
    expect((await screen.findByTestId('picture-compress-result')).textContent).toBe('giảm 3.0 KB');
    expect(sent).toContainEqual({
      cmd: 'pictureCompress',
      ppi: 96,
      dropCropped: true,
      jpegQuality: 85,
      all: false,
    });
  });

  it('gallery previews come from the engine and a pick merges the preset', async () => {
    const sent: PictureToolsCommand[] = [];
    const run = vi.fn(async (c: PictureToolsCommand) => {
      sent.push(c);
      if (c.cmd === 'picturePreviews') {
        return { picture: { ...info, previews: c.presets.map(() => 'iVBORw0KGgo=') } };
      }
      return { picture: info };
    });
    const hub = new PictureActionsHub();
    render(<Harness run={run} hub={hub} />);
    await waitFor(() => expect(sent[0]).toEqual({ cmd: 'pictureInfo' }));
    fireEvent.click(screen.getByTestId('picture-color'));
    const sepia = await screen.findByTestId('picture-color-grid-recolor.sepia');
    await waitFor(() => expect(sepia.querySelector('img')).toBeTruthy());
    const previews = sent.find((c) => c.cmd === 'picturePreviews');
    // 7 saturation + 13 recolour presets, each on top of the current adjustments.
    expect(previews && 'presets' in previews && previews.presets).toHaveLength(20);
    fireEvent.click(sepia);
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: 'pictureFx',
        fx: { bright: 20000, grayscale: false, bilevel: null, duotone: [0x2b1d0e, 0xf2e3c6] },
      }),
    );
  });
});
