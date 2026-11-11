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
  graphCutProvider,
  type SegmentationProvider,
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
  removeBackground: 'Xoá nền',
  markKeep: 'Đánh dấu vùng giữ lại',
  markRemove: 'Đánh dấu vùng xoá bỏ',
  undoMark: 'Bỏ dấu',
  discard: 'Huỷ thay đổi',
  keepChanges: 'Giữ thay đổi',
  computing: 'Đang tách nền',
  kept: (p) => `giữ ${p}%`,
  canvas: 'Ảnh xem trước',
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

function BgHarness({
  run,
  hub,
  segmentation,
}: {
  run: (c: PictureToolsCommand) => Promise<{ picture?: PictureToolsInfo | null } | null>;
  hub: PictureActionsHub;
  segmentation?: SegmentationProvider;
}) {
  const p = usePictureTools({
    labels,
    enabled: true,
    selectionKey: 'a',
    run,
    exec: (id) => hub.get()?.[id as (typeof PICTURE_COMMANDS)[number]](),
    pickFile: async () => null,
    hub,
    ...(segmentation ? { segmentation } : {}),
  });
  return (
    <div className="bo-root">
      {p.tab && <Ribbon tabs={[p.tab]} />}
      {p.dialogs}
    </div>
  );
}

type Sent = PictureToolsCommand & { marks?: { keep: boolean; points: [number, number][] }[] };

function bgEngine(removable = true) {
  const sent: Sent[] = [];
  const run = vi.fn(async (c: PictureToolsCommand) => {
    sent.push(c as Sent);
    if (c.cmd === 'pictureBackgroundPreview') {
      return {
        picture: {
          ...info,
          removable,
          background: {
            image: 'iVBORw0KGgo=',
            width: 200,
            height: 100,
            kept: 24000,
            ms: 3,
            ...(c.source ? { source: 'U1JD' } : {}),
          },
        },
      };
    }
    return { picture: { ...info, removable } };
  });
  return { sent, run };
}

const previews = (sent: Sent[]) => sent.filter((c) => c.cmd === 'pictureBackgroundPreview');

