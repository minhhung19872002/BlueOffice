/**
 * Text orientation in the Format Cells dialog. The dialog shows an angle of
 * −90…90 degrees (positive = counter-clockwise) plus a "vertical text" switch;
 * the file stores `alignment/@textRotation` (ECMA-376 Part 1, §18.8.1):
 * 0–90 counter-clockwise, 91–180 meaning 1–90 degrees clockwise, 255 stacked.
 */

/** `textRotation` value meaning vertical (stacked) text. */
export const VERTICAL_TEXT = 255;

/** Dialog angle (−90…90) for a stored `textRotation`. */
export function rotationToDegrees(rotation: number): number {
  if (rotation > 90 && rotation <= 180) return -(rotation - 90);
  if (rotation >= 0 && rotation <= 90) return rotation;
  return 0;
}

/** Stored `textRotation` for a dialog angle (clamped to −90…90) or vertical text. */
export function degreesToRotation(degrees: number, vertical: boolean): number {
  if (vertical) return VERTICAL_TEXT;
  const d = Math.round(Math.max(-90, Math.min(90, Number.isFinite(degrees) ? degrees : 0)));
  return d >= 0 ? d : 90 - d;
}
