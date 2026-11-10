import { useTranslation } from '@blueoffice/i18n';
import type {
  SlideActionInfo,
  SlideActionSpec,
  SlideActionTrigger,
  SlideCustomShow,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState, type ReactNode } from 'react';
import {
  ACTION_BUTTONS,
  BUILTIN_SOUNDS,
  JUMP_KINDS,
  base64Of,
  builtinOf,
  builtinWav,
  draftOf,
  linkDraftOf,
  linkTargetOf,
  specOf,
  targetOf,
  type ActionDraft,
  type ActionPolicy,
  type BuiltinSound,
  type LinkDraft,
  type LinkKind,
  type LinkPlace,
  type LinkToKind,
  type SoundChoice,
} from './actions';
import styles from './actions.module.css';
import forms from './showTools.module.css';

/** "3. Title" of a slide in the pickers. */
function slideName(i: number, titles: string[]): string {
  const title = titles[i]?.split('\n')[0]?.trim();
  return title ? `${i + 1}. ${title}` : `${i + 1}`;
}

// ------------------------------------------------------------------ Insert › Link

export interface LinkDialogProps {
  /** Actions of the selection (`actionInfo`); the dialog edits the click action. */
  info: SlideActionInfo;
  /** Slides of the deck. */
  count: number;
  customShows: SlideCustomShow[];
  /** A picture of slide `i`, `width` px wide. */
  thumb?: (slide: number, width: number) => ReactNode;
  /** Message code of a refused command. */
  error?: string;
  onClose: () => void;
  onApply: (spec: SlideActionSpec) => void;
  /** Remove the link. */
  onRemove: () => void;
}

const LINK_KINDS: LinkKind[] = ['url', 'place', 'show', 'mail', 'file'];
const PLACES = ['first', 'prev', 'next', 'last'] as const;

/**
 * Insert › Link: a web address, a place in this deck (slide picker with
 * thumbnails), a custom show, an e-mail address or a file — on the selected
 * shapes or the selected text (§21.1.2.3.5 hlinkClick).
 */
