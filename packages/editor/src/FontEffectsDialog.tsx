/**
 * Advanced character formatting dialogs of BlueDoc (DOC-FMT-001):
 * - text effects and character spacing (ECMA-376 Part 1, §17.3.2 run properties and
 *   the [MS-DOCX] w14 text effects / OpenType typography), sent as one
 *   `formatText` / `advanced` patch holding only what the user changed;
 * - phonetic guide (§17.3.3.25 ruby) → `insertRuby`;
 * - two lines in one (§17.3.2.10 eastAsianLayout combine).
 * Each dialog reads the current state with `{ cmd: 'state' }`.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CharAdvanced,
  CharAdvancedPatch,
  EditCommand,
  EditResult,
  RubySpec,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState, type ReactNode } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** State when nothing is set (plain text). */
export const PLAIN: CharAdvanced = {
  emboss: false,
  imprint: false,
  outline: false,
  shadow: false,
  hidden: false,
  spacing: 0,
  position: 0,
  scale: 100,
  kern: null,
  glow: null,
  softShadow: null,
  reflection: null,
  textOutline: null,
  gradient: null,
  ligatures: '',
  numForm: '',
  numSpacing: '',
  stylisticSet: 0,
  contextualAlternates: false,
  border: null,
  shading: null,
  combine: false,
  combineBrackets: '',
};

/** The fields of `next` that differ from `initial` (the patch to send). */
export function advancedPatch(initial: CharAdvanced, next: CharAdvanced): CharAdvancedPatch {
  const patch: Record<string, unknown> = {};
  for (const k of Object.keys(next) as (keyof CharAdvanced)[]) {
    if (JSON.stringify(initial[k]) !== JSON.stringify(next[k])) patch[k] = next[k];
  }
  return patch as CharAdvancedPatch;
}

/** Read the advanced state at the selection (falls back to plain); `onState` seeds
 * the form once. */
function useAdvanced(run: Run, onState?: (a: CharAdvanced) => void): CharAdvanced | null {
  const [state, setState] = useState<CharAdvanced | null>(null);
  useEffect(() => {
    let live = true;
    void run({ cmd: 'state' }).then((r) => {
      if (!live) return;
      const a = { ...PLAIN, ...(r?.format?.advanced ?? {}) };
      setState(a);
      onState?.(a);
    });
    return () => {
      live = false;
    };
    // `onState` only seeds the form once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run]);
  return state;
}

function Footer({
  onClose,
  onApply,
  testId,
}: {
  onClose: () => void;
  onApply: () => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.dialogButton} onClick={onClose}>
        {t('charFx.cancel')}
      </button>
      <button
        type="button"
        className={styles.dialogPrimary}
        data-testid={`${testId}-ok`}
        onClick={onApply}
      >
        {t('charFx.ok')}
      </button>
    </>
  );
}

function Num({
  label,
  value,
  onChange,
  testId,
  min,
  max,
  step = 1,
  disabled,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  testId: string;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
}) {
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        data-testid={testId}
        value={Number.isFinite(value) ? value : 0}
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)));
        }}
      />
    </label>
  );
}

function ColorField({
  label,
  value,
  onChange,
  testId,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (hex: string) => void;
  testId: string;
  disabled?: boolean;
}) {
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <input
        type="color"
        disabled={disabled}
        data-testid={testId}
        value={`#${value}`}
        onChange={(e) => onChange(e.target.value.replace('#', '').toUpperCase())}
      />
    </label>
  );
}

function Check({
  label,
  checked,
  onChange,
  testId,
}: {
  label: string;
  checked: boolean;
  onChange: (on: boolean) => void;
  testId: string;
}) {
  return (
    <label>
      <input
        type="checkbox"
        data-testid={testId}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />{' '}
      {label}
    </label>
  );
}

/** An optional effect: a check box that turns `value` on (with `fallback`) or off. */
function Optional<T>({
  label,
  value,
  fallback,
  onChange,
  testId,
  children,
}: {
  label: string;
  value: T | null;
  fallback: T;
  onChange: (v: T | null) => void;
  testId: string;
  children: (v: T) => ReactNode;
}) {
  return (
    <fieldset className={styles.fieldset}>
      <legend>
        <Check
          label={label}
          checked={value !== null}
          testId={testId}
          onChange={(on) => onChange(on ? fallback : null)}
        />
      </legend>
      {value !== null && children(value)}
    </fieldset>
  );
}

