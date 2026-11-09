/**
 * BlueSheet sparklines (SHEET-SPARK-001, [MS-XLSX] `x14:sparklineGroup`): the groups the engine
 * lists and the commands that insert and change them (`bo_engine::sheet::SheetCommand`).
 */
import type { SheetRange } from './sheet';

/** `line`, `column` or `stacked` (win/loss). */
export type SparklineType = 'line' | 'column' | 'stacked';
/** How empty cells are drawn: leave a gap, as zero, or connect the neighbours (`span`). */
export type SparklineEmpty = 'gap' | 'zero' | 'span';
/** Vertical axis bound: per sparkline, shared by the group, or a custom value. */
export type SparklineAxis = 'individual' | 'group' | 'custom';

/** Colours of a group, resolved to `#RRGGBB`. */
export interface SparklineColorsHex {
  series: string;
  negative: string;
  axis: string;
  markers: string;
  first: string;
  last: string;
  high: string;
  low: string;
}

/** One sparkline: its data range and the cell it is drawn in. */
export interface SparklineInfo {
  formula: string | null;
  location: { row: number; col: number };
}

/** A sparkline group as `sparklines` lists it. */
export interface SparklineGroupInfo {
  index: number;
  type: SparklineType;
  sparklines: SparklineInfo[];
  emptyAs: SparklineEmpty;
  markers: boolean;
  high: boolean;
  low: boolean;
  first: boolean;
  last: boolean;
  negative: boolean;
  displayXAxis: boolean;
  displayHidden: boolean;
  rightToLeft: boolean;
  minAxis: SparklineAxis;
  maxAxis: SparklineAxis;
  manualMin: number | null;
  manualMax: number | null;
  /** Points. */
  lineWeight: number;
  dateAxis: boolean;
  dateRange: string | null;
  colorsHex: SparklineColorsHex;
}

/** Settings to change on a group; colours are `#RRGGBB`. */
export interface SparklinePatch {
  type?: SparklineType;
  markers?: boolean;
  high?: boolean;
  low?: boolean;
  first?: boolean;
  last?: boolean;
  negative?: boolean;
  displayXAxis?: boolean;
  displayHidden?: boolean;
  rightToLeft?: boolean;
  emptyAs?: SparklineEmpty;
  minAxis?: SparklineAxis;
  maxAxis?: SparklineAxis;
  manualMin?: number;
  manualMax?: number;
  lineWeight?: number;
  colorSeries?: string;
  colorNegative?: string;
  colorAxis?: string;
  colorMarkers?: string;
  colorFirst?: string;
  colorLast?: string;
  colorHigh?: string;
  colorLow?: string;
}

/** Sparkline commands of the sheet command bridge. */
export type SparklineCommand =
  /** Sparkline groups of a sheet → `SparklineGroupInfo[]`. */
  | { cmd: 'sparklines'; sheet: number }
  /** Insert a group over `data` (A1 range) into `location` (one row or column) → `{ index }`. */
  | {
      cmd: 'sparklineInsert';
      sheet: number;
      type: SparklineType;
      data: string;
      location: SheetRange;
    }
  /** Change a group's settings. */
  | ({ cmd: 'sparklineEdit'; sheet: number; group: number } & SparklinePatch)
  /** Put the sparklines of a range into one group → `{ index }`. */
  | { cmd: 'sparklineGroup'; sheet: number; range: SheetRange }
  /** Split the groups with a sparkline in a range → `{ groups }`. */
  | { cmd: 'sparklineUngroup'; sheet: number; range: SheetRange }
  /** Remove the sparklines in a range (or their whole groups) → `{ removed }`. */
  | { cmd: 'sparklineClear'; sheet: number; range: SheetRange; wholeGroups?: boolean };
