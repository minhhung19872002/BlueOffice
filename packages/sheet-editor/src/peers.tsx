/**
 * Live collaboration presence in BlueSheet (CORE-COLLAB-001): a border in each
 * collaborator's colour around the cell / range they selected, with a name tag.
 * Selections travel on the awareness channel (`SheetHost.collabSelect`,
 * `SheetHost.onSheetPeers`).
 */
import { useEffect, useState } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { ScreenRect, SheetHost, SheetPeer, SheetView } from '@blueoffice/platform';
import { parseA1, rangeOf } from './grid';
import styles from './SheetEditor.module.css';

export interface PeerBox {
  clientId: number;
  name: string;
  color: string;
  range: string;
  rect: ScreenRect;
}

/** The peers on `sheet` with their range parsed (`[r0, c0, r1, c1]`). */
export function peersOnSheet(
  peers: SheetPeer[],
  sheet: string,
): (SheetPeer & { cells: [number, number, number, number] })[] {
  return peers.flatMap((p) => {
    if (p.sheet !== sheet) return [];
    const sel = parseA1(p.range);
    if (!sel) return [];
    const r = rangeOf(sel);
    return [{ ...p, cells: [r.r0, r.c0, r.r1, r.c1] as [number, number, number, number] }];
  });
}

/** Boxes of the collaborators' ranges on the shown sheet (screen pixels via `px`). */
export function usePeerBoxes(
  host: SheetHost | undefined,
  view: SheetView,
  sheet: string | undefined,
  px: (r: ScreenRect | null) => ScreenRect | null,
  revision: number,
): PeerBox[] {
  const [peers, setPeers] = useState<SheetPeer[]>([]);
  const [boxes, setBoxes] = useState<PeerBox[]>([]);
  useEffect(() => host?.onSheetPeers?.(setPeers), [host]);
  useEffect(() => {
    if (!host || sheet === undefined) return;
    const on = peersOnSheet(peers, sheet);
    let cancelled = false;
    void Promise.all(on.map((p) => host.rangeRect(view, ...p.cells)))
      .then((rects) => {
        if (cancelled) return;
        setBoxes(
          on.flatMap((p, i) => {
            const rect = px(rects[i] ?? null);
            return rect
              ? [{ clientId: p.clientId, name: p.name, color: p.color, range: p.range, rect }]
              : [];
          }),
        );
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, view, sheet, peers, px, revision]);
  return boxes;
}

export function PeerSelections({ boxes }: { boxes: PeerBox[] }) {
  const { t } = useTranslation();
  return (
    <>
      {boxes.map((b) => {
        const name = b.name || t('collab.anonymous');
        const label = t('collab.inRange', { name, range: b.range });
        return (
          <div
            key={b.clientId}
            className={styles.peerSelection}
            data-testid="peer-selection"
            role="img"
            aria-label={label}
            title={label}
            style={{
              left: b.rect.x,
              top: b.rect.y,
              width: b.rect.w,
              height: b.rect.h,
              borderColor: b.color,
            }}
          >
            <span className={styles.peerTag} style={{ background: b.color }}>
              {name}
            </span>
          </div>
        );
      })}
    </>
  );
}
