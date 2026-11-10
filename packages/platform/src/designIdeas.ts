/**
 * Design suggestions (CORE-DESIGN-001): BlueSlide "Gợi ý thiết kế" (`bo_engine::slide_edit::designer`)
 * and BlueSheet "Phân tích nhanh" with recommended charts (`bo_engine::sheet::quick_cmd`,
 * `bo_sheet::chart_recommend`).
 */
import type { SheetRange } from './sheet';
import type { PageDisplayList } from './types';

/** What a slide holds, as far as its arrangement goes. */
export type SlideDesignKind =
  'titleOnly' | 'section' | 'bullets' | 'longText' | 'pictures' | 'visual';

/** One suggestion with a thumbnail of the slide as it would look. */
export interface SlideDesignIdea {
  /** Id for `designApply` (`kind.name`). */
  id: string;
  /** Thumbnail display list (points, `width` of the request). */
  page: PageDisplayList;
}

/** Answer of `designIdeas`. */
export interface SlideDesignIdeas {
  /** `null`: nothing on the slide to arrange. */
  kind: SlideDesignKind | null;
  ideas: SlideDesignIdea[];
}

export type SlideDesignCommand =
  /** Suggestions for the current slide → `designIdeas` (nothing changes). */
  | { cmd: 'designIdeas'; width?: number }
  /** Apply a suggestion to the current slide (one undo step). */
  | { cmd: 'designApply'; idea: string };

/** Why a chart type is recommended. */
export type ChartRecommendReason =
  | 'trend'
  | 'compare'
  | 'parts'
  | 'relationship'
  | 'manyCategories'
  | 'hierarchy'
  | 'composition'
  | 'default';

/** Shape of the selected data (`bo_sheet::chart_recommend::DataShape`). */
export interface SheetDataShape {
  header: boolean;
  rows: number;
  labelCols: number;
  labels: 'none' | 'text' | 'time';
  series: number;
  positive: boolean;
  labelLen: number;
}

/** A recommended chart type with its preview. */
export interface ChartRecommendation {
  /** Chart type for `insertChart`. */
  kind: string;
  score: number;
  reason: ChartRecommendReason;
  /** SVG preview drawn by the shared chart renderer. */
  svg: string;
}

/** Answer of `recommendCharts`. */
export interface RecommendChartsResult {
  shape: SheetDataShape | null;
  /** The data the chart plots (the region around the selection). */
  range: SheetRange;
  charts: ChartRecommendation[];
}

/** Answer of `quickAnalysis`. */
export interface QuickAnalysisInfo {
  shape: SheetDataShape | null;
  header: boolean;
  labelCols: number;
  /** Columns (0-based) with numbers below the header. */
  numericCols: number[];
  /** Rows (0-based) with numbers right of the labels. */
  numericRows: number[];
  /** The row below the selection is empty. */
  belowFree: boolean;
  /** The column right of the selection is empty. */
  rightFree: boolean;
  region: SheetRange;
}

export type QuickAnalysisCommand =
  | {
      cmd: 'recommendCharts';
      sheet: number;
      range: SheetRange;
      width?: number;
      height?: number;
      limit?: number;
    }
  | { cmd: 'quickAnalysis'; sheet: number; range: SheetRange };