export function LinkDialog({
  info,
  count,
  customShows,
  thumb,
  error,
  onClose,
  onApply,
  onRemove,
}: LinkDialogProps) {
  const { t } = useTranslation();
  const [d, setD] = useState<LinkDraft>(() => linkDraftOf(info.click, customShows[0]?.id ?? null));
  const target = linkTargetOf(d);
  const locked = !!info.click?.locked;
  const has = !!info.click && info.click.target.kind !== 'none' && !locked;
  const samePlace = (p: LinkPlace) =>
    d.place.kind === p.kind &&
    (p.kind !== 'slide' || (d.place as { slide: number }).slide === p.slide);
  const place = (p: LinkPlace, label: ReactNode, testId: string, extra = '') => (
    <button
      key={testId}
      type="button"
      className={`${styles.place} ${extra} ${samePlace(p) ? styles.placeOn : ''}`}
      aria-pressed={samePlace(p)}
      data-testid={testId}
      onClick={() => setD({ ...d, place: p })}
    >
      {label}
    </button>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideActions.linkTitle')}
      testId="link-dialog"
      width={600}
      footer={
        <>
          {has && (
            <button type="button" data-testid="link-remove" onClick={onRemove}>
              {t('slideActions.removeLink')}
            </button>
          )}
          <button type="button" data-testid="link-cancel" onClick={onClose}>
            {t('slideActions.cancel')}
          </button>
          <button
            type="button"
            data-testid="link-ok"
            disabled={!target || locked || info.scope === 'none'}
            onClick={() => {
              if (!target) return;
              const tooltip = d.tooltip.trim();
              onApply({
                target,
                ...(tooltip ? { tooltip } : {}),
                highlight: info.click?.highlight ?? false,
                sound: { kind: 'keep' },
              });
            }}
          >
            {t('slideActions.ok')}
          </button>
        </>
      }
    >
      {error && (
        <p role="alert" className={styles.warn} data-testid="link-error">
          {t(`slideActions.err.${error}`, { defaultValue: error })}
        </p>
      )}
      <p className={forms.muted} data-testid="link-scope">
        {info.scope === 'text'
          ? t('slideActions.scopeText', { text: info.text })
          : info.scope === 'shape'
            ? t('slideActions.scopeShape')
            : t('slideActions.scopeNone')}
      </p>
      {locked && <p className={styles.warn}>{t('slideActions.locked')}</p>}
      <div className={styles.tabs} role="tablist" aria-label={t('slideActions.linkTo')}>
        {LINK_KINDS.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={d.kind === k}
            className={d.kind === k ? styles.tabOn : styles.tab}
            data-testid={`link-kind-${k}`}
            onClick={() => setD({ ...d, kind: k })}
          >
            {t(`slideActions.kind.${k}`)}
          </button>
        ))}
      </div>
      <div className={styles.panel} role="tabpanel">
        {d.kind === 'url' && (
          <label className={styles.field}>
            {t('slideActions.address')}
            <input
              type="url"
              data-testid="link-url"
              placeholder="https://"
              value={d.url}
              autoFocus
              onChange={(e) => setD({ ...d, url: e.target.value })}
            />
            <span className={forms.muted}>{t('slideActions.urlHint')}</span>
          </label>
        )}
        {d.kind === 'place' && (
          <>
            <div className={forms.row}>
              {PLACES.map((p) =>
                place({ kind: p }, t(`slideActions.jump.${p}`), `link-place-${p}`),
              )}
            </div>
            <div className={styles.slides} data-testid="link-slides">
              {Array.from({ length: count }, (_, i) =>
                place(
                  { kind: 'slide', slide: i },
                  <>
                    {thumb?.(i, 112)}
                    <span>{slideName(i, info.slides)}</span>
                  </>,
                  `link-slide-${i + 1}`,
                  styles.slide,
                ),
              )}
            </div>
          </>
        )}
        {d.kind === 'show' &&
          (customShows.length === 0 ? (
            <p className={forms.muted}>{t('slideActions.noShows')}</p>
          ) : (
            <>
              <label className={styles.field}>
                {t('slideActions.kind.show')}
                <select
                  data-testid="link-show"
                  value={d.show ?? ''}
                  onChange={(e) => setD({ ...d, show: Number(e.target.value) })}
                >
                  {customShows.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className={forms.row}>
                <input
                  type="checkbox"
                  data-testid="link-show-return"
                  checked={d.ret}
                  onChange={(e) => setD({ ...d, ret: e.target.checked })}
                />
                {t('slideActions.showReturn')}
              </label>
            </>
          ))}
        {d.kind === 'mail' && (
          <>
            <label className={styles.field}>
              {t('slideActions.mailAddress')}
              <input
                type="email"
                data-testid="link-mail"
                value={d.address}
                onChange={(e) => setD({ ...d, address: e.target.value })}
              />
            </label>
            <label className={styles.field}>
              {t('slideActions.mailSubject')}
              <input
                type="text"
                data-testid="link-subject"
                value={d.subject}
                onChange={(e) => setD({ ...d, subject: e.target.value })}
              />
            </label>
          </>
        )}
        {d.kind === 'file' && (
          <label className={styles.field}>
            {t('slideActions.filePath')}
            <input
              type="text"
              data-testid="link-file"
              value={d.path}
              onChange={(e) => setD({ ...d, path: e.target.value })}
            />
            <span className={styles.warn}>{t('slideActions.fileHint')}</span>
          </label>
        )}
      </div>
      <label className={styles.field}>
        {t('slideActions.tooltip')}
        <input
          type="text"
          data-testid="link-tooltip"
          value={d.tooltip}
          onChange={(e) => setD({ ...d, tooltip: e.target.value })}
        />
      </label>
    </Dialog>
  );
}

// ------------------------------------------------------------------ Insert › Action

export interface ActionDialogProps {
  info: SlideActionInfo;
  count: number;
  customShows: SlideCustomShow[];
  /** Pick a WAV file. */
  pickSound?: () => Promise<{ bytes: Uint8Array; name: string } | null>;
  /** Play a WAV (the "listen" button). */
  preview?: (wav: Uint8Array) => void;
  error?: string;
  onClose: () => void;
  /** Apply the tabs that changed, click first. */
  onApply: (changes: { on: SlideActionTrigger; spec: SlideActionSpec }[]) => void;
}

const TRIGGERS: SlideActionTrigger[] = ['click', 'hover'];
const LINK_TO: LinkToKind[] = [...JUMP_KINDS, 'slide', 'customShow', 'url', 'file'];

/**
 * Insert › Action: what a click on, and the pointer over, the selection does —
 * nothing, a hyperlink (jumps, a slide, a custom show, a web address, a file),
 * a sound (built-in or a WAV file) and a highlight (§21.1.2.3.5–6).
 */