describe('background removal mode (CORE-PIC-002)', () => {
  it('marks from the keyboard and the pointer, previews each change, keeps the result', async () => {
    const { sent, run } = bgEngine();
    const hub = new PictureActionsHub();
    const { container } = render(<BgHarness run={run} hub={hub} />);
    await waitFor(() => expect(sent[0]).toEqual({ cmd: 'pictureInfo' }));
    const button = container.querySelector('[data-command="picture.removeBackground"]');
    await waitFor(() => expect(button?.getAttribute('aria-disabled')).toBe('false'));
    act(() => hub.get()!['picture.removeBackground']());
    expect(await screen.findByTestId('picture-bg-dialog')).toBeTruthy();
    // The first preview: no marks, the engine's rectangle (graph-cut provider: no prior).
    await waitFor(() =>
      expect(screen.getByTestId('picture-bg-status').textContent).toBe('giữ 24%'),
    );
    expect(previews(sent)[0]).toEqual({ cmd: 'pictureBackgroundPreview', marks: [] });
    expect(screen.getByTestId('picture-bg-preview').getAttribute('src')).toBe(
      'data:image/png;base64,iVBORw0KGgo=',
    );
    // "Mark areas to remove", then two steps right and Space at the cursor.
    const remove = screen.getByTestId('picture-bg-mark-remove');
    fireEvent.click(remove);
    expect(remove.getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('picture-bg-mark-keep').getAttribute('aria-pressed')).toBe('false');
    const area = screen.getByTestId('picture-bg-area');
    area.focus();
    fireEvent.keyDown(area, { key: 'ArrowRight' });
    fireEvent.keyDown(area, { key: 'ArrowRight' });
    fireEvent.keyDown(area, { key: 'ArrowUp', shiftKey: true });
    fireEvent.keyDown(area, { key: ' ' });
    await waitFor(() => expect(previews(sent)).toHaveLength(2));
    const m = previews(sent)[1]!.marks!;
    expect(m).toHaveLength(1);
    expect(m[0]!.keep).toBe(false);
    expect(m[0]!.points[0]![0]).toBeCloseTo(0.54);
    expect(m[0]!.points[0]![1]).toBeCloseTo(0.4);
    // A pointer stroke with "Mark areas to keep": fractions of the area.
    fireEvent.click(screen.getByTestId('picture-bg-mark-keep'));
    area.getBoundingClientRect = () =>
      ({
        left: 0,
        top: 0,
        width: 200,
        height: 100,
        right: 200,
        bottom: 100,
        x: 0,
        y: 0,
      }) as DOMRect;
    fireEvent.pointerDown(area, { clientX: 20, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(area, { clientX: 100, clientY: 50, pointerId: 1 });
    fireEvent.pointerUp(area, { clientX: 100, clientY: 50, pointerId: 1 });
    await waitFor(() => expect(previews(sent)).toHaveLength(3));
    const stroke = previews(sent)[2]!.marks![1]!;
    expect(stroke).toEqual({
      keep: true,
      points: [
        [0.1, 0.1],
        [0.5, 0.5],
      ],
    });
    expect(area.getAttribute('data-marks')).toBe('2');
    // Undo the last mark: back to one mark (a new preview).
    fireEvent.click(screen.getByTestId('picture-bg-undo-mark'));
    await waitFor(() => expect(previews(sent)).toHaveLength(4));
    expect(previews(sent)[3]!.marks).toHaveLength(1);
    // Keep changes: one removal command with the marks; the mode closes.
    fireEvent.click(screen.getByTestId('picture-bg-keep'));
    await waitFor(() =>
      expect(sent.find((c) => c.cmd === 'pictureRemoveBackground')).toEqual({
        cmd: 'pictureRemoveBackground',
        marks: previews(sent)[3]!.marks,
        source: false,
      }),
    );
    await waitFor(() => expect(screen.queryByTestId('picture-bg-dialog')).toBeNull());
  });

  it('discarding sends no removal; pictures that cannot be cleared disable the button', async () => {
    const { sent, run } = bgEngine();
    const hub = new PictureActionsHub();
    render(<BgHarness run={run} hub={hub} />);
    await waitFor(() => expect(sent[0]).toEqual({ cmd: 'pictureInfo' }));
    act(() => hub.get()!['picture.removeBackground']());
    await screen.findByTestId('picture-bg-dialog');
    await waitFor(() => expect(previews(sent)).toHaveLength(1));
    fireEvent.click(screen.getByTestId('picture-bg-discard'));
    await waitFor(() => expect(screen.queryByTestId('picture-bg-dialog')).toBeNull());
    expect(sent.some((c) => c.cmd === 'pictureRemoveBackground')).toBe(false);
    cleanup();
    const svg = bgEngine(false);
    const { container } = render(<BgHarness run={svg.run} hub={new PictureActionsHub()} />);
    await waitFor(() => expect(svg.sent[0]).toEqual({ cmd: 'pictureInfo' }));
    await waitFor(() =>
      expect(
        container
          .querySelector('[data-command="picture.removeBackground"]')
          ?.getAttribute('aria-disabled'),
      ).toBe('true'),
    );
  });

  it('a segmentation provider gets the working picture and supplies the starting mask', async () => {
    const { sent, run } = bgEngine();
    const hub = new PictureActionsHub();
    const prior = vi.fn(async () => ({ width: 2, height: 1, data: '/wA=' }));
    render(<BgHarness run={run} hub={hub} segmentation={{ id: 'test-model', prior }} />);
    await waitFor(() => expect(sent[0]).toEqual({ cmd: 'pictureInfo' }));
    act(() => hub.get()!['picture.removeBackground']());
    await waitFor(() =>
      expect(prior).toHaveBeenCalledWith({ image: 'U1JD', width: 200, height: 100 }),
    );
    expect(previews(sent)[0]).toEqual({ cmd: 'pictureBackgroundPreview', marks: [], source: true });
    await waitFor(() =>
      expect(previews(sent)[1]).toEqual({
        cmd: 'pictureBackgroundPreview',
        marks: [],
        prior: { width: 2, height: 1, data: '/wA=' },
      }),
    );
    expect(graphCutProvider.prior).toBeUndefined();
  });
});
