import type { ComponentType, SVGProps } from 'react';

/** Props every icon drawing accepts (lucide-compatible subset). */
export interface IconDrawingProps extends SVGProps<SVGSVGElement> {
  size?: number | string;
  strokeWidth?: number | string;
}

/** Where an icon comes from — recorded in SOURCES.md. */
export type IconSource = 'lucide' | 'bluestar';

export interface IconSpec {
  svg: ComponentType<IconDrawingProps>;
  source: IconSource;
}