export function ActionDialog({
  info,
  count,
  customShows,
  pickSound,
  preview,
  error,
  onClose,
  onApply,
}: ActionDialogProps) {
  const { t } = useTranslation();
  const firstShow = customShows[0]?.id ?? null;
  const [tab, setTab] = useState<SlideActionTrigger>('click');
  const [drafts, setDrafts] = useState<Record<SlideActionTrigger, ActionDraft>>(() => ({
    click: draftOf(info.click, firstShow),
    hover: draftOf(info.hover, firstShow),
  }));
  const [dirty, setDirty] = useState<Record<SlideActionTrigger, boolean>>({
    click: false,
    hover: false,
  });
  const d = drafts[tab];
  const set = (patch: Partial<ActionDraft>) => {
    setDrafts((x) => ({ ...x, [tab]: { ...x[tab], ...patch } }));
    setDirty((x) => ({ ...x, [tab]: true }));
  };
  const current = tab === 'click' ? info.click : info.hover;
  const kept = d.mode === 'keep';
  // A kept action (a program, a macro…) is simply not sent.
  const valid = TRIGGERS.every(
    (k) => !dirty[k] || drafts[k].mode === 'keep' || specOf(drafts[k]) !== null,
  );
  const builtin = d.sound === 'keep' ? builtinOf(d.soundName) : null;
  const soundValue: SoundChoice = builtin ?? d.sound;
  const onSound = async (v: string) => {
    if (v === 'file') {
      const f = pickSound ? await pickSound() : null;
      if (f) set({ sound: 'file', file: { name: f.name, data: base64Of(f.bytes) } });
      return;
    }
    set({ sound: v as SoundChoice, file: null });
  };
  const isBuiltin = (BUILTIN_SOUNDS as readonly string[]).includes(soundValue);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideActions.actionTitle')}
      testId="action-dialog"
      width={520}
      footer={
        <>
          <button type="button" data-testid="action-cancel" onClick={onClose}>
            {t('slideActions.cancel')}
          </button>
          <button
            type="button"
            data-testid="action-ok"
            disabled={!valid || info.scope === 'none'}
            onClick={() => {
              const changes = TRIGGERS.flatMap((on) => {
                const spec = dirty[on] ? specOf(drafts[on]) : null;
                return spec ? [{ on, spec }] : [];
              });
              if (changes.length) onApply(changes);
              else onClose();
            }}
          >
            {t('slideActions.ok')}
          </button>
        </>
      }
    >
      {error && (
        <p role="alert" className={styles.warn} data-testid="action-error">
          {t(`slideActions.err.${error}`, { defaultValue: error })}
        </p>
      )}
      <p className={forms.muted}>
        {info.scope === 'text'
          ? t('slideActions.scopeText', { text: info.text })
          : info.scope === 'shape'
            ? t('slideActions.scopeShape')
            : t('slideActions.scopeNone')}
      </p>
      <div className={styles.tabs} role="tablist" aria-label={t('slideActions.actionTitle')}>
        {TRIGGERS.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={tab === k ? styles.tabOn : styles.tab}
            data-testid={`action-tab-${k}`}
            onClick={() => setTab(k)}
          >
            {t(`slideActions.on.${k}`)}
          </button>
        ))}
      </div>
      <div className={styles.panel} role="tabpanel" data-testid={`action-panel-${tab}`}>
        {kept && current && (
          <p className={styles.warn} data-testid="action-kept">
            {current.locked
              ? t('slideActions.locked')
              : t('slideActions.neverRun', {
                  what: t(`slideActions.blocked.${current.target.kind}`, {
                    defaultValue: t('slideActions.blocked.unknown'),
                  }),
                })}
          </p>
        )}
        <fieldset className={forms.form} disabled={!!current?.locked}>
          <label className={forms.row}>
            <input
              type="radio"
              name={`action-mode-${tab}`}
              data-testid="action-none"
              checked={d.mode === 'none'}
              onChange={() => set({ mode: 'none' })}
            />
            {t('slideActions.none')}
          </label>
          <label className={forms.row}>
            <input
              type="radio"
              name={`action-mode-${tab}`}
              data-testid="action-link"
              checked={d.mode === 'link'}
              onChange={() => set({ mode: 'link' })}
            />
            {t('slideActions.hyperlinkTo')}
            <select
              data-testid="action-to"
              value={d.to}
              disabled={d.mode !== 'link'}
              onChange={(e) => set({ to: e.target.value as LinkToKind })}
            >
              {LINK_TO.map((k) => (
                <option key={k} value={k} disabled={k === 'customShow' && firstShow === null}>
                  {t(`slideActions.to.${k}`)}
                </option>
              ))}
            </select>
          </label>
          {d.mode === 'link' && d.to === 'slide' && (
            <label className={styles.field}>
              {t('slideActions.to.slide')}
              <select
                data-testid="action-slide"
                value={d.slide}
                onChange={(e) => set({ slide: Number(e.target.value) })}
              >
                {Array.from({ length: count }, (_, i) => (
                  <option key={i} value={i}>
                    {slideName(i, info.slides)}
                  </option>
                ))}
              </select>
            </label>
          )}
          {d.mode === 'link' && d.to === 'customShow' && (
            <>
              <select
                data-testid="action-show"
                value={d.show ?? ''}
                onChange={(e) => set({ show: Number(e.target.value) })}
              >
                {customShows.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
              <label className={forms.row}>
                <input
                  type="checkbox"
                  data-testid="action-show-return"
                  checked={d.ret}
                  onChange={(e) => set({ ret: e.target.checked })}
                />
                {t('slideActions.showReturn')}
              </label>
            </>
          )}
          {d.mode === 'link' && d.to === 'url' && (
            <label className={styles.field}>
              {t('slideActions.address')}
              <input
                type="url"
                data-testid="action-url"
                placeholder="https://"
                value={d.url}
                onChange={(e) => set({ url: e.target.value })}
              />
              {d.url.trim() !== '' && targetOf(d) === null && (
                <span className={styles.warn}>{t('slideActions.urlHint')}</span>
              )}
            </label>
          )}
          {d.mode === 'link' && d.to === 'file' && (
            <label className={styles.field}>
              {t('slideActions.filePath')}
              <input
                type="text"
                data-testid="action-file"
                value={d.path}
                onChange={(e) => set({ path: e.target.value })}
              />
              <span className={styles.warn}>{t('slideActions.fileHint')}</span>
            </label>
          )}
          {tab === 'hover' && d.mode === 'link' && d.to === 'url' && (
            <p className={forms.muted}>{t('slideActions.hoverExternal')}</p>
          )}
        </fieldset>
        <fieldset className={forms.form} disabled={!!current?.locked}>
          <label className={forms.row}>
            {t('slideActions.sound')}
            <select
              data-testid="action-sound"
              value={soundValue}
              onChange={(e) => void onSound(e.target.value)}
            >
              <option value="none">{t('slideActions.soundNone')}</option>
              <option value="stop">{t('slideActions.soundStop')}</option>
              {BUILTIN_SOUNDS.map((s) => (
                <option key={s} value={s}>
                  {t(`slideActions.builtin.${s}`)}
                </option>
              ))}
              {d.sound === 'keep' && !builtin && <option value="keep">{d.soundName}</option>}
              <option value="file">
                {d.sound === 'file' && d.file ? d.file.name : t('slideActions.soundFile')}
              </option>
            </select>
            {preview && isBuiltin && (
              <button
                type="button"
                data-testid="action-sound-preview"
                onClick={() => preview(builtinWav(soundValue as BuiltinSound))}
              >
                {t('slideActions.listen')}
              </button>
            )}
          </label>
          <label className={forms.row}>
            <input
              type="checkbox"
              data-testid="action-highlight"
              checked={d.highlight}
              onChange={(e) => set({ highlight: e.target.checked })}
            />
            {t(tab === 'click' ? 'slideActions.highlightClick' : 'slideActions.highlightHover')}
          </label>
        </fieldset>
      </div>
    </Dialog>
  );
}

