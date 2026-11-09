import {
  Contrast,
  Crop,
  FileImage,
  ImageDown,
  ImageMinus,
  Palette,
  RotateCcw,
  Shapes,
  SlidersHorizontal,
  SquareDashed,
} from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import {
  PictureAdjustDialog,
  PictureColorGallery,
  PictureCompressDialog,
  PictureCorrectionsGallery,
  PictureCropDialog,
  PictureCropMenu,
  PictureTransparencyGallery,
  type CropAction,
  type PictureFxValue,
  type PictureToolsLabels,
} from './PictureTools';
import type { RibbonTab } from './Ribbon';

/**
 * The "Picture format" contextual tab of BlueDoc and BlueSlide (CORE-PIC-001), built from the
 * shared picture tools: Adjust (corrections, colour, transparency, compress, change, reset,
 * SVG → shapes) and Size (crop menu, adjustment dialog). Galleries and dialogs talk to the
 * engine through the host's command channel; plain buttons run registry commands
 * (`picture.*`, see [`PICTURE_COMMANDS`]).
 */

/** Picture tools command (structurally the platform's `PictureCommand`). */
export type PictureToolsCommand =
  | { cmd: 'pictureInfo' | 'pictureFill' | 'pictureFit' | 'pictureToShapes' }
  | { cmd: 'picturePreviews'; presets: PictureFxValue[]; size?: number }
  | { cmd: 'pictureFx'; fx: PictureFxValue }
  | { cmd: 'pictureCrop'; crop: [number, number, number, number] }
  | { cmd: 'pictureCropAspect'; ratio: string }
  | { cmd: 'pictureCropShape'; preset: string }
  | {
      cmd: 'pictureCompress';
      ppi?: number | null;
      dropCropped?: boolean;
      jpegQuality?: number | null;
      all?: boolean;
    }
  | { cmd: 'pictureChange'; data: string; name?: string }
  | { cmd: 'pictureReset'; size?: boolean };

/** What the tools read from a command result. */
export interface PictureToolsInfo {
  fx: PictureFxValue;
  crop?: [number, number, number, number];
  shape: string;
  pixelWidth: number;
  pixelHeight: number;
  bytes: number;
  ppi: number;
  svg: boolean;
  previews?: string[];
  compressed?: { pictures: number; before: number; after: number };
}

/** Labels of the tab (on top of the gallery and dialog labels). */
export interface PictureTabLabels extends PictureToolsLabels {
  tab: string;
  groupAdjust: string;
  groupSize: string;
  change: string;
  resetPicture: string;
  resetSize: string;
  compress: string;
  toShapes: string;
  adjust: string;
}

/** Registry command ids of the picture tools buttons. */
export const PICTURE_COMMANDS = [
  'picture.change',
  'picture.reset',
  'picture.resetSize',
  'picture.compress',
  'picture.toShapes',
  'picture.adjust',
  'picture.customCrop',
  'picture.fill',
  'picture.fit',
] as const;

/** Action behind each registry command. */
export type PictureActions = Record<(typeof PICTURE_COMMANDS)[number], () => void>;

export interface PictureToolsHost {
  labels: PictureTabLabels;
  /** A picture the tools act on is selected. */
  enabled: boolean;
  /** Changes when the selection changes (the picture info is read again). */
  selectionKey: string;
  run: (
    cmd: PictureToolsCommand,
  ) => Promise<{ picture?: PictureToolsInfo | null; error?: string | null } | null>;
  /** Run a registry command (`picture.*`). */
  exec: (id: string) => void;
  /** Ask the user for a picture file. */
  pickFile: () => Promise<{ bytes: Uint8Array; name: string } | null>;
  /** Offer "convert to shapes" for SVG pictures (BlueSlide). */
  shapes?: boolean;
  /** Called with an error code of a refused command. */
  onError?: (code: string) => void;
  /** Receives the actions of the registry commands (after each render). */
  hub?: PictureActionsHub;
}

/**
 * Hands the actions of the mounted picture tools to the host's registry commands, which
 * run outside rendering (`get` is called when a command runs).
 */
export class PictureActionsHub {
  private actions: PictureActions | null = null;

  set(a: PictureActions | null): void {
    this.actions = a;
  }

  get(): PictureActions | null {
    return this.actions;
  }
}

/** Base64 of bytes (chunked, no stack overflow on large pictures). */
export function toBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

type Dialogs = 'adjust' | 'crop' | 'compress' | null;

