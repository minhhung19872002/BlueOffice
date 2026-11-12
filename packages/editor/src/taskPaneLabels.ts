import type { TaskPaneLabels } from '@blueoffice/ui';

type T = (key: string) => string;

/** Translated strings of the shared task pane host (CORE-UI-006), for all three editors. */
export function taskPaneLabels(t: T): TaskPaneLabels {
  return {
    dockLeft: t('taskPane.dockLeft'),
    dockRight: t('taskPane.dockRight'),
    float: t('taskPane.float'),
    close: t('taskPane.close'),
    resize: t('taskPane.resize'),
    move: t('taskPane.move'),
    moveKeys: t('taskPane.moveKeys'),
    tabs: t('taskPane.tabs'),
  };
}