const LIGATURES = ['', 'none', 'standard', 'standardContextual', 'historicalDiscretional', 'all'];
const NUM_FORMS = ['', 'lining', 'oldStyle'];
const NUM_SPACINGS = ['', 'proportional', 'tabular'];
const BORDER_STYLES = ['single', 'double', 'dotted', 'dashed', 'thick'];
export const BRACKETS = ['', 'round', 'square', 'angle', 'curly'];

/** Text effects, borders and character spacing of the selection. */
export function FontEffectsDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [v, setV] = useState<CharAdvanced | null>(null);
  const initial = useAdvanced(run, setV);
  const [tab, setTab] = useState<'effects' | 'spacing'>('effects');
  if (!initial || !v) return null;
  const set = (p: Partial<CharAdvanced>) => setV({ ...v, ...p });
  const apply = () => {
    onClose();
    const patch = advancedPatch(initial, v);
    if (Object.keys(patch).length > 0) {
      void run({ cmd: 'formatText', format: { kind: 'advanced', patch } });
    }
  };
  const effects = (
    <div className={styles.formGrid}>
      <fieldset className={styles.fieldset}>
        <legend>{t('charFx.classic')}</legend>
        <Check
          label={t('charFx.outline')}
          checked={v.outline}
          testId="fx-outline"
          onChange={(on) => set({ outline: on })}
        />
        <Check
          label={t('charFx.shadow')}
          checked={v.shadow}
          testId="fx-shadow"
          onChange={(on) => set({ shadow: on })}
        />
        <Check
          label={t('charFx.emboss')}
          checked={v.emboss}
          testId="fx-emboss"
          // Raised and engraved exclude each other and the hollow outline.
          onChange={(on) =>
            set(on ? { emboss: true, imprint: false, outline: false } : { emboss: false })
          }
        />
        <Check
          label={t('charFx.imprint')}
          checked={v.imprint}
          testId="fx-imprint"
          onChange={(on) =>
            set(on ? { imprint: true, emboss: false, outline: false } : { imprint: false })
          }
        />
        <Check
          label={t('charFx.hidden')}
          checked={v.hidden}
          testId="fx-hidden"
          onChange={(on) => set({ hidden: on })}
        />
      </fieldset>
      <Optional
        label={t('charFx.glow')}
        value={v.glow}
        fallback={{ color: 'FFC000', radius: 5, transparency: 40 }}
        testId="fx-glow"
        onChange={(glow) => set({ glow })}
      >
        {(g) => (
          <>
            <ColorField
              label={t('charFx.color')}
              value={g.color}
              testId="fx-glow-color"
              onChange={(color) => set({ glow: { ...g, color } })}
            />
            <Num
              label={t('charFx.radiusPt')}
              value={g.radius}
              min={0}
              max={150}
              step={0.5}
              testId="fx-glow-radius"
              onChange={(radius) => set({ glow: { ...g, radius } })}
            />
            <Num
              label={t('charFx.transparency')}
              value={g.transparency}
              min={0}
              max={100}
              testId="fx-glow-transparency"
              onChange={(transparency) => set({ glow: { ...g, transparency } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.softShadow')}
        value={v.softShadow}
        fallback={{ color: '000000', blur: 3, distance: 2, angle: 45, transparency: 60 }}
        testId="fx-soft-shadow"
        onChange={(softShadow) => set({ softShadow })}
      >
        {(s) => (
          <>
            <ColorField
              label={t('charFx.color')}
              value={s.color}
              testId="fx-shadow-color"
              onChange={(color) => set({ softShadow: { ...s, color } })}
            />
            <Num
              label={t('charFx.blurPt')}
              value={s.blur}
              min={0}
              max={100}
              step={0.5}
              testId="fx-shadow-blur"
              onChange={(blur) => set({ softShadow: { ...s, blur } })}
            />
            <Num
              label={t('charFx.distancePt')}
              value={s.distance}
              min={0}
              max={200}
              step={0.5}
              testId="fx-shadow-distance"
              onChange={(distance) => set({ softShadow: { ...s, distance } })}
            />
            <Num
              label={t('charFx.angle')}
              value={s.angle}
              min={0}
              max={359}
              testId="fx-shadow-angle"
              onChange={(angle) => set({ softShadow: { ...s, angle } })}
            />
            <Num
              label={t('charFx.transparency')}
              value={s.transparency}
              min={0}
              max={100}
              testId="fx-shadow-transparency"
              onChange={(transparency) => set({ softShadow: { ...s, transparency } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.reflection')}
        value={v.reflection}
        fallback={{ size: 50, transparency: 50, distance: 0 }}
        testId="fx-reflection"
        onChange={(reflection) => set({ reflection })}
      >
        {(r) => (
          <>
            <Num
              label={t('charFx.sizePercent')}
              value={r.size}
              min={1}
              max={100}
              testId="fx-reflection-size"
              onChange={(size) => set({ reflection: { ...r, size } })}
            />
            <Num
              label={t('charFx.transparency')}
              value={r.transparency}
              min={0}
              max={100}
              testId="fx-reflection-transparency"
              onChange={(transparency) => set({ reflection: { ...r, transparency } })}
            />
            <Num
              label={t('charFx.distancePt')}
              value={r.distance}
              min={0}
              max={100}
              step={0.5}
              testId="fx-reflection-distance"
              onChange={(distance) => set({ reflection: { ...r, distance } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.textOutline')}
        value={v.textOutline}
        fallback={{ color: '1F3A68', width: 0.75 }}
        testId="fx-text-outline"
        onChange={(textOutline) => set({ textOutline })}
      >
        {(o) => (
          <>
            <ColorField
              label={t('charFx.color')}
              value={o.color}
              testId="fx-text-outline-color"
              onChange={(color) => set({ textOutline: { ...o, color } })}
            />
            <Num
              label={t('charFx.widthPt')}
              value={o.width}
              min={0}
              max={20}
              step={0.25}
              testId="fx-text-outline-width"
              onChange={(width) => set({ textOutline: { ...o, width } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.gradient')}
        value={v.gradient}
        fallback={{ from: '1F3A68', to: '2E7D32', angle: 0 }}
        testId="fx-gradient"
        onChange={(gradient) => set({ gradient })}
      >
        {(g) => (
          <>
            <ColorField
              label={t('charFx.from')}
              value={g.from}
              testId="fx-gradient-from"
              onChange={(from) => set({ gradient: { ...g, from } })}
            />
            <ColorField
              label={t('charFx.to')}
              value={g.to}
              testId="fx-gradient-to"
              onChange={(to) => set({ gradient: { ...g, to } })}
            />
            <Num
              label={t('charFx.angle')}
              value={g.angle}
              min={0}
              max={359}
              testId="fx-gradient-angle"
              onChange={(angle) => set({ gradient: { ...g, angle } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.border')}
        value={v.border}
        fallback={{ style: 'single', size: 4, color: null }}
        testId="fx-border"
        onChange={(border) => set({ border })}
      >
        {(b) => (
          <>
            <label className={styles.field}>
              <span>{t('charFx.borderStyle')}</span>
              <select
                data-testid="fx-border-style"
                value={b.style}
                onChange={(e) => set({ border: { ...b, style: e.target.value } })}
              >
                {BORDER_STYLES.map((s) => (
                  <option key={s} value={s}>
                    {t(`charFx.borderStyles.${s}`)}
                  </option>
                ))}
              </select>
            </label>
            <Num
              label={t('charFx.widthPt')}
              value={b.size / 8}
              min={0.25}
              max={6}
              step={0.25}
              testId="fx-border-width"
              onChange={(w) => set({ border: { ...b, size: Math.round(w * 8) } })}
            />
            <ColorField
              label={t('charFx.color')}
              value={b.color ?? '000000'}
              testId="fx-border-color"
              onChange={(color) => set({ border: { ...b, color } })}
            />
          </>
        )}
      </Optional>
      <Optional
        label={t('charFx.shading')}
        value={v.shading}
        fallback="D9D9D9"
        testId="fx-shading"
        onChange={(shading) => set({ shading })}
      >
        {(s) => (
          <ColorField
            label={t('charFx.color')}
            value={s}
            testId="fx-shading-color"
            onChange={(shading) => set({ shading })}
          />
        )}
      </Optional>
    </div>
  );
  const spacing = (
    <div className={styles.formGrid}>
      <Num
        label={t('charFx.spacingPt')}
        value={v.spacing / 20}
        min={-1584}
        max={1584}
        step={0.1}
        testId="fx-spacing"
        onChange={(pt) => set({ spacing: Math.round(pt * 20) })}
      />
      <Num
        label={t('charFx.positionPt')}
        value={v.position / 2}
        min={-1584}
        max={1584}
        step={0.5}
        testId="fx-position"
        onChange={(pt) => set({ position: Math.round(pt * 2) })}
      />
      <Num
        label={t('charFx.scalePercent')}
        value={v.scale}
        min={1}
        max={600}
        testId="fx-scale"
        onChange={(scale) => set({ scale: Math.round(scale) })}
      />
      <Check
        label={t('charFx.kerning')}
        checked={v.kern !== null}
        testId="fx-kern"
        onChange={(on) => set({ kern: on ? 2 : null })}
      />
      {v.kern !== null && (
        <Num
          label={t('charFx.kernFromPt')}
          value={v.kern / 2}
          min={1}
          max={1638}
          step={0.5}
          testId="fx-kern-from"
          onChange={(pt) => set({ kern: Math.max(1, Math.round(pt * 2)) })}
        />
      )}
      <fieldset className={styles.fieldset}>
        <legend>{t('charFx.opentype')}</legend>
        <label className={styles.field}>
          <span>{t('charFx.ligatures')}</span>
          <select
            data-testid="fx-ligatures"
            value={v.ligatures}
            onChange={(e) => set({ ligatures: e.target.value })}
          >
            {LIGATURES.map((l) => (
              <option key={l} value={l}>
                {t(`charFx.ligatureKinds.${l || 'unset'}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('charFx.numForm')}</span>
          <select
            data-testid="fx-num-form"
            value={v.numForm}
            onChange={(e) => set({ numForm: e.target.value })}
          >
            {NUM_FORMS.map((n) => (
              <option key={n} value={n}>
                {t(`charFx.numForms.${n || 'unset'}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('charFx.numSpacing')}</span>
          <select
            data-testid="fx-num-spacing"
            value={v.numSpacing}
            onChange={(e) => set({ numSpacing: e.target.value })}
          >
            {NUM_SPACINGS.map((n) => (
              <option key={n} value={n}>
                {t(`charFx.numSpacings.${n || 'unset'}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('charFx.stylisticSet')}</span>
          <select
            data-testid="fx-stylistic-set"
            value={v.stylisticSet}
            onChange={(e) => set({ stylisticSet: Number(e.target.value) })}
          >
            {Array.from({ length: 21 }, (_, i) => (
              <option key={i} value={i}>
                {i === 0 ? t('charFx.none') : String(i)}
              </option>
            ))}
          </select>
        </label>
        <Check
          label={t('charFx.contextualAlternates')}
          checked={v.contextualAlternates}
          testId="fx-calt"
          onChange={(on) => set({ contextualAlternates: on })}
        />
      </fieldset>
    </div>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('charFx.title')}
      width={520}
      testId="font-effects-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="font-effects" />}
    >
      <div role="tablist" className={styles.formGrid}>
        <div>
          {(['effects', 'spacing'] as const).map((k) => (
            <button
              key={k}
              type="button"
              role="tab"
              aria-selected={tab === k}
              className={tab === k ? styles.dialogPrimary : styles.dialogButton}
              data-testid={`font-effects-tab-${k}`}
              onClick={() => setTab(k)}
            >
              {t(`charFx.tabs.${k}`)}
            </button>
          ))}
        </div>
      </div>
      {tab === 'effects' ? effects : spacing}
    </Dialog>
  );
}

const RUBY_ALIGNS = ['center', 'distributeLetter', 'distributeSpace', 'left', 'right'];

/** Defaults of a new guide over text of `baseSize` half-points: half the size, its
 * baseline one base size above (BlueOffice choice). */
export function rubyDefaults(baseSize: number, lang: string): RubySpec {
  const b = Math.max(2, Math.round(baseSize));
  return {
    base: '',
    guide: '',
    align: 'center',
    size: Math.max(2, Math.round(b / 2)),
    raise: b,
    baseSize: b,
    lang: lang || 'vi-VN',
  };
}

/** Phonetic guide: base text with its reading set above it. */
export function PhoneticGuideDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [spec, setSpec] = useState<RubySpec | null>(null);
  const [replace, setReplace] = useState(false);
  useEffect(() => {
    let live = true;
    void run({ cmd: 'state' }).then((r) => {
      if (!live) return;
      if (r?.ruby) {
        setSpec(r.ruby);
        setReplace(true);
      } else {
        setSpec(rubyDefaults(r?.format?.sizeHalfPoints ?? 24, r?.format?.lang ?? ''));
      }
    });
    return () => {
      live = false;
    };
  }, [run]);
  if (!spec) return null;
  const set = (p: Partial<RubySpec>) => setSpec({ ...spec, ...p });
  const apply = () => {
    onClose();
    if (!spec.base.trim()) return;
    void run({ cmd: 'insertRuby', ruby: spec, replace });
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('charFx.ruby.title')}
      width={440}
      testId="ruby-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="ruby" />}
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('charFx.ruby.base')}</span>
          <input
            data-testid="ruby-base"
            value={spec.base}
            onChange={(e) => set({ base: e.target.value })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('charFx.ruby.guide')}</span>
          <input
            data-testid="ruby-guide"
            value={spec.guide}
            onChange={(e) => set({ guide: e.target.value })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('charFx.ruby.align')}</span>
          <select
            data-testid="ruby-align"
            value={spec.align}
            onChange={(e) => set({ align: e.target.value })}
          >
            {RUBY_ALIGNS.map((a) => (
              <option key={a} value={a}>
                {t(`charFx.ruby.aligns.${a}`)}
              </option>
            ))}
          </select>
        </label>
        <Num
          label={t('charFx.ruby.sizePt')}
          value={spec.size / 2}
          min={1}
          max={100}
          step={0.5}
          testId="ruby-size"
          onChange={(pt) => set({ size: Math.round(pt * 2) })}
        />
        <Num
          label={t('charFx.ruby.raisePt')}
          value={spec.raise / 2}
          min={0}
          max={100}
          step={0.5}
          testId="ruby-raise"
          onChange={(pt) => set({ raise: Math.round(pt * 2) })}
        />
        <div data-testid="ruby-preview" className={styles.field}>
          <span>{t('charFx.preview')}</span>
          <ruby>
            {spec.base}
            <rt>{spec.guide}</rt>
          </ruby>
        </div>
      </div>
    </Dialog>
  );
}

/** Two lines in one: the selected text set in two half-height lines, optionally
 * between brackets. */
export function CombineCharsDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [on, setOn] = useState<boolean | null>(null);
  const [brackets, setBrackets] = useState('');
  const initial = useAdvanced(run, (a) => {
    setOn(a.combine);
    setBrackets(a.combineBrackets);
  });
  if (!initial || on === null) return null;
  const apply = () => {
    onClose();
    const patch = advancedPatch(initial, {
      ...initial,
      combine: on,
      combineBrackets: on ? brackets : '',
    });
    if (Object.keys(patch).length > 0) {
      void run({ cmd: 'formatText', format: { kind: 'advanced', patch } });
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('charFx.combine.title')}
      width={380}
      testId="combine-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="combine" />}
    >
      <div className={styles.formGrid}>
        <Check label={t('charFx.combine.on')} checked={on} testId="combine-on" onChange={setOn} />
        <label className={styles.field}>
          <span>{t('charFx.combine.brackets')}</span>
          <select
            data-testid="combine-brackets"
            disabled={!on}
            value={brackets}
            onChange={(e) => setBrackets(e.target.value)}
          >
            {BRACKETS.map((b) => (
              <option key={b} value={b}>
                {t(`charFx.combine.bracketKinds.${b || 'none'}`)}
              </option>
            ))}
          </select>
        </label>
      </div>
    </Dialog>
  );
}
