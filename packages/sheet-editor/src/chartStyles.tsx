/**
 * BlueSheet Chart Design tab (CORE-CHART-007): chart style gallery, "Change colours", quick
 * layouts and chart templates (save as template, insert from a template, apply a template). The
 * presets are BlueOffice's own; previews are drawn by the engine's chart renderer. Every action
 * goes through the engine command bridge (`chartGallery`, `chartDesign`, `insertChart`), one undo
 * step each.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ChartGallery,
  ChartLayoutTexts,
  ChartTemplate,
  FileAccess,
  SheetCommand,
  SheetHost,
} from '@blueoffice/platform';
import {
  ChartColorsMenu,
  ChartLayoutMenu,
  ChartStyleGallery,
  Dialog,
  loadChartTemplates,
  saveChartTemplate,
  type ChartDesignLabels,
  type RibbonGroup,
} from '@blueoffice/ui';
import { Download, LayoutDashboard, Palette, Paintbrush, Save, Upload } from 'lucide-react';
import { useCallback, useState } from 'react';
import styles from './SheetEditor.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Prefix of the insert-chart menu items that insert from a saved template. */
export const TEMPLATE_ITEM = 'template:';

/** Labels of the shared chart design controls. */
export function chartDesignLabels(t: T): ChartDesignLabels {
  return {
    styles: t('chartDesign.styles'),
    colors: t('chartDesign.colors'),
    layouts: t('chartDesign.layouts'),
    loading: t('chartDesign.loading'),
    styleName: (id) => t(`chartDesign.style.${id}`),
    colorName: (id) => t(`chartDesign.palette.${id}`),
    layoutName: (id) => t(`chartDesign.layout.${id}`),
  };
}

/** Titles a quick layout or template adds (localised). */
export function layoutTexts(t: T): ChartLayoutTexts {
  return {
    title: t('chartDesign.titleText'),
    xTitle: t('chartDesign.xTitleText'),
    yTitle: t('chartDesign.yTitleText'),
  };
}

/** Insert-menu entries for the saved templates. */
export function templateMenuItems(t: T): { id: string; label: string }[] {
  return loadChartTemplates<ChartTemplate>().map((x) => ({
    id: `${TEMPLATE_ITEM}${x.name}`,
    label: t('chartDesign.fromTemplate', { name: x.name }),
  }));
}

/** The saved template behind an insert-menu entry. */
export function templateOfItem(id: string): ChartTemplate | null {
  if (!id.startsWith(TEMPLATE_ITEM)) return null;
  const name = id.slice(TEMPLATE_ITEM.length);
  return loadChartTemplates<ChartTemplate>().find((x) => x.name === name) ?? null;
}

