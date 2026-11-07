import * as Tabs from '@radix-ui/react-tabs';
import * as Tooltip from '@radix-ui/react-tooltip';
import type { ReactNode } from 'react';
import styles from './Ribbon.module.css';

export interface RibbonItem {
  /** Command id from the command registry (e.g. `file.open`). */
  id: string;
  label: string;
  icon?: ReactNode;
  enabled?: boolean;
  checked?: boolean;
  size?: 'large' | 'small';
  tooltip?: string;
  shortcut?: string;
  onSelect: () => void;
  /** Custom control (combobox, colour picker…) shown instead of a button. */
  render?: ReactNode;
}

export interface RibbonGroup {
  id: string;
  label: string;
  items: RibbonItem[];
}

export interface RibbonTab {
  id: string;
  label: string;
  groups: RibbonGroup[];
}

export interface RibbonProps {
  tabs: RibbonTab[];
  value?: string;
  onValueChange?: (tab: string) => void;
  /** Content shown at the right of the tab strip (e.g. host Share/Close buttons). */
  trailing?: ReactNode;
}

function RibbonButton({ item }: { item: RibbonItem }) {
  const disabled = item.enabled === false;
  const button = (
    <button
      type="button"
      className={`${styles.button} ${item.size === 'small' ? styles.small : ''}`}
      data-command={item.id}
      aria-disabled={disabled}
      aria-pressed={item.checked ?? undefined}
      aria-keyshortcuts={item.shortcut}
      // Keep keyboard focus in the document (caret, IME) when clicking the ribbon.
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => {
        if (!disabled) item.onSelect();
      }}
    >
      {item.icon && <span className={styles.icon}>{item.icon}</span>}
      {/* Small buttons show only their icon; the label stays for screen readers and tests. */}
      <span className={item.size === 'small' && item.icon ? styles.srOnly : undefined}>
        {item.label}
      </span>
    </button>
  );
  const tip = [item.tooltip, item.shortcut].filter(Boolean).join(' · ');
  if (!tip) return button;
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>{button}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={styles.tooltip} sideOffset={4}>
          {tip}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

/** Tabbed toolbar. Every button dispatches a registry command. */
export function Ribbon({ tabs, value, onValueChange, trailing }: RibbonProps) {
  return (
    <Tooltip.Provider delayDuration={400}>
      <Tabs.Root
        className={styles.ribbon}
        value={value}
        defaultValue={tabs[0]?.id}
        onValueChange={onValueChange}
      >
        <div style={{ display: 'flex', background: 'var(--bo-brand)', alignItems: 'center' }}>
          <Tabs.List className={styles.tabList} style={{ flex: 1 }}>
            {tabs.map((t) => (
              <Tabs.Trigger key={t.id} value={t.id} className={styles.tab}>
                {t.label}
              </Tabs.Trigger>
            ))}
          </Tabs.List>
          {trailing}
        </div>
        {tabs.map((t) => (
          <Tabs.Content key={t.id} value={t.id} className={styles.panel}>
            {t.groups.map((g) => (
              <div key={g.id} className={styles.group} role="group" aria-label={g.label}>
                <div
                  className={`${styles.groupItems} ${
                    g.items.some((i) => i.render)
                      ? styles.mixed
                      : g.items.length > 3 && g.items.every((i) => i.size === 'small')
                        ? styles.compact
                        : ''
                  }`}
                >
                  {g.items.map((i) =>
                    i.render ? (
                      <div key={i.id} className={styles.control} data-control={i.id}>
                        {i.render}
                      </div>
                    ) : (
                      <RibbonButton key={i.id} item={i} />
                    ),
                  )}
                </div>
                <div className={styles.groupLabel}>{g.label}</div>
              </div>
            ))}
          </Tabs.Content>
        ))}
      </Tabs.Root>
    </Tooltip.Provider>
  );
}
