// BlueOffice's own icon drawings (source "bluestar" in SOURCES.md), on a 24×24 grid, 1.5 stroke,
// two tones: strokes in currentColor, one accent part in the app accent (`.bo-icon-accent`, which
// turns monochrome in high contrast). Drawn from scratch — not traced from any other product.
import type { IconDrawingProps, IconSpec } from './types';

function frame(children: React.ReactNode, label: string) {
  const Drawing = ({ size = 24, strokeWidth = 1.5, ...rest }: IconDrawingProps) => (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...rest}
    >
      {children}
    </svg>
  );
  Drawing.displayName = label;
  return Drawing;
}

const own = (svg: IconSpec['svg']): IconSpec => ({ svg, source: 'bluestar' });

/**
 * App marks (drafts, docs/parity/NEEDS_HUMAN.md UI-BRAND-002): a shared rounded hexagon-ish
 * "tile with a folded top-right notch" frame, a different inner sign per app.
 */
const TILE = 'M7 3h7.5L20 8.5V18a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3z';

export const CUSTOM_ICONS: Record<string, IconSpec> = {
  'app.doc': own(
    frame(
      <>
        <path d={TILE} className="bo-icon-accent" fill="currentColor" stroke="none" />
        <path d="M8 10h8M8 13.5h8M8 17h5" stroke="var(--bo-icon-on)" />
      </>,
      'AppDoc',
    ),
  ),
  'app.sheet': own(
    frame(
      <>
        <path d={TILE} className="bo-icon-accent" fill="currentColor" stroke="none" />
        <path d="M8 10h8v7H8zM12 10v7M8 13.5h8" stroke="var(--bo-icon-on)" />
      </>,
      'AppSheet',
    ),
  ),
  'app.slide': own(
    frame(
      <>
        <path d={TILE} className="bo-icon-accent" fill="currentColor" stroke="none" />
        <path d="M10 9.5v7l6-3.5z" stroke="var(--bo-icon-on)" fill="var(--bo-icon-on)" />
      </>,
      'AppSlide',
    ),
  ),
  // Number format: thousands separator "000" with a comma accent.
  'format.thousands': own(
    frame(
      <>
        <path d="M5 9.5a2 2.5 0 1 0 0.01 0M12 9.5a2 2.5 0 1 0 0.01 0M19 9.5a2 2.5 0 1 0 0.01 0" />
        <path d="M8.6 16l-1 2.5" className="bo-icon-accent" />
      </>,
      'Thousands',
    ),
  ),
  // Dong currency sign for the Vietnamese currency format.
  'format.currencyVnd': own(
    frame(
      <>
        <path d="M14 4v11M11.5 6.5h5" />
        <path d="M14 11.5a3 3 0 1 0 0 3.5" />
        <path d="M7 19h10" className="bo-icon-accent" />
      </>,
      'CurrencyVnd',
    ),
  ),
};
