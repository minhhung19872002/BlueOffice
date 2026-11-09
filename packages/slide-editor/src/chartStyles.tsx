/**
 * BlueSlide Chart Design groups (CORE-CHART-007): chart style gallery, "Change colours", quick
 * layouts and "Save as template…" for the selected chart. The presets are BlueOffice's own;
 * previews are drawn by the engine's chart renderer with the deck's theme. Edits are `setChart`
 * commands (one undo step each).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ChartGallery,
  ChartLayoutTexts,
  ChartTemplate,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import {
  ChartColorsMenu,
  ChartLayoutMenu,
  ChartStyleGallery,
  Dialog,
  saveChartTemplate,
  type ChartDesignLabels,
  type RibbonGroup,
} from '@blueoffice/ui';
import { LayoutDashboard, Palette, Paintbrush, Save } from 'lucide-react';
import { useState } from 'react';

type T = ReturnType<typeof useTranslation>['t'];

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

/** Gallery of the selected chart through the slide command channel. */
export function slideGalleryLoader(
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>,
): () => Promise<ChartGallery | null> {
  return async () =>
    (await run({ cmd: 'chartGallery', width: 100, height: 68 }))?.chartGallery ?? null;
}

/** Chart Design groups of the chart contextual tab. */
export function slideChartDesignGroups({
  t,
  load,
  run,
  enabled,
  onSaveTemplate,
}: {
  t: T;
  load: () => Promise<ChartGallery | null>;
  run: (cmd: SlideCommand) => void;
  enabled: boolean;
  onSaveTemplate: () => void;
}): RibbonGroup[] {
  const labels = chartDesignLabels(t);
  const design = (spec: { style?: string; colors?: string; layout?: string }) =>
    run({ cmd: 'setChart', ...spec, texts: layoutTexts(t) });
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
          enabled,
          render: (
            <ChartLayoutMenu
              load={load}
              labels={labels}
              disabled={!enabled}
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
          enabled,
          render: (
            <ChartColorsMenu
              load={load}
              labels={labels}
              disabled={!enabled}
              icon={<Palette size={18} aria-hidden="true" />}
              onPick={(colors) => design({ colors })}
            />
          ),
        },
        {
          id: 'chart.styles',
          label: labels.styles,
          onSelect: none,
          enabled,
          render: (
            <ChartStyleGallery
              load={load}
              labels={labels}
              disabled={!enabled}
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
          enabled,
          onSelect: onSaveTemplate,
        },
      ],
    },
  ];
}

/** "Save as template…": names the selected chart's formatting and keeps it. */
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
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="chart-template-ok"
            disabled={!name.trim()}
            onClick={() => void ok()}
          >
            {t('slideEditor.ok')}
          </button>
        </>
      }
    >
      <label>
        {t('chartDesign.templateName')}{' '}
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
    </Dialog>
  );
}
