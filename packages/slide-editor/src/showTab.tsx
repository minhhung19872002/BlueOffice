import type { RibbonTab } from '@blueoffice/ui';
import {
  Cast,
  Circle,
  CircleOff,
  Eraser,
  Mic,
  Link2,
  ListVideo,
  MonitorPlay,
  Play,
  Presentation,
  Settings2,
  Timer,
  TimerOff,
} from 'lucide-react';
import type { ReactNode } from 'react';

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

/**
 * The Slide Show tab (SLIDE-SHOW-001): starting the show, custom shows, setup,
 * rehearsal and the audience screen. Commands are registered by the editor
 * (`show.*`).
 */
export function slideShowTab(
  t: (key: string, o?: Record<string, unknown>) => string,
  item: Item,
  useTimings: boolean,
  narration = false,
): RibbonTab {
  const sm = 'small' as const;
  return {
    id: 'slideShow',
    label: t('showTools.tab'),
    groups: [
      {
        id: 'showStart',
        label: t('showTools.groupStart'),
        items: [
          item('show.fromStart', t('showTools.fromStart'), <Play size={18} />),
          item('show.fromCurrent', t('showTools.fromCurrent'), <MonitorPlay size={16} />, sm),
          item('show.presenter', t('showTools.presenter'), <Presentation size={16} />, sm),
          item('show.customShows', t('showTools.customShows'), <ListVideo size={16} />, sm),
        ],
      },
      {
        id: 'showSetup',
        label: t('showTools.groupSetup'),
        items: [
          item('show.setup', t('showTools.setup'), <Settings2 size={18} />),
          item('show.rehearse', t('showTools.rehearse'), <Timer size={16} />, sm),
          item('show.useTimings', t('showTools.useTimings'), <Timer size={16} />, sm, useTimings),
          item('show.linkCustom', t('showTools.linkShape'), <Link2 size={16} />, sm),
        ],
      },
      {
        // Recording the show (SLIDE-REC-001).
        id: 'showRecord',
        label: t('showRecord.groupRecord'),
        items: [
          item('show.recordFromCurrent', t('showRecord.recordFromCurrent'), <Circle size={18} />),
          item('show.recordFromStart', t('showRecord.recordFromStart'), <Circle size={16} />, sm),
          item(
            'show.playNarration',
            t('showRecord.playNarration'),
            <Mic size={16} />,
            sm,
            narration,
          ),
          item(
            'show.clearRecordingCurrent',
            t('showRecord.clearRecordingCurrent'),
            <Eraser size={16} />,
            sm,
          ),
          item(
            'show.clearRecordingAll',
            t('showRecord.clearRecordingAll'),
            <CircleOff size={16} />,
            sm,
          ),
          item(
            'show.clearTimingCurrent',
            t('showRecord.clearTimingCurrent'),
            <TimerOff size={16} />,
            sm,
          ),
          item('show.clearTimingAll', t('showRecord.clearTimingAll'), <TimerOff size={16} />, sm),
        ],
      },
      {
        id: 'showScreens',
        label: t('showTools.groupScreens'),
        items: [item('show.audience', t('showTools.audience'), <Cast size={18} />)],
      },
    ],
  };
}
