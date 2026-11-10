/**
 * BlueAI task pane shared by BlueDoc, BlueSheet and BlueSlide (CORE-AI-001): `useBlueAi`
 * gives the content of an `ai` task pane (shown through the apps' `TaskPaneHost`). Each app
 * passes its features (rewrite / summarise / draft, slides from an outline, formula suggestion,
 * data analysis); the pane shows what would be sent and where (the host the administrator
 * configured), runs the feature and shows the outcome as a **preview** the user applies
 * (Insert / Replace / Create slides / …), copies or discards.
 *
 * Safety rules the pane enforces for every feature:
 * - BlueAI off (no provider, or not enabled by the administrator): features that need it are
 *   disabled with an explanation and nothing is sent; rule-based features keep working.
 * - A request larger than the provider's limit is refused before it is sent, with the limit.
 * - A provider answer is untrusted text: it is shown in a `<pre>` (never as markup), and the
 *   only thing that can change the document is a button the user presses on the preview.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  AI_TONES,
  AiError,
  aiComplete,
  aiError,
  type AiInfo,
  type AiProvider,
  type AiRequest,
  type AiTone,
  type ClipboardAccess,
} from '@blueoffice/platform';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './AiPane.module.css';

/** A failure a feature explains in its own (already translated) words. */
export class AiNotice extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AiNotice';
  }
}

/** The text a feature works on, as read from the app (selection, document, cells). */
export interface AiMaterial {
  text: string;
  /** What it is, for the user ("Selected text", "Cells A1:C5"). */
  label: string;
  /** Anything the feature needs back when it runs (where the text came from). */
  from?: string;
}

/** What a feature gets when the user runs it. */
export interface AiRunContext {
  /** The request typed by the user ('' when the feature has no prompt). */
  prompt: string;
  tone: AiTone;
  /** Answer language (`vi` / `en`, from the interface language). */
  language: string;
  material: AiMaterial | null;
  signal: AbortSignal;
  /** Asks the provider (size-checked, plain text back). Throws `AiError`. */
  ask(request: AiRequest): Promise<string>;
}

/** A way to use a previewed result. */
export interface AiApply {
  id: string;
  label: string;
  /** Applies the result; a returned text is shown as the outcome. */
  apply(): Promise<string | void> | string | void;
}

/** The outcome of a feature, shown before anything changes. */
export interface AiPreview {
  /** Shown as plain text. */
  text: string;
  /** A remark under the preview (how a formula was checked …). */
  note?: string;
  actions: AiApply[];
}

export interface AiFeature {
  id: string;
  label: string;
  /** Needs the provider (`false`: rule-based, works while BlueAI is off). */
  ai: boolean;
  hint?: string;
  /** A request the user types. */
  prompt?: { label: string; placeholder?: string; required: boolean; rows?: number };
  /** Offers the rewrite tones. */
  tones?: boolean;
  /** Reads the material from the app; with `required`, the feature cannot run without text. */
  material?: { read(): Promise<AiMaterial | null>; required: boolean; empty: string };
  run(ctx: AiRunContext): Promise<AiPreview>;
}

export interface AiOpen {
  features: AiFeature[];
  /** Feature shown first. */
  feature?: string;
}

export interface BlueAi {
  active: boolean;
  /** BlueAI is known to be off (no provider or not enabled). */
  off: boolean;
  open(o: AiOpen): void;
  close(): void;
  /** Content of the `ai` task pane (`null` while closed). */
  content: ReactNode;
}

/** Characters of the material shown in the pane. */
const SNIPPET = 400;

