/**
 * Contextual tabs of BlueSlide for the selected object (CORE-UI-008):
 *  - Equation: the selected shape holds an equation (`equation` flag of the engine);
 *  - Object: the selected frame is an embedded / linked workbook (`kind: 'object'`);
 *  - Ink: a pen, highlighter or eraser of the Draw tab is in use (ink objects are content
 *    parts the engine does not select, so the tab follows the inking instead).
 * The tabs only gather registry commands that already exist (`arrange.*`, `edit.*`,
 * `object.*`, `draw.*`).
 */
import type { SlideShapeInfo } from '@blueoffice/platform';
import type { RibbonContext, RibbonItem, RibbonTab } from '@blueoffice/ui';
import {
  BringToFront,
  Copy,
  Download,
  Eraser,
  Highlighter,
  MousePointer2,
  Pencil,
  PenLine,
  Play,
  RefreshCw,
  Scissors,
  SendToBack,
  Trash,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { isChecked } from './draw/drawTab';
import { INITIAL_DRAW, type DrawState } from './draw/inkTools';

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonItem;
type T = (key: string) => string;

/** Object contexts of the selection and the Draw tool in use. */
export function slideObjectContexts(
  selection: readonly Pick<SlideShapeInfo, 'kind' | 'equation'>[] | null | undefined,
  tool: DrawState['tool'],
): RibbonContext[] {
  const out: RibbonContext[] = [];
  const one = selection?.length === 1 ? selection[0] : undefined;
  if (one?.equation) out.push('equation');
  if (one?.kind === 'object') out.push('object');
  if (tool !== 'select') out.push('ink');
  return out;
}

/** The contextual tabs of `contexts` (in that order); `draw` gives the pressed tool. */
export function slideObjectTabs(
  t: T,
  item: Item,
  contexts: readonly RibbonContext[],
  draw: DrawState = INITIAL_DRAW,
): RibbonTab[] {
  const sm = 'small' as const;
  // The Draw tools' pressed state comes from the tool state itself (the registry reads it
  // from a hub updated after the render).
  const tool = (id: string, label: string, icon: ReactNode, size?: 'large' | 'small') =>
    item(id, label, icon, size, isChecked(draw, id));
  const tabs: RibbonTab[] = [];
  for (const c of contexts) {
    if (c === 'equation') {
      tabs.push({
        id: 'equationTools',
        context: 'equation',
        label: t('ribbon.objectTabs.equation'),
        groups: [
          {
            id: 'equationArrange',
            label: t('ribbon.objectTabs.groupArrange'),
            items: [
              item('arrange.front', t('slideEditor.bringToFront'), <BringToFront size={16} />, sm),
              item('arrange.back', t('slideEditor.sendToBack'), <SendToBack size={16} />, sm),
            ],
          },
          {
            id: 'equationEdit',
            label: t('slideEditor.groupEdit'),
            items: [
              item('edit.duplicate', t('slideEditor.duplicate'), <Copy size={16} />, sm),
              item('edit.delete', t('slideEditor.delete'), <Trash size={16} />, sm),
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
              item('object.edit', t('commands.object.edit'), <Pencil size={18} />),
              item('object.saveCopy', t('commands.object.saveCopy'), <Download size={18} />),
            ],
          },
          {
            id: 'objectLinks',
            label: t('ribbon.objectTabs.groupLinks'),
            items: [
              item('object.updateLinks', t('commands.object.updateLinks'), <RefreshCw size={18} />),
            ],
          },
        ],
      });
    } else if (c === 'ink') {
      tabs.push({
        id: 'inkTools',
        context: 'ink',
        label: t('ribbon.objectTabs.ink'),
        groups: [
          {
            id: 'inkPens',
            label: t('ribbon.objectTabs.groupPens'),
            items: [
              tool('draw.pen', t('drawTools.pen'), <PenLine size={18} />),
              tool('draw.highlighter', t('drawTools.highlighter'), <Highlighter size={18} />),
              tool('draw.select', t('drawTools.select'), <MousePointer2 size={18} />),
            ],
          },
          {
            id: 'inkErase',
            label: t('ribbon.objectTabs.groupErase'),
            items: [
              tool('draw.eraserStroke', t('drawTools.eraserStroke'), <Eraser size={16} />, sm),
              tool('draw.eraserPoint', t('drawTools.eraserPoint'), <Scissors size={16} />, sm),
            ],
          },
          {
            id: 'inkReplay',
            label: t('ribbon.objectTabs.groupReplay'),
            items: [tool('draw.replay', t('drawTools.replay'), <Play size={18} />)],
          },
        ],
      });
    }
  }
  return tabs;
}
