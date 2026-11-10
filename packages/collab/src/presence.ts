/**
 * Presence helpers shared by the BlueDoc / BlueSheet / BlueSlide bindings and
 * their editors (who is editing which paragraph / cell / slide). Presence
 * travels on the y-sync awareness channel (`bo_collab::Presence`).
 */
import type { Presence } from './sheet';

/** Awareness `cell` value of a selection: `"<sheet>!<range>"`. */
export const cellPresence = (sheet: string, range: string): string => `${sheet}!${range}`;

/** Splits an awareness `cell` value (sheet names may contain `!`: the last one separates). */
export function parseCellPresence(cell: string): { sheet: string; range: string } | null {
  const bang = cell.lastIndexOf('!');
  if (bang <= 0 || bang === cell.length - 1) return null;
  return { sheet: cell.slice(0, bang), range: cell.slice(bang + 1) };
}

/** A collaborator's selected cell / range (`SheetPeer` in @blueoffice/platform). */
export interface SheetPeerView {
  clientId: number;
  name: string;
  color: string;
  sheet: string;
  range: string;
}

/** The collaborators of a replica's `peersJson()` that selected a cell / range. */
export function sheetPeersOf(peersJson: string): SheetPeerView[] {
  let peers: [number, Presence][];
  try {
    peers = JSON.parse(peersJson) as [number, Presence][];
  } catch {
    return [];
  }
  return peers.flatMap(([clientId, p]) => {
    const at = typeof p.cell === 'string' ? parseCellPresence(p.cell) : null;
    return at ? [{ clientId, name: p.name, color: p.color, ...at }] : [];
  });
}

/** Awareness `shape` value of a slide without a selected object. */
export const slidePresence = (slideKey: string): string => `${slideKey}/`;
