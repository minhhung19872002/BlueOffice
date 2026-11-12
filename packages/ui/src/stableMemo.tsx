import {
  cloneElement,
  createElement,
  isValidElement,
  memo,
  useLayoutEffect,
  useRef,
  type ComponentType,
  type ReactElement,
} from 'react';

/**
 * Equality of props built afresh on every render of a big editor component (ribbon tabs,
 * status bar items, …): plain objects, arrays and React elements are compared by content,
 * functions are all equal (they are event callbacks; see {@link stableMemo}), anything else
 * (class instances, DOM nodes, maps) by identity.
 */
export function sameContent(a: unknown, b: unknown, depth = 0): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a === 'function' && typeof b === 'function') return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  if (depth > 64) return false;
  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (!sameContent(a[i], b[i], depth + 1)) return false;
    return true;
  }
  if (Array.isArray(b)) return false;
  if (isValidElement(a) || isValidElement(b)) {
    if (!isValidElement(a) || !isValidElement(b)) return false;
    const ea = a as ReactElement & { ref?: unknown };
    const eb = b as ReactElement & { ref?: unknown };
    return (
      ea.type === eb.type &&
      ea.key === eb.key &&
      sameContent(ea.ref, eb.ref, depth + 1) &&
      sameContent(ea.props, eb.props, depth + 1)
    );
  }
  if (!isPlain(a) || !isPlain(b)) return false;
  const ka = Object.keys(a);
  if (ka.length !== Object.keys(b).length) return false;
  for (const k of ka) {
    if (!Object.prototype.hasOwnProperty.call(b, k)) return false;
    if (
      !sameContent((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], depth + 1)
    )
      return false;
  }
  return true;
}

function isPlain(v: object): boolean {
  const p: unknown = Object.getPrototypeOf(v);
  return p === Object.prototype || p === null;
}

type Path = readonly (string | number)[];

/** The value at `path` of `root` (through arrays, plain objects and element props). */
function at(root: unknown, path: Path): unknown {
  let v: unknown = root;
  for (const k of path) {
    if (v === null || typeof v !== 'object') return undefined;
    v = isValidElement(v)
      ? k === 'ref'
        ? (v as ReactElement & { ref?: unknown }).ref
        : (v.props as Record<string | number, unknown>)[k]
      : (v as Record<string | number, unknown>)[k];
  }
  return v;
}

/**
 * `value` with every function replaced by a forwarder calling the function found at the same
 * place of `latest()` (the props of the last render) — so a memoised component that skipped
 * re-rendering still calls the current callbacks. Parts without functions are returned as is.
 */
function forwarders(value: unknown, path: Path, latest: () => unknown, depth = 0): unknown {
  if (typeof value === 'function') {
    const own = value as (...a: unknown[]) => unknown;
    return function (this: unknown, ...a: unknown[]) {
      const f = at(latest(), path);
      return (typeof f === 'function' ? f : own).apply(this, a);
    };
  }
  if (typeof value !== 'object' || value === null || depth > 64) return value;
  if (Array.isArray(value)) {
    let changed = false;
    const out = value.map((v, i) => {
      const w = forwarders(v, [...path, i], latest, depth + 1);
      if (w !== v) changed = true;
      return w;
    });
    return changed ? out : value;
  }
  if (isValidElement(value)) {
    const props = value.props as Record<string, unknown>;
    const next = forwarders(props, path, latest, depth + 1);
    // `ref` stays as it is (callback refs are called by React, not by the component).
    return next === props ? value : cloneElement(value, next as Record<string, unknown>);
  }
  if (!isPlain(value)) return value;
  let changed = false;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value)) {
    const w = forwarders(v, [...path, k], latest, depth + 1);
    if (w !== v) changed = true;
    out[k] = w;
  }
  return changed ? out : value;
}

/**
 * `Component` re-rendered only when its props change in content ({@link sameContent}), for
 * chrome that the editor rebuilds on every keystroke although nothing in it changed (ribbon,
 * status bar, title bar): typing must not re-render them (docs/perf/RESULTS.md, step 4).
 *
 * Function props are not compared: they must be event callbacks (or lookups called from
 * event handlers). They are forwarded to the latest render's functions, so a skipped render
 * never calls a stale closure. A function whose *result* is rendered (a render prop) would go
 * stale — do not wrap components taking one.
 */
export function stableMemo<P extends object>(Component: ComponentType<P>): ComponentType<P> {
  const Memo = memo(Component, (a, b) => {
    const same = sameContent(a, b);
    // Diagnostics (perf work): `window.__boDiff = []` collects why a render was not skipped.
    const w = globalThis as unknown as { __boDiff?: string[] };
    if (!same && w.__boDiff) w.__boDiff.push(diffPath(a, b, ''));
    return same;
  });
  Memo.displayName = `Stable(${Component.displayName ?? Component.name ?? 'Component'})`;
  function Stable(props: P) {
    const latest = useRef(props);
    // Callbacks run from events, after the commit that made these props current.
    useLayoutEffect(() => {
      latest.current = props;
    });
    const stable = forwarders(props, [], () => latest.current) as P;
    return createElement(Memo as unknown as ComponentType<P>, stable);
  }
  // The perf probe does not count this pass-through render (apps/desktop/.../perf-probe.js).
  (Stable as unknown as { boStable: boolean }).boStable = true;
  Stable.displayName = `StableProps(${Component.displayName ?? Component.name ?? 'Component'})`;
  return Stable;
}

/** Where `a` and `b` first differ (by {@link sameContent}), e.g. `.tabs.2[design].enabled`. */
function diffPath(a: unknown, b: unknown, p: string): string {
  if (sameContent(a, b)) return '';
  if (typeof a !== 'object' || typeof b !== 'object' || !a || !b)
    return `${p}=${String(a).slice(0, 40)}|${String(b).slice(0, 40)}`;
  if (isValidElement(a) && isValidElement(b)) {
    if (a.type !== b.type) return `${p}.type`;
    return diffPath(
      a.props,
      b.props,
      `${p}<${typeof a.type === 'string' ? a.type : (a.type as { name?: string }).name}>`,
    );
  }
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
    const ida = (a as { id?: unknown }).id;
    const d = diffPath(
      (a as Record<string, unknown>)[k],
      (b as Record<string, unknown>)[k],
      `${p}${ida !== undefined ? `[${String(ida)}|${String((b as { id?: unknown }).id)}]` : ''}.${k}`,
    );
    if (d) return d;
  }
  return `${p}(proto)`;
}