/** The picture format tab, its dialogs and the actions of its registry commands. */
export function usePictureTools(host: PictureToolsHost): {
  tab: RibbonTab | null;
  dialogs: ReactNode;
  actions: PictureActions;
} {
  const { labels: l, enabled, exec, hub } = host;
  const [read, setInfo] = useState<PictureToolsInfo | null>(null);
  const info = enabled ? read : null;
  const [dialog, setDialog] = useState<Dialogs>(null);
  const [report, setReport] = useState<PictureToolsInfo['compressed'] | null>(null);
  const hostRef = useRef(host);
  useEffect(() => {
    hostRef.current = host;
  });

  const send = useCallback(async (cmd: PictureToolsCommand) => {
    const r = await hostRef.current.run(cmd);
    if (r?.error) hostRef.current.onError?.(r.error);
    if (r?.picture) setInfo(r.picture);
    return r;
  }, []);

  useEffect(() => {
    if (enabled) void send({ cmd: 'pictureInfo' });
  }, [enabled, host.selectionKey, send]);

  const fx = info?.fx ?? {};
  const fxKey = JSON.stringify(info?.fx ?? null);
  const load = useCallback(
    async (presets: PictureFxValue[]) => {
      const r = await hostRef.current.run({ cmd: 'picturePreviews', presets, size: 96 });
      return r?.picture?.previews ?? null;
    },
    // A new picture or new adjustments give new previews.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [info?.bytes, fxKey, host.selectionKey],
  );
  const setFx = (f: PictureFxValue) => void send({ cmd: 'pictureFx', fx: f });

  const actions: PictureActions = {
    'picture.change': () => {
      void (async () => {
        const f = await hostRef.current.pickFile();
        if (f) await send({ cmd: 'pictureChange', data: toBase64(f.bytes), name: f.name });
      })();
    },
    'picture.reset': () => void send({ cmd: 'pictureReset' }),
    'picture.resetSize': () => void send({ cmd: 'pictureReset', size: true }),
    'picture.compress': () => {
      setReport(null);
      setDialog('compress');
    },
    'picture.toShapes': () => void send({ cmd: 'pictureToShapes' }),
    'picture.adjust': () => setDialog('adjust'),
    'picture.customCrop': () => setDialog('crop'),
    'picture.fill': () => void send({ cmd: 'pictureFill' }),
    'picture.fit': () => void send({ cmd: 'pictureFit' }),
  };

  useEffect(() => {
    hub?.set(actions);
  });

  const onCrop = (a: CropAction) => {
    if (a.kind === 'aspect') void send({ cmd: 'pictureCropAspect', ratio: a.ratio });
    else if (a.kind === 'shape') void send({ cmd: 'pictureCropShape', preset: a.preset });
    else exec(a.kind === 'custom' ? 'picture.customCrop' : `picture.${a.kind}`);
  };

  const noop = () => {};
  const button = (
    id: string,
    label: string,
    icon: ReactNode,
    size: 'large' | 'small' = 'small',
  ) => ({
    id,
    label,
    icon,
    size,
    enabled,
    onSelect: () => exec(id),
  });

  const tab: RibbonTab | null = enabled
    ? {
        id: 'pictureFormat',
        label: l.tab,
        groups: [
          {
            id: 'pictureAdjust',
            label: l.groupAdjust,
            items: [
              {
                id: 'picture.corrections',
                label: l.corrections,
                onSelect: noop,
                enabled,
                render: (
                  <PictureCorrectionsGallery
                    current={fx}
                    load={load}
                    onPick={setFx}
                    labels={l}
                    disabled={!enabled}
                    icon={<Contrast size={18} aria-hidden="true" />}
                    onOptions={() => exec('picture.adjust')}
                  />
                ),
              },
              {
                id: 'picture.color',
                label: l.color,
                onSelect: noop,
                enabled,
                render: (
                  <PictureColorGallery
                    current={fx}
                    load={load}
                    onPick={setFx}
                    labels={l}
                    disabled={!enabled}
                    icon={<Palette size={18} aria-hidden="true" />}
                    onOptions={() => exec('picture.adjust')}
                  />
                ),
              },
              {
                id: 'picture.transparency',
                label: l.transparency,
                onSelect: noop,
                enabled,
                render: (
                  <PictureTransparencyGallery
                    current={fx}
                    load={load}
                    onPick={setFx}
                    labels={l}
                    disabled={!enabled}
                    icon={<SquareDashed size={18} aria-hidden="true" />}
                  />
                ),
              },
              button('picture.compress', l.compress, <ImageDown size={16} aria-hidden="true" />),
              button('picture.change', l.change, <FileImage size={16} aria-hidden="true" />),
              button('picture.reset', l.resetPicture, <RotateCcw size={16} aria-hidden="true" />),
              button('picture.resetSize', l.resetSize, <ImageMinus size={16} aria-hidden="true" />),
              ...(host.shapes && info?.svg
                ? [button('picture.toShapes', l.toShapes, <Shapes size={16} aria-hidden="true" />)]
                : []),
            ],
          },
          {
            id: 'pictureSize',
            label: l.groupSize,
            items: [
              {
                id: 'picture.crop',
                label: l.crop,
                onSelect: noop,
                enabled,
                render: (
                  <PictureCropMenu
                    labels={l}
                    onCrop={onCrop}
                    disabled={!enabled}
                    icon={<Crop size={18} aria-hidden="true" />}
                  />
                ),
              },
              button(
                'picture.adjust',
                l.adjust,
                <SlidersHorizontal size={16} aria-hidden="true" />,
              ),
            ],
          },
        ],
      }
    : null;

  const close = () => setDialog(null);
  const dialogs = (
    <>
      {dialog === 'adjust' && (
        <PictureAdjustDialog open fx={fx} labels={l} onApply={setFx} onClose={close} />
      )}
      {dialog === 'crop' && (
        <PictureCropDialog
          open
          crop={info?.crop}
          labels={l}
          onApply={(crop) => void send({ cmd: 'pictureCrop', crop })}
          onClose={close}
        />
      )}
      {dialog === 'compress' && (
        <PictureCompressDialog
          open
          labels={l}
          info={info}
          report={report}
          onApply={(v) =>
            void send({ cmd: 'pictureCompress', ...v }).then((r) =>
              setReport(r?.picture?.compressed ?? { pictures: 0, before: 0, after: 0 }),
            )
          }
          onClose={close}
        />
      )}
    </>
  );
  return { tab, dialogs, actions };
}
