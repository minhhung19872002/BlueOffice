import { resources, type Language } from '@blueoffice/i18n';

export type StartKey = keyof (typeof resources)['vi-VN']['translation']['startScreen'];
export type UpdatesKey = keyof (typeof resources)['vi-VN']['translation']['updates'];

/** `updates.*` string (key without the namespace, or a full `updates.x` key). */
export function updatesText(
  lang: Language,
  key: UpdatesKey | `updates.${UpdatesKey}`,
  vars: Record<string, string> = {},
): string {
  const k = key.replace(/^updates\./, '') as UpdatesKey;
  const s: string = resources[lang].translation.updates[k];
  return s.replace(/\{\{(\w+)\}\}/g, (_, v: string) => vars[v] ?? '');
}

/** `start.*` string with `{{var}}` interpolation (the Start screen has no i18next context). */
export function startText(
  lang: Language,
  key: StartKey,
  vars: Record<string, string> = {},
): string {
  const s: string = resources[lang].translation.startScreen[key];
  return s.replace(/\{\{(\w+)\}\}/g, (_, k: string) => vars[k] ?? '');
}
