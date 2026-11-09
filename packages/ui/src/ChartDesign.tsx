import { useEffect, useState, type ReactNode } from 'react';
import { Popover } from './Popover';
import styles from './ChartDesign.module.css';

/**
 * Chart Design controls (CORE-CHART-007) shared by BlueSheet and BlueSlide: the chart style
 * gallery, the colour variation menu, the quick layout menu and the template list. Previews are
 * SVG thumbnails drawn by the engine's chart renderer; every string comes from the host (i18n).
 */

/** A preset with its thumbnail. */
export interface ChartPresetPreview {
  id: string;
  svg: string;
}

/** Gallery data of one chart (engine `chartGallery`). */
export interface ChartDesignGalleryData {
  styles: ChartPresetPreview[];
  colors: { id: string; colors: string[] }[];
  layouts: ChartPresetPreview[];
}

export interface ChartDesignLabels {
  styles: string;
  colors: string;
  layouts: string;
  loading: string;
  styleName: (id: string) => string;
  colorName: (id: string) => string;
  layoutName: (id: string) => string;
}

interface DropdownProps {
  /** Loads the gallery of the selected chart (when the dropdown opens). */
  load: () => Promise<ChartDesignGalleryData | null>;
  onPick: (id: string) => void;
  labels: ChartDesignLabels;
  disabled?: boolean;
  /** Content of the trigger button (icon). */
  icon?: ReactNode;
}

