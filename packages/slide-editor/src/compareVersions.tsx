import { VersionsPane, toBase64, type ChangeRow, type CompareOutcome } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import type {
  DeckChange,
  DeckCompareResult,
  EngineHost,
  OpenedFile,
  VersionHistory,
} from '@blueoffice/platform';
import { useCallback } from 'react';

type T = (key: string, opts?: Record<string, unknown>) => string;

/** Rows of the compare pane, grouped by slide (the old slide number for removed slides). */
export function deckChangeRows(changes: DeckChange[], t: T): ChangeRow<DeckChange>[] {
  return changes.map((c, i) => ({
    id: `${i}`,
    group:
      c.slide !== undefined
        ? t('versions.slideN', { n: c.slide + 1 })
        : t('versions.oldSlideN', { n: (c.oldSlide ?? 0) + 1 }),
    kind: t(`versions.slide.${c.kind}`),
    where: c.shape ?? '',
    ...(c.old !== undefined ? { old: c.old } : {}),
    ...(c.new !== undefined ? { new: c.new } : {}),
    target: c,
  }));
}

/** Version history / compare versions of BlueSlide (Review › Compare versions, CORE-VERSION-001). */
export function SlideVersionsPane({
  engine,
  versions,
  openFile,
  onShow,
  onClose,
  embedded = false,
}: {
  engine: EngineHost;
  versions?: VersionHistory | undefined;
  openFile?: () => Promise<OpenedFile | null>;
  /** Shows a change of the open deck (its slide, its shape selected). */
  onShow: (slide: number, shapeId?: number) => void;
  onClose: () => void;
  /** Inside a task pane (CORE-UI-006). */
  embedded?: boolean;
}) {
  const { t } = useTranslation();
  const compare = useCallback(
    async (bytes: Uint8Array, name: string): Promise<CompareOutcome<DeckChange>> => {
      const r = (await engine.slideCommand({
        cmd: 'compareDeck',
        name,
        base64: toBase64(bytes),
      })) as unknown as Partial<DeckCompareResult> & { error?: string };
      if (r.error || !r.deckChanges) throw new Error(r.error ?? 'compareDeck');
      return {
        rows: deckChangeRows(r.deckChanges, t),
        total: r.total ?? r.deckChanges.length,
        truncated: r.truncated ?? false,
      };
    },
    [engine, t],
  );
  return (
    <VersionsPane
      title={t('versions.slideTitle')}
      versions={versions}
      {...(openFile ? { openFile } : {})}
      compare={compare}
      onSelect={(row) => {
        const c = row.target;
        if (c.slide === undefined) return;
        const removedShape = c.kind === 'shapeRemoved';
        onShow(c.slide, removedShape ? undefined : c.shapeId);
      }}
      onClose={onClose}
      embedded={embedded}
    />
  );
}