/** Loads the gallery of chart `index` (memoised per revision by the caller's deps). */
export function useChartGallery(
  host: SheetHost | undefined,
  sheet: number,
  index: number | null,
  revision: number,
) {
  return useCallback(async (): Promise<ChartGallery | null> => {
    if (!host || index === null) return null;
    return host.exec<ChartGallery>({ cmd: 'chartGallery', sheet, index, width: 100, height: 68 });
    // The revision invalidates previews after edits.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, sheet, index, revision]);
}

/** Groups of the Chart Design contextual tab. */
export function chartDesignGroups({
  t,
  sheet,
  index,
  load,
  exec,
  onSaveTemplate,
  onExportTemplate,
  onImportTemplate,
}: {
  t: T;
  sheet: number;
  index: number | null;
  load: () => Promise<ChartGallery | null>;
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onSaveTemplate: () => void;
  /** Export the chart's formatting as a `.crtx` file. */
  onExportTemplate?: () => void;
  /** Import a `.crtx` file into the saved templates. */
  onImportTemplate?: () => void;
}): RibbonGroup[] {
  const labels = chartDesignLabels(t);
  const disabled = index === null;
  const design = (spec: { style?: string; colors?: string; layout?: string }) => {
    if (index === null) return;
    void exec({ cmd: 'chartDesign', sheet, index, ...spec, texts: layoutTexts(t) });
  };
  const none = () => {};
  return [
    {
      id: 'chartLayouts',
      label: t('chartDesign.groupLayouts'),
      items: [
        {
          id: 'chart.quickLayout',
          label: labels.layouts,
          onSelect: none,
          enabled: !disabled,
          render: (
            <ChartLayoutMenu
              load={load}
              labels={labels}
              disabled={disabled}
              icon={<LayoutDashboard size={18} aria-hidden="true" />}
              onPick={(layout) => design({ layout })}
            />
          ),
        },
      ],
    },
    {
      id: 'chartStyles',
      label: t('chartDesign.groupStyles'),
      items: [
        {
          id: 'chart.colors',
          label: labels.colors,
          onSelect: none,
          enabled: !disabled,
          render: (
            <ChartColorsMenu
              load={load}
              labels={labels}
              disabled={disabled}
              icon={<Palette size={18} aria-hidden="true" />}
              onPick={(colors) => design({ colors })}
            />
          ),
        },
        {
          id: 'chart.styles',
          label: labels.styles,
          onSelect: none,
          enabled: !disabled,
          render: (
            <ChartStyleGallery
              load={load}
              labels={labels}
              disabled={disabled}
              icon={<Paintbrush size={18} aria-hidden="true" />}
              onPick={(style) => design({ style })}
            />
          ),
        },
      ],
    },
    {
      id: 'chartTemplates',
      label: t('chartDesign.groupTemplates'),
      items: [
        {
          id: 'chart.saveTemplate',
          label: t('chartDesign.saveTemplate'),
          icon: <Save size={18} aria-hidden="true" />,
          size: 'large',
          enabled: !disabled,
          onSelect: onSaveTemplate,
        },
        ...(onExportTemplate
          ? [
              {
                id: 'chart.exportTemplate',
                label: t('chartDesign.exportTemplate'),
                icon: <Download size={16} aria-hidden="true" />,
                size: 'small' as const,
                enabled: !disabled,
                onSelect: onExportTemplate,
              },
            ]
          : []),
        ...(onImportTemplate
          ? [
              {
                id: 'chart.importTemplate',
                label: t('chartDesign.importTemplate'),
                icon: <Upload size={16} aria-hidden="true" />,
                size: 'small' as const,
                enabled: true,
                onSelect: onImportTemplate,
              },
            ]
          : []),
      ],
    },
  ];
}

function toBase64(bytes: Uint8Array): string {
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** MIME type of a chart template package. */
const CRTX_MIME = 'application/vnd.openxmlformats-officedocument.drawingml.chart+xml';

/** Saves the chart's template as a `.crtx` package; resolves the saved name (`null`: cancelled). */
export async function exportChartTemplate(
  load: () => Promise<ChartGallery | null>,
  host: SheetHost,
  files: FileAccess,
  name: string,
): Promise<string | null> {
  const g = await load();
  if (!g) return null;
  const r = await host.exec<{ base64: string; name: string }>({
    cmd: 'chartTemplateExport',
    template: { ...g.template, name },
  });
  return files.saveFile(fromBase64(r.base64), r.name, CRTX_MIME);
}

/** Reads a `.crtx` file chosen by the user into the saved templates; resolves its name. */
export async function importChartTemplate(
  host: SheetHost,
  files: FileAccess,
): Promise<string | null> {
  const f = await files.openFile(['.crtx']);
  if (!f) return null;
  const tpl = await host.exec<ChartTemplate>({
    cmd: 'chartTemplateImport',
    base64: toBase64(f.bytes),
    name: f.name,
  });
  saveChartTemplate<ChartTemplate>(tpl);
  return tpl.name;
}

/** "Save as template…": names the chart's current formatting and keeps it. */
export function SaveChartTemplateDialog({
  load,
  onClose,
  onSaved,
}: {
  load: () => Promise<ChartGallery | null>;
  onClose: () => void;
  onSaved: (name: string) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const ok = async () => {
    const n = name.trim();
    if (!n) return;
    const g = await load();
    if (!g) return;
    saveChartTemplate<ChartTemplate>({ ...g.template, name: n });
    onSaved(n);
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('chartDesign.saveTemplateTitle')}
      testId="chart-template-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="chart-template-ok"
            disabled={!name.trim()}
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('chartDesign.templateName')}
          <input
            data-testid="chart-template-name"
            value={name}
            autoFocus
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void ok();
            }}
          />
        </label>
      </div>
    </Dialog>
  );
}
