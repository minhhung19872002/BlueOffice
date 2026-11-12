import {
  FluentProvider,
  createDarkTheme,
  createHighContrastTheme,
  createLightTheme,
  type BrandVariants,
  type Theme,
} from '@fluentui/react-components';
import type { ComponentProps, ReactNode } from 'react';
import { ACCENTS, type AppId, type ThemeName } from './tokens/tokens';

/** BlueOffice's contrast-safe app accent at step 80, with darker and lighter steps. */
export function blueOfficeBrand(app: AppId): BrandVariants {
  const base = ACCENTS.light[app].strong;
  const rgb = [1, 3, 5].map((i) => Number.parseInt(base.slice(i, i + 2), 16));
  const steps = Array.from({ length: 16 }, (_, i) => {
    const step = (i + 1) * 10;
    const factor = step <= 80 ? step / 80 : (step - 80) / 80;
    const channels = rgb.map((c) =>
      Math.round(step <= 80 ? c * factor : c + (255 - c) * factor)
        .toString(16)
        .padStart(2, '0'),
    );
    return [step, `#${channels.join('')}`];
  });
  return Object.fromEntries(steps) as BrandVariants;
}

const apps: AppId[] = ['doc', 'sheet', 'slide'];
const withBlueOfficeFonts = (theme: Theme): Theme => ({
  ...theme,
  fontFamilyBase: 'var(--bo-font)',
  fontFamilyNumeric: 'var(--bo-font)',
});
const themes = Object.fromEntries(
  apps.map((app) => {
    const brand = blueOfficeBrand(app);
    return [
      app,
      {
        light: withBlueOfficeFonts(createLightTheme(brand)),
        dark: withBlueOfficeFonts(createDarkTheme(brand)),
        hc: withBlueOfficeFonts(createHighContrastTheme()),
      },
    ];
  }),
) as Record<AppId, Record<ThemeName, Theme>>;

/** Stable theme instances keep editor updates from changing the Fluent context. */
export function blueOfficeFluentTheme(app: AppId, mode: ThemeName): Theme {
  return themes[app][mode];
}

/** Fluent context shares the existing editor root; no additional layout wrapper is inserted. */
export function BlueFluentProvider({
  app,
  mode,
  children,
  className,
  ...props
}: Omit<ComponentProps<typeof FluentProvider>, 'theme'> & {
  app: AppId;
  mode: ThemeName;
  children?: ReactNode;
}) {
  return (
    <FluentProvider
      theme={blueOfficeFluentTheme(app, mode)}
      className={`bo-fluent-root ${className ?? ''}`}
      {...props}
    >
      {children}
    </FluentProvider>
  );
}