// ------------------------------------------------------------------ action buttons

export interface ActionButtonsDialogProps {
  onClose: () => void;
  /** Insert the button with preset geometry `preset`. */
  onPick: (preset: string) => void;
}

/** Simple line drawings of the action buttons (own artwork). */
const BUTTON_ART: Record<string, string> = {
  actionButtonBackPrevious: 'M15 5 7 12l8 7Z',
  actionButtonForwardNext: 'M9 5l8 7-8 7Z',
  actionButtonBeginning: 'M7 5v14M18 5l-9 7 9 7Z',
  actionButtonEnd: 'M17 5v14M6 5l9 7-9 7Z',
  actionButtonHome: 'M4 12 12 5l8 7M7 11v8h10v-8',
  actionButtonReturn: 'M18 6v6a3 3 0 0 1-3 3H7M10 11l-4 4 4 4',
  actionButtonInformation: 'M12 10v8M12 6.5v.5',
  actionButtonHelp: 'M9 9a3 3 0 1 1 4.5 2.6c-1 .6-1.5 1.2-1.5 2.4M12 17.5v.5',
  actionButtonDocument: 'M7 4h7l4 4v12H7ZM14 4v4h4',
  actionButtonSound: 'M5 10v4h3l4 4V6l-4 4ZM15.5 9a4 4 0 0 1 0 6',
  actionButtonMovie: 'M4 7h12v10H4ZM16 10l4-2v8l-4-2',
  actionButtonBlank: '',
};