const svgUrl = (svg: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;

function useGallery(load: DropdownProps['load']) {
  const [data, setData] = useState<ChartDesignGalleryData | null>(null);
  useEffect(() => {
    let live = true;
    load()
      .then((g) => {
        if (live) setData(g);
      })
      .catch(() => {
        if (live) setData(null);
      });
    return () => {
      live = false;
    };
  }, [load]);
  return data;
}

function PreviewGrid({
  load,
  pick,
  kind,
  name,
  loading,
  testId,
}: {
  load: DropdownProps['load'];
  pick: (id: string) => void;
  kind: 'styles' | 'layouts';
  name: (id: string) => string;
  loading: string;
  testId: string;
}) {
  const data = useGallery(load);
  if (!data) return <div className={styles.loading}>{loading}</div>;
  return (
    <div className={styles.grid} role="listbox" data-testid={testId}>
      {data[kind].map((p) => (
        <button
          key={p.id}
          type="button"
          role="option"
          aria-selected={false}
          className={styles.preview}
          title={name(p.id)}
          aria-label={name(p.id)}
          data-testid={`${testId}-${p.id}`}
          onClick={() => pick(p.id)}
        >
          <img src={svgUrl(p.svg)} alt="" draggable={false} />
        </button>
      ))}
    </div>
  );
}

/** "Chart styles" gallery dropdown. */
export function ChartStyleGallery({ load, onPick, labels, disabled, icon }: DropdownProps) {
  return (
    <Popover
      trigger={icon ?? <span>{labels.styles}</span>}
      label={labels.styles}
      disabled={disabled}
      testId="chart-styles"
    >
      {(close) => (
        <PreviewGrid
          load={load}
          kind="styles"
          name={labels.styleName}
          loading={labels.loading}
          testId="chart-styles-grid"
          pick={(id) => {
            close();
            onPick(id);
          }}
        />
      )}
    </Popover>
  );
}

/** "Quick layout" dropdown. */
export function ChartLayoutMenu({ load, onPick, labels, disabled, icon }: DropdownProps) {
  return (
    <Popover
      trigger={icon ?? <span>{labels.layouts}</span>}
      label={labels.layouts}
      disabled={disabled}
      testId="chart-layouts"
    >
      {(close) => (
        <PreviewGrid
          load={load}
          kind="layouts"
          name={labels.layoutName}
          loading={labels.loading}
          testId="chart-layouts-grid"
          pick={(id) => {
            close();
            onPick(id);
          }}
        />
      )}
    </Popover>
  );
}

function ColorList({
  load,
  pick,
  labels,
}: {
  load: DropdownProps['load'];
  pick: (id: string) => void;
  labels: ChartDesignLabels;
}) {
  const data = useGallery(load);
  if (!data) return <div className={styles.loading}>{labels.loading}</div>;
  return (
    <div className={styles.list} role="menu" data-testid="chart-colors-list">
      {data.colors.map((c) => (
        <button
          key={c.id}
          type="button"
          role="menuitem"
          className={styles.colorRow}
          data-testid={`chart-colors-${c.id}`}
          onClick={() => pick(c.id)}
        >
          <span className={styles.swatches} aria-hidden="true">
            {c.colors.slice(0, 6).map((hex, i) => (
              <span key={i} className={styles.swatch} style={{ background: hex }} />
            ))}
          </span>
          <span>{labels.colorName(c.id)}</span>
        </button>
      ))}
    </div>
  );
}

/** "Change colours" dropdown. */
export function ChartColorsMenu({ load, onPick, labels, disabled, icon }: DropdownProps) {
  return (
    <Popover
      trigger={icon ?? <span>{labels.colors}</span>}
      label={labels.colors}
      disabled={disabled}
      testId="chart-colors"
    >
      {(close) => (
        <ColorList
          load={load}
          labels={labels}
          pick={(id) => {
            close();
            onPick(id);
          }}
        />
      )}
    </Popover>
  );
}

// ------------------------------------------------------------------ templates

/** Storage key of the user's chart templates (per browser / desktop profile). */
export const CHART_TEMPLATES_KEY = 'blueoffice.chartTemplates';

/** A stored chart template (only the name is known to the UI kit). */
export interface NamedChartTemplate {
  name: string;
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The saved chart templates. */
export function loadChartTemplates<T extends NamedChartTemplate>(): T[] {
  try {
    const raw = storage()?.getItem(CHART_TEMPLATES_KEY);
    const list: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(list)
      ? (list.filter(
          (t): t is T => typeof t === 'object' && t !== null && typeof (t as T).name === 'string',
        ) as T[])
      : [];
  } catch {
    return [];
  }
}

function store<T extends NamedChartTemplate>(list: T[]) {
  try {
    storage()?.setItem(CHART_TEMPLATES_KEY, JSON.stringify(list));
  } catch {
    // Storage full or unavailable: the template is not kept.
  }
}

/** Saves a template (replacing one with the same name); returns the new list. */
export function saveChartTemplate<T extends NamedChartTemplate>(t: T): T[] {
  const list = loadChartTemplates<T>().filter((x) => x.name !== t.name);
  list.push(t);
  list.sort((a, b) => a.name.localeCompare(b.name));
  store(list);
  return list;
}

/** Removes the template named `name`; returns the new list. */
export function deleteChartTemplate<T extends NamedChartTemplate>(name: string): T[] {
  const list = loadChartTemplates<T>().filter((x) => x.name !== name);
  store(list);
  return list;
}

export interface ChartTemplateListLabels {
  title: string;
  none: string;
  noTemplate: string;
  remove: string;
}

/** Template picker of the insert / change chart type dialogs. */
export function ChartTemplateList<T extends NamedChartTemplate>({
  templates,
  value,
  onChange,
  onDelete,
  labels,
}: {
  templates: T[];
  /** Selected template name; `null` = none. */
  value: string | null;
  onChange: (name: string | null) => void;
  onDelete?: (name: string) => void;
  labels: ChartTemplateListLabels;
}) {
  return (
    <fieldset className={styles.templates} data-testid="chart-templates">
      <legend>{labels.title}</legend>
      {templates.length === 0 ? (
        <p className={styles.loading}>{labels.none}</p>
      ) : (
        <>
          <label className={styles.templateRow}>
            <input
              type="radio"
              name="chart-template"
              checked={value === null}
              onChange={() => onChange(null)}
            />
            <span>{labels.noTemplate}</span>
          </label>
          {templates.map((t) => (
            <label key={t.name} className={styles.templateRow}>
              <input
                type="radio"
                name="chart-template"
                checked={value === t.name}
                data-testid={`chart-template-${t.name}`}
                onChange={() => onChange(t.name)}
              />
              <span className={styles.templateName}>{t.name}</span>
              {onDelete && (
                <button
                  type="button"
                  className={styles.remove}
                  aria-label={`${labels.remove}: ${t.name}`}
                  title={labels.remove}
                  onClick={() => onDelete(t.name)}
                >
                  ×
                </button>
              )}
            </label>
          ))}
        </>
      )}
    </fieldset>
  );
}
