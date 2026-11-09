/**
 * Chart Design (CORE-CHART-007): chart styles, colour variations, quick layouts and chart
 * templates shared by BlueSheet and BlueSlide (`bo_engine::chart_design`).
 */

/** Titles a quick layout or template adds to a chart that has none (localised by the UI). */
export interface ChartLayoutTexts {
  title?: string;
  xTitle?: string;
  yTitle?: string;
}

/** A chart template: type and formatting without data (`bo_sheet::chart_style::ChartTemplate`). */
export interface ChartTemplate {
  version?: number;
  name: string;
  /** Chart type name (`column`, `line`, …). */
  kind: string;
  /** Formatting (`bo_model::chart_style::ChartFormat`, opaque to the UI). */
  format?: Record<string, unknown>;
  legend?: string | null;
  title?: boolean;
  xTitle?: boolean;
  yTitle?: boolean;
  dataTable?: boolean;
  view3d?: unknown;
}

/**
 * What a chart design command applies (template, then style, colours, layout). A type alias, not
 * an interface: command unions intersect it and must stay assignable to `{ [k: string]: unknown }`.
 */
export type ChartDesignSpec = {
  template?: ChartTemplate;
  style?: string;
  colors?: string;
  layout?: string;
  texts?: ChartLayoutTexts;
};

/** A gallery preview: preset id and its thumbnail (SVG drawn by the shared chart renderer). */
export interface ChartPreview {
  id: string;
  svg: string;
}

/** Gallery of the Chart Design tab for one chart. */
export interface ChartGallery {
  styles: ChartPreview[];
  /** Colour variations: id and swatches (`#RRGGBB`). */
  colors: { id: string; colors: string[] }[];
  layouts: ChartPreview[];
  /** The chart's current formatting as a template (name empty). */
  template: ChartTemplate;
}
