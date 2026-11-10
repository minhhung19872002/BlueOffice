import type { ReactNode } from 'react';
import { Icon } from './icons';
import styles from './TitleBar.module.css';

/** One quick-access command (Save, Undo, Redo, …) drawn as an icon button. */
export interface QuickAccessItem {
  /** Command id; also the icon name unless `icon` is set. */
  id: string;
  /** Icon name when it differs from the command id (a bound script: `script.run`). */
  icon?: string;
  label: string;
  shortcut?: string;
  enabled?: boolean;
  onSelect: () => void;
}

export type SaveState = 'saved' | 'saving' | 'unsaved';

export interface TitleBarProps {
  /** App id: picks the app mark and (through `data-app` on the root) the accent. */
  app: 'doc' | 'sheet' | 'slide';
  /** App name shown before the document, e.g. "BlueSheet" (or "BlueDrive · BlueSheet"). */
  appName: string;
  /** Document name; empty when nothing is open. */
  docName?: string;
  saveState?: SaveState | null;
  /** Localized text of the save state ("Đã lưu", "Đang lưu…", "Chưa lưu"). */
  saveLabel?: string;
  quickAccess?: QuickAccessItem[];
  /** Accessible name of the quick-access toolbar. */
  quickAccessLabel: string;
  /** Optional search box opener (command search, UI-FRAME-003). */
  search?: { label: string; shortcut?: string; onOpen: () => void };
  /** Right side: collaborators, Comments, Share, account, host buttons. */
  actions?: ReactNode;
}

/**
 * The Quick Access Toolbar's buttons: in the title bar, or in its own row below the ribbon
 * (`below`, CORE-UI-005).
 */
export function QuickAccessBar({
  items,
  label,
  below,
}: {
  items: readonly QuickAccessItem[];
  label: string;
  below?: boolean;
}) {
  return (
    <div
      className={below ? `${styles.qat} ${styles.qatBelow}` : styles.qat}
      role="toolbar"
      aria-label={label}
      data-testid={below ? 'qat-below' : 'qat'}
    >
      {items.map((q) => (
        <button
          key={q.id}
          type="button"
          className={styles.qatButton}
          data-command={`qat:${q.id}`}
          aria-label={q.label}
          title={q.shortcut ? `${q.label} (${q.shortcut})` : q.label}
          disabled={q.enabled === false}
          onClick={q.onSelect}
        >
          <Icon name={q.icon ?? q.id} size={16} />
        </button>
      ))}
    </div>
  );
}

/**
 * The 40px title bar shared by the three apps (docs/ui/DESIGN.md "Khung cửa sổ"): app mark and
 * quick-access commands on the left, document name with its save state in the middle, search
 * and actions on the right. It sits on the app accent band (`--bo-app-accent-strong`).
 */
export function TitleBar(p: TitleBarProps) {
  return (
    <header className={styles.bar} data-testid="title-bar">
      <span className={styles.app}>
        <Icon name={`app.${p.app}`} size={20} />
        <span className={styles.appName}>{p.appName}</span>
      </span>
      {p.quickAccess && p.quickAccess.length > 0 && (
        <QuickAccessBar items={p.quickAccess} label={p.quickAccessLabel} />
      )}
      <span className={styles.doc}>
        <span className={styles.docName} data-testid="doc-name">
          {p.docName ?? ''}
        </span>
        {p.docName && p.saveState && p.saveLabel && (
          <span className={styles.save} data-state={p.saveState} data-testid="save-state">
            {p.saveLabel}
          </span>
        )}
      </span>
      {p.search && (
        <button
          type="button"
          className={styles.search}
          data-testid="title-search"
          onClick={p.search.onOpen}
          aria-keyshortcuts={p.search.shortcut}
        >
          <Icon name="edit.find" size={16} />
          <span>{p.search.label}</span>
        </button>
      )}
      <span className={styles.actions}>{p.actions}</span>
    </header>
  );
}
