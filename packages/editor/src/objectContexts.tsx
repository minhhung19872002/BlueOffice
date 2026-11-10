/**
 * Contextual tabs of BlueDoc for the object at the selection (CORE-UI-008): the Equation tab
 * while the caret is in (or right after) an equation, the Object tab while an embedded /
 * linked object is selected. The tabs only gather registry commands that already exist
 * (`insert.equation`, `equation.*`, `object.*`).
 */
import type { EditResult } from '@blueoffice/platform';
import type { RibbonContext, RibbonItem, RibbonTab } from '@blueoffice/ui';
import { Download, Pencil, RefreshCw, Sigma, SquareFunction, TextCursorInput } from 'lucide-react';
import type { ReactNode } from 'react';

type Item = (id: string, labelKey: string, icon: ReactNode, size?: 'large' | 'small') => RibbonItem;
type T = (key: string) => string;

/** Object contexts of the selection, from the last engine result. */
export function docObjectContexts(
  edit: Pick<EditResult, 'equation' | 'format'> | null | undefined,
): RibbonContext[] {
  const out: RibbonContext[] = [];
  if (edit?.equation) out.push('equation');
  if (edit?.format?.ole) out.push('object');
  return out;
}

const ICON = 20;

/** The contextual tabs of `contexts` (in that order). */
export function docObjectTabs(t: T, item: Item, contexts: readonly RibbonContext[]): RibbonTab[] {
  const tabs: RibbonTab[] = [];
  for (const c of contexts) {
    if (c === 'equation') {
      tabs.push({
        id: 'equationTools',
        context: 'equation',
        label: t('ribbon.objectTabs.equation'),
        groups: [
          {
            id: 'equationEdit',
            label: t('ribbon.objectTabs.groupEquation'),
            items: [item('insert.equation', 'commands.insert.equation', <Sigma size={ICON} />)],
          },
          {
            id: 'equationConvert',
            label: t('ribbon.objectTabs.groupConvert'),
            items: [
              item(
                'equation.professional',
                'commands.equation.professional',
                <SquareFunction size={ICON} />,
              ),
              item('equation.linear', 'commands.equation.linear', <TextCursorInput size={ICON} />),
            ],
          },
        ],
      });
    } else if (c === 'object') {
      tabs.push({
        id: 'objectTools',
        context: 'object',
        label: t('ribbon.objectTabs.object'),
        groups: [
          {
            id: 'objectEdit',
            label: t('ribbon.objectTabs.groupObject'),
            items: [
              item('object.edit', 'commands.object.edit', <Pencil size={ICON} />),
              item('object.saveCopy', 'commands.object.saveCopy', <Download size={ICON} />),
            ],
          },
          {
            id: 'objectLinks',
            label: t('ribbon.objectTabs.groupLinks'),
            items: [
              item('object.updateLinks', 'commands.object.updateLinks', <RefreshCw size={ICON} />),
            ],
          },
        ],
      });
    }
  }
  return tabs;
}
