import { startTransition, useEffect, useMemo, useState } from 'react';
import { useStore } from 'zustand';
import { useShallow } from 'zustand/react/shallow';
import type { StoreApi } from 'zustand/vanilla';

const pick = <S extends object, K extends keyof S>(s: S, keys: readonly K[]): Pick<S, K> => {
  const out = {} as Pick<S, K>;
  for (const k of keys) out[k] = s[k];
  return out;
};

/**
 * The whole state of `store`, like zustand's `useStore(store)`, except that changes of the
 * `deferred` fields re-render in a transition: React renders the component in small slices
 * after the current task instead of synchronously inside it. The engine's answer to a
 * keystroke (`edit`, `doc`, …) repaints the page canvas first (DocumentView, outside React)
 * and the editor around it after, so the glyph is not painted after a whole editor render
 * (docs/perf/RESULTS.md, step 4). `store.getState()` is always current; the other fields
 * re-render as with `useStore`.
 */
export function useDeferredStore<S extends object, K extends keyof S>(
  store: StoreApi<S>,
  deferred: readonly K[],
): S {
  const now = useStore(
    store,
    useShallow((s: S) => {
      const rest = { ...s };
      for (const k of deferred) delete rest[k];
      return rest as Omit<S, K>;
    }),
  );
  const [later, setLater] = useState(() => pick(store.getState(), deferred));
  useEffect(() => {
    const update = (s: S) => startTransition(() => setLater(pick(s, deferred)));
    const unsubscribe = store.subscribe((s, prev) => {
      if (deferred.some((k) => s[k] !== prev[k])) update(s);
    });
    // Changes between the first render and this subscription.
    const s = store.getState();
    if (deferred.some((k) => s[k] !== later[k])) queueMicrotask(() => update(store.getState()));
    return unsubscribe;
    // `later` only matters at subscription time; `deferred` is a constant list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [store]);
  return useMemo(() => ({ ...now, ...later }) as S, [now, later]);
}
