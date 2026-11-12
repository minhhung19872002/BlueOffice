import { CUSTOM_ICONS } from './custom';
import { LUCIDE_ICONS } from './lucide';
import type { IconSpec } from './types';

/** All icons by command id (custom drawings win over lucide on the same id). */
export const ICONS: Readonly<Record<string, IconSpec>> = { ...LUCIDE_ICONS, ...CUSTOM_ICONS };

/** Sizes of the icon system: 16 (menus), 20 (small ribbon buttons), 24, 32 (large ribbon buttons). */
export type IconSize = 16 | 20 | 24 | 32;

export interface IconProps {
  /** Command id (`format.bold`, `insert.table`…). */
  name: string;
  size?: IconSize;
  /** Accessible label; omitted ⇒ decorative (aria-hidden). */
  label?: string;
  className?: string;
}

export function hasIcon(name: string): boolean {
  return Object.prototype.hasOwnProperty.call(ICONS, name);
}

/**
 * One icon of the BlueOffice set. The stroke is 1.5 at 24px and scales with the size; the accent
 * part (custom drawings) follows `--bo-app-accent`.
 */
export function Icon({ name, size = 20, label, className }: IconProps) {
  const spec = ICONS[name];
  if (!spec) return null;
  const Svg = spec.svg;
  return (
    <Svg
      size={size}
      strokeWidth={1.5}
      className={['bo-icon', className].filter(Boolean).join(' ')}
      data-icon={name}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      focusable="false"
    />
  );
}

/** SOURCES.md content: one row per icon id with its origin. */
export function renderSourcesMd(): string {
  const rows = Object.entries(ICONS)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([id, s]) => {
      const name = s.svg.displayName ?? (s.svg as { name?: string }).name ?? '';
      const origin =
        s.source === 'lucide'
          ? `Lucide (ISC) \`${name}\``
          : `Bluestar — tự vẽ (\`custom.tsx\` ${name})`;
      return `| \`${id}\` | ${origin} |`;
    });
  return [
    '# Nguồn gốc icon',
    '',
    'Sinh từ `Icon.tsx` (`BO_WRITE_ICONS=1 pnpm --filter @blueoffice/ui test`). Không icon nào được vẽ theo icon của sản',
    'phẩm khác. Lucide: <https://lucide.dev>, giấy phép ISC (THIRD_PARTY_NOTICES.md).',
    '',
    'Các icon Lưu, Hoàn tác và Làm lại trong thanh truy cập nhanh dùng Fluent System Icons',
    '(`@fluentui/react-icons`, MIT, <https://github.com/microsoft/fluentui-system-icons>).',
    'Icon và logo sản phẩm BlueOffice vẫn là tài sản riêng của dự án.',
    '',
    '| Command id | Nguồn |',
    '|---|---|',
    ...rows,
    '',
  ].join('\n');
}