export function useBlueAi(
  provider: AiProvider | undefined,
  clipboard?: Pick<ClipboardAccess, 'writeText'>,
): BlueAi {
  const { t, i18n } = useTranslation();
  const [req, setReq] = useState<AiOpen | null>(null);
  const [featureId, setFeatureId] = useState<string | null>(null);
  /** `undefined`: not looked up yet; `null`: off. */
  const [info, setInfo] = useState<AiInfo | null | undefined>(provider ? undefined : null);
  const [prompt, setPrompt] = useState('');
  const [tone, setTone] = useState<AiTone>('formal');
  const [material, setMaterial] = useState<AiMaterial | null>(null);
  const [preview, setPreview] = useState<AiPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const abort = useRef<AbortController | null>(null);
  /** Bumped by each open / close / feature change: late answers of an older one are ignored. */
  const gen = useRef(0);
  /** The feature shown now (for answers that arrive later). */
  const shown = useRef<AiFeature | null>(null);
  const language = (i18n.language || 'vi-VN').toLowerCase().startsWith('vi') ? 'vi' : 'en';

  const errorText = useCallback(
    (e: unknown) => {
      if (e instanceof AiNotice) return e.message;
      const err = aiError(e);
      return t(`ai.errors.${err.code}`, { max: err.limit?.max ?? 0, size: err.limit?.size ?? 0 });
    },
    [t],
  );

  const cancel = useCallback(() => {
    abort.current?.abort();
    abort.current = null;
  }, []);

  useEffect(() => () => abort.current?.abort(), []);

  const feature = req?.features.find((f) => f.id === featureId) ?? req?.features[0] ?? null;

  const readMaterial = useCallback(async (f: AiFeature | null) => {
    const g = gen.current;
    const m = f?.material ? await f.material.read().catch(() => null) : null;
    if (gen.current === g) setMaterial(m);
  }, []);

  const select = useCallback(
    (f: AiFeature | null) => {
      gen.current++;
      shown.current = f;
      cancel();
      setBusy(false);
      setFeatureId(f?.id ?? null);
      setPreview(null);
      setNotice(null);
      setMaterial(null);
      void readMaterial(f);
    },
    [cancel, readMaterial],
  );

  /**
   * BlueAI turned out to be off: when the feature shown needs it, move to the first feature of
   * the app that works without it (rule-based), if there is one.
   */
  const turnOff = useCallback(
    (features: AiFeature[]) => {
      setInfo(null);
      const local = features.find((f) => !f.ai);
      if (shown.current?.ai && local) select(local);
    },
    [select],
  );

  const open = useCallback(
    (o: AiOpen) => {
      setReq(o);
      setPrompt('');
      const first = o.features.find((f) => f.id === o.feature) ?? o.features[0] ?? null;
      select(first);
      void (async () => {
        const i = provider ? await provider.info().catch(() => null) : null;
        if (i) setInfo(i);
        else turnOff(o.features);
      })();
    },
    [provider, select, turnOff],
  );

  const close = useCallback(() => {
    gen.current++;
    cancel();
    setReq(null);
    setBusy(false);
    setPreview(null);
    setNotice(null);
    setMaterial(null);
  }, [cancel]);

  const off = info === null;
  const max = info?.maxInputChars ?? 0;
  const size = (material?.text.length ?? 0) + prompt.length;
  const needsAi = !!feature?.ai;
  const tooLarge = needsAi && !!info && size > max;
  const missingPrompt = !!feature?.prompt?.required && !prompt.trim();
  const missingMaterial = !!feature?.material?.required && !material?.text.trim();

  const run = useCallback(async () => {
    if (!feature) return;
    cancel();
    const ac = new AbortController();
    abort.current = ac;
    const g = gen.current;
    setNotice(null);
    setPreview(null);
    setBusy(true);
    try {
      // Only what the pane shows may be sent: when the selection changed since it was read,
      // show the new material and let the user look at it before running.
      if (feature.material) {
        const fresh = await feature.material.read().catch(() => null);
        if (gen.current !== g) return;
        if ((fresh?.text ?? '') !== (material?.text ?? '')) {
          setMaterial(fresh);
          setNotice(t('ai.materialChanged'));
          return;
        }
      }
      const out = await feature.run({
        prompt: prompt.trim(),
        tone,
        language,
        material,
        signal: ac.signal,
        ask: (request) => {
          if (!feature.ai) throw new AiError('unavailable');
          return aiComplete(provider, request, { signal: ac.signal });
        },
      });
      if (gen.current === g && !ac.signal.aborted) setPreview(out);
    } catch (e) {
      if (gen.current === g) {
        setNotice(errorText(e));
        // The server says BlueAI is off: show the pane as off from now on.
        if (!(e instanceof AiNotice) && aiError(e).code === 'unavailable' && feature.ai)
          turnOff(req?.features ?? []);
      }
    } finally {
      if (abort.current === ac) abort.current = null;
      if (gen.current === g) setBusy(false);
    }
  }, [feature, cancel, prompt, tone, language, material, provider, errorText, t, turnOff, req]);

  const apply = useCallback(
    async (a: AiApply) => {
      setNotice(null);
      try {
        const said = await a.apply();
        setPreview(null);
        setNotice(said || t('ai.applied'));
        void readMaterial(feature);
      } catch (e) {
        setNotice(
          e instanceof AiError || e instanceof AiNotice ? errorText(e) : t('ai.applyFailed'),
        );
      }
    },
    [t, errorText, readMaterial, feature],
  );

  const offReason = t('ai.offTooltip');
  const runBlocked = (needsAi && info !== undefined && off) || (needsAi && info === undefined);
  const content = req ? (
    <div
      className={styles.pane}
      data-testid="ai-pane"
      data-busy={busy ? 'true' : 'false'}
      data-state={info === undefined ? 'loading' : off ? 'off' : 'on'}
      onPointerEnter={() => {
        // Back from the document: what would be sent follows the selection.
        if (!busy && !preview) void readMaterial(feature);
      }}
    >
      {info === undefined ? (
        <p className={styles.hint} role="status">
          {t('ai.checking')}
        </p>
      ) : off ? (
        <p className={styles.notice} role="status" data-testid="ai-off">
          {t('ai.off')}
        </p>
      ) : (
        <p className={styles.consent} data-testid="ai-consent">
          {t('ai.consent', { host: info.host || t('ai.unknownHost'), max })}
        </p>
      )}
      <div className={styles.features} role="radiogroup" aria-label={t('ai.featureList')}>
        {req.features.map((f) => {
          const disabled = f.ai && off;
          return (
            <button
              key={f.id}
              type="button"
              role="radio"
              aria-checked={feature?.id === f.id}
              className={`${styles.feature} ${feature?.id === f.id ? styles.selected : ''}`}
              data-testid={`ai-feature-${f.id}`}
              disabled={disabled}
              title={disabled ? offReason : f.hint}
              onClick={() => select(f)}
            >
              {f.label}
            </button>
          );
        })}
      </div>
      {feature && !(feature.ai && off) && (
        <>
          {feature.hint && <p className={styles.hint}>{feature.hint}</p>}
          {feature.tones && (
            <label className={styles.label}>
              {t('ai.tone.label')}
              <select
                className={styles.select}
                data-testid="ai-tone"
                value={tone}
                onChange={(e) => setTone(e.target.value as AiTone)}
              >
                {AI_TONES.map((k) => (
                  <option key={k} value={k}>
                    {t(`ai.tone.${k}`)}
                  </option>
                ))}
              </select>
            </label>
          )}
          {feature.prompt && (
            <label className={styles.label}>
              {feature.prompt.label}
              <textarea
                className={styles.input}
                data-testid="ai-prompt"
                rows={feature.prompt.rows ?? 3}
                value={prompt}
                placeholder={feature.prompt.placeholder}
                onChange={(e) => setPrompt(e.target.value)}
              />
            </label>
          )}
          {feature.material && (
            <div className={styles.section}>
              <div className={styles.caption}>
                <span>
                  {feature.ai ? t('ai.material.sent') : t('ai.material.used')}
                  {material?.text ? ` — ${material.label}` : ''}
                </span>
                <button
                  type="button"
                  className={styles.link}
                  data-testid="ai-material-refresh"
                  onClick={() => void readMaterial(feature)}
                >
                  {t('ai.material.refresh')}
                </button>
              </div>
              {material?.text.trim() ? (
                <div className={styles.material} data-testid="ai-material">
                  {material.text.length > SNIPPET
                    ? `${material.text.slice(0, SNIPPET)}…`
                    : material.text}
                </div>
              ) : (
                <p className={styles.hint} data-testid="ai-material-empty">
                  {feature.material.empty}
                </p>
              )}
            </div>
          )}
          {needsAi && !!info && (
            <p
              className={tooLarge ? styles.notice : styles.hint}
              role={tooLarge ? 'alert' : undefined}
              data-testid={tooLarge ? 'ai-too-large' : 'ai-size'}
            >
              {tooLarge ? t('ai.errors.tooLarge', { max, size }) : t('ai.size', { size, max })}
            </p>
          )}
          <div className={styles.row}>
            <button
              type="button"
              className={styles.primary}
              data-testid="ai-run"
              disabled={busy || runBlocked || tooLarge || missingPrompt || missingMaterial}
              title={needsAi && off ? offReason : undefined}
              onClick={() => void run()}
            >
              {feature.ai ? t('ai.run') : t('ai.preview')}
            </button>
            {busy && (
              <>
                <span role="status" data-testid="ai-working">
                  {t('ai.working')}
                </span>
                <button
                  type="button"
                  className={styles.button}
                  data-testid="ai-cancel"
                  onClick={() => {
                    cancel();
                    gen.current++;
                    setBusy(false);
                    setNotice(t('ai.errors.cancelled'));
                  }}
                >
                  {t('ai.cancel')}
                </button>
              </>
            )}
          </div>
        </>
      )}
      {preview && (
        <div className={styles.section}>
          <div className={styles.caption}>{t('ai.result')}</div>
          {/* Untrusted text: rendered as text only, never as markup. */}
          <pre className={styles.preview} data-testid="ai-preview" tabIndex={0}>
            {preview.text}
          </pre>
          {preview.note && (
            <p className={styles.hint} data-testid="ai-preview-note">
              {preview.note}
            </p>
          )}
          <div className={styles.row}>
            {preview.actions.map((a) => (
              <button
                key={a.id}
                type="button"
                className={styles.button}
                data-testid={`ai-apply-${a.id}`}
                onClick={() => void apply(a)}
              >
                {a.label}
              </button>
            ))}
            <button
              type="button"
              className={styles.button}
              data-testid="ai-copy"
              disabled={!clipboard}
              onClick={() => void clipboard?.writeText(preview.text)}
            >
              {t('ai.copy')}
            </button>
            <button
              type="button"
              className={styles.button}
              data-testid="ai-discard"
              onClick={() => {
                setPreview(null);
                setNotice(null);
              }}
            >
              {t('ai.discard')}
            </button>
          </div>
          <p className={styles.hint}>{t('ai.review')}</p>
        </div>
      )}
      {notice && (
        <p className={styles.notice} role="status" data-testid="ai-notice">
          {notice}
        </p>
      )}
    </div>
  ) : null;

  return { active: !!req, off, open, close, content };
}