/** Insert › Action buttons: preset shapes that come with their usual action. */
export function ActionButtonsDialog({ onClose, onPick }: ActionButtonsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideActions.buttonsTitle')}
      testId="action-buttons-dialog"
      width={420}
      footer={
        <button type="button" onClick={onClose}>
          {t('slideActions.cancel')}
        </button>
      }
    >
      <p className={forms.muted}>{t('slideActions.buttonsHint')}</p>
      <div className={styles.buttons}>
        {ACTION_BUTTONS.map((b) => {
          const label = t(`slideActions.button.${b.preset}`);
          return (
            <button
              key={b.preset}
              type="button"
              className={styles.button}
              title={label}
              aria-label={label}
              data-testid={`action-button-${b.preset}`}
              onClick={() => onPick(b.preset)}
            >
              <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
                <rect x="1.5" y="1.5" width="21" height="21" rx="3" className={styles.buttonFace} />
                <path d={BUTTON_ART[b.preset] ?? ''} className={styles.buttonMark} />
              </svg>
              <span>{label}</span>
            </button>
          );
        })}
      </div>
    </Dialog>
  );
}

// ------------------------------------------------------------------ in the show

export interface ShowLinkPromptProps {
  /** An external target waiting for the user's answer. */
  ask: Extract<ActionPolicy, { kind: 'external' }> | null;
  /** An action that was not run, or a target the host could not open. */
  notice: { reason: string; detail: string } | null;
  onOpen: () => void;
  onCancel: () => void;
  onDismiss: () => void;
}

/**
 * Over the slide show: "open this address?" before anything leaves the app,
 * and the notice that an action was blocked. Drawn inside the show's own
 * element, so it is visible in full screen.
 */
export function ShowLinkPrompt({ ask, notice, onOpen, onCancel, onDismiss }: ShowLinkPromptProps) {
  const { t } = useTranslation();
  if (!ask && !notice) return null;
  // Clicks and keys on the prompt never reach the show (they would advance it);
  // Esc answers "no".
  const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();
  const keys = (e: { key: string; stopPropagation: () => void; preventDefault: () => void }) => {
    e.stopPropagation();
    if (e.key !== 'Escape') return;
    e.preventDefault();
    if (ask) onCancel();
    else onDismiss();
  };
  if (ask) {
    return (
      <div
        className={styles.prompt}
        role="alertdialog"
        aria-label={t('slideActions.openTitle')}
        data-testid="show-link-ask"
        onClick={stop}
        onContextMenu={stop}
        onKeyDown={keys}
      >
        <strong>{t(ask.mail ? 'slideActions.openMail' : 'slideActions.openTitle')}</strong>
        <code className={styles.target} data-testid="show-link-url">
          {ask.url}
        </code>
        <span className={forms.muted}>{t('slideActions.openHint')}</span>
        <div className={forms.row}>
          <button type="button" data-testid="show-link-open" autoFocus onClick={onOpen}>
            {t('slideActions.open')}
          </button>
          <button type="button" data-testid="show-link-cancel" onClick={onCancel}>
            {t('slideActions.cancel')}
          </button>
        </div>
      </div>
    );
  }
  const n = notice!;
  return (
    <div
      className={styles.prompt}
      role="alert"
      data-testid="show-link-blocked"
      data-reason={n.reason}
      onClick={stop}
      onContextMenu={stop}
      onKeyDown={keys}
    >
      <strong>
        {t(`slideActions.blockedNotice.${n.reason}`, {
          defaultValue: t('slideActions.blockedNotice.unknown'),
        })}
      </strong>
      {n.detail && <code className={styles.target}>{n.detail}</code>}
      <span className={forms.muted}>{t('slideActions.blockedHint')}</span>
      <div className={forms.row}>
        <button type="button" data-testid="show-link-dismiss" autoFocus onClick={onDismiss}>
          {t('slideActions.ok')}
        </button>
      </div>
    </div>
  );
}
