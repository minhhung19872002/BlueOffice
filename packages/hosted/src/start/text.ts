import { resources, type Language } from '@blueoffice/i18n';

export type StartKey = keyof (typeof resources)['vi-VN']['translation']['startScreen'];

/** `start.*` string with `{{var}}` interpolation (the Start screen has no i18next context). */
export function startText(
  lang: Language,
  key: StartKey,
  vars: Record<string, string> = {},
): string {
  const s: string = resources[lang].translation.startScreen[key];
  return s.replace(/\{\{(\w+)\}\}/g, (_, k: string) => vars[k] ?? '');
}
