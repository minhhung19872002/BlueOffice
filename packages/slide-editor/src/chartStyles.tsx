/**
 * BlueSlide Chart Design groups (CORE-CHART-007): the shared chart design controls of
 * `@blueoffice/editor` (`src/chart/chartDesign.tsx`) on the slide command channel. Edits are
 * `setChart` commands (one undo step each); previews are drawn with the deck's theme.
 */
import { chartDesignGroups } from '@blueoffice/editor';
import type { useTranslation } from '@blueoffice/i18n';
import type { ChartGallery, SlideCommand, SlideEditResult } from '@blueoffice/platform';
import type { RibbonGroup } from '@blueoffice/ui';

export { SaveChartTemplateDialog, chartDesignLabels, layoutTexts } from '@blueoffice/editor';

type T = ReturnType<typeof useTranslation>['t'];

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
  return chartDesignGroups({
    t,
    load,
    apply: (spec) => run({ cmd: 'setChart', ...spec }),
    enabled,
    onSaveTemplate,
  });
}
