// Performance probe of the desktop app, injected only when BLUEOFFICE_PERF_LOG is set
// (src/perf.rs, docs/perf/BASELINE.md). It records, in `window.__boPerf`:
//   marks   – start-up milestones (epoch ms), also sent to the Rust log through `perf_mark`;
//   inputs  – keydown / mousedown / wheel event times (performance.now());
//   ipc     – every Tauri IPC request: command, start, response, body read;
//   paints  – every page canvas paint: start (getContext) and end of that task;
//   renders – React components that rendered per commit, by UI region (counting is off
//             until `__boPerf.countRenders = true`; a fake DevTools hook sees each commit).
// The harness (scripts/perf-desktop.mjs) reads it through WebDriver.
(() => {
  if (window.__boPerf || window.top !== window) return;
  const now = () => performance.now();
  const epoch = (t = now()) => performance.timeOrigin + t;
  const P = {
    marks: {},
    inputs: [],
    ipc: [],
    paints: [],
    commits: [],
    countRenders: false,
    reset() {
      P.inputs.length = 0;
      P.ipc.length = 0;
      P.paints.length = 0;
      P.commits.length = 0;
    },
  };
  Object.defineProperty(window, '__boPerf', { value: P });
  const cap = (list, max = 5000) => {
    if (list.length > max) list.splice(0, list.length - max);
  };

  // ------------------------------------------------------------ marks → Rust log
  const queue = [];
  let origFetch = null;
  const ipcUrl = /^(?:https?:\/\/ipc\.localhost|ipc:\/\/localhost)\/([^?#]+)/;
  let waits = 0;
  const flush = () => {
    const ti = window.__TAURI_INTERNALS__;
    // Outside the app (the web E2E loads this probe too) the marks stay in the page.
    if (!ti || !('ipc' in ti)) return void (++waits < 500 && setTimeout(flush, 20));
    while (queue.length) {
      const [event, data] = queue.shift();
      ti.invoke('perf_mark', { event, data }).catch(() => {});
    }
  };
  const mark = (name, t = now(), data = {}) => {
    if (P.marks[name] !== undefined) return;
    P.marks[name] = epoch(t);
    queue.push([name, { at: P.marks[name], ...data }]);
    flush();
  };
  /** Calls `fn` once the frame being produced now has been presented (approximately). */
  const afterFrame = (fn) =>
    requestAnimationFrame(() => {
      const ch = new MessageChannel();
      ch.port1.onmessage = () => fn(now());
      ch.port2.postMessage(0);
    });
  mark('nav-start', 0);
  document.addEventListener('DOMContentLoaded', () => mark('dom-content-loaded'));
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) mark(e.name, e.startTime);
    }).observe({ type: 'paint', buffered: true });
  } catch {
    /* no paint timing */
  }

  // ------------------------------------------------------------ inputs
  for (const type of ['keydown', 'mousedown', 'wheel'])
    window.addEventListener(
      type,
      (e) => {
        P.inputs.push({ type, key: e.key, t: e.timeStamp, seen: now() });
        cap(P.inputs);
      },
      { capture: true, passive: true },
    );

  // ------------------------------------------------------------ IPC (custom protocol fetch)
  // Commands of the start-up / open path, logged once (first call) with their IPC time.
  const LOGGED = new Set(['startup_file', 'read_file', 'engine_init', 'engine_open', 'sheet_open']);
  origFetch = window.fetch;
  window.fetch = function (input) {
    const url = typeof input === 'string' ? input : input && input.url;
    const m = url && ipcUrl.exec(url);
    if (!m) return origFetch.apply(this, arguments);
    const rec = { cmd: decodeURIComponent(m[1]), start: now(), response: 0, end: 0 };
    if (rec.cmd !== 'perf_mark') {
      P.ipc.push(rec);
      cap(P.ipc);
    }
    return origFetch.apply(this, arguments).then((r) => {
      rec.response = now();
      for (const k of ['json', 'text', 'arrayBuffer']) {
        const f = r[k].bind(r);
        r[k] = () =>
          f().then((v) => {
            rec.end = now();
            if (LOGGED.has(rec.cmd))
              mark(`ipc:${rec.cmd}`, rec.start, { end: epoch(rec.end), ms: rec.end - rec.start });
            return v;
          });
      }
      return r;
    });
  };

  // ------------------------------------------------------------ page canvas paints
  const getContext = HTMLCanvasElement.prototype.getContext;
  const isPage = (c) => !!c.parentElement && c.parentElement.classList.contains('bo-page');
  HTMLCanvasElement.prototype.getContext = function () {
    // A page's first canvas is drawn before it is put into the page: decided after the draw.
    if (isPage(this) || !this.parentElement) {
      const canvas = this;
      const rec = { start: now(), end: 0, w: this.width, h: this.height };
      // The draw is synchronous after getContext: the next task starts when it is done.
      const ch = new MessageChannel();
      ch.port1.onmessage = () => {
        if (!isPage(canvas)) return;
        rec.end = now();
        P.paints.push(rec);
        cap(P.paints);
        if (P.marks['page-painted'] === undefined) {
          mark('page-painted', rec.start);
          afterFrame((t) => mark('page-presented', t));
          check(); // the caret may already be there (no DOM change follows)
        }
      };
      ch.port2.postMessage(0);
    }
    return getContext.apply(this, arguments);
  };

  // ------------------------------------------------------------ shell / editor milestones
  const check = () => {
    const root = document.getElementById('root');
    if (P.marks['shell-visible'] === undefined && root && root.firstElementChild) {
      P.marks['shell-visible'] = -1; // pending
      afterFrame((t) => {
        delete P.marks['shell-visible'];
        mark('shell-visible', t);
      });
    }
    if (P.marks['ribbon-visible'] === undefined && document.querySelector('[data-testid=ribbon]')) {
      P.marks['ribbon-visible'] = -1;
      afterFrame((t) => {
        delete P.marks['ribbon-visible'];
        mark('ribbon-visible', t);
      });
    }
    if (
      P.marks['start-screen'] === undefined &&
      document.querySelector('[data-testid^="start-new-"]')
    ) {
      P.marks['start-screen'] = -1;
      afterFrame((t) => {
        delete P.marks['start-screen'];
        mark('start-screen', t);
      });
    }
    if (
      P.marks['editor-ready'] === undefined &&
      P.marks['page-painted'] !== undefined &&
      document.querySelector('[data-testid=caret]')
    ) {
      P.marks['editor-ready'] = -1;
      afterFrame((t) => {
        delete P.marks['editor-ready'];
        mark('editor-ready', t);
        watch.disconnect();
      });
    }
  };
  const watch = new MutationObserver(check);
  watch.observe(document, { childList: true, subtree: true });

  // ------------------------------------------------------------ React renders per commit
  const REGIONS = [
    ['ribbon', '[data-testid=ribbon]'],
    ['status', '[data-testid=status-bar]'],
    ['title', '[data-testid=title-bar],[data-testid^=qat]'],
    ['pane', '[data-testid^=taskpane-]'],
    ['formula', '[data-testid=formula-bar]'],
  ];
  const COMPOSITE = new Set([0, 1, 11, 14, 15]); // function, class, forwardRef, memo, simple memo
  const hostOf = (fiber) => {
    for (let f = fiber; f; f = f.child) if (f.tag === 5 && f.stateNode instanceof Element) return f.stateNode;
    return null;
  };
  const regionOf = (fiber) => {
    const el = hostOf(fiber);
    if (!el || !el.isConnected) return 'other';
    for (const [name, sel] of REGIONS) if (el.closest(sel)) return name;
    return 'other';
  };
  const nameOf = (f) => {
    const t = f.type && (f.type.type || f.type.render || f.type);
    return (t && (t.displayName || t.name)) || '?';
  };
  let commit = null;
  const count = (f, mounted) => {
    // Skipped: stableMemo's pass-through wrapper (packages/ui/src/stableMemo.tsx).
    if (!COMPOSITE.has(f.tag) || (f.type && f.type.boStable)) return;
    const region = regionOf(f);
    commit.byRegion[region] = (commit.byRegion[region] || 0) + 1;
    if (mounted) commit.mounts += 1;
    if (region !== 'other') commit.names.push(`${region}:${nameOf(f)}`);
  };
  const mountTree = (f) => {
    for (let c = f; c; c = c.sibling) {
      count(c, true);
      if (c.child) mountTree(c.child);
    }
  };
  const update = (next, prev) => {
    if (COMPOSITE.has(next.tag) && (next.flags & 1) === 1) count(next, false);
    if (next.child === prev.child) return;
    for (let c = next.child; c; c = c.sibling) {
      if (c.alternate) update(c, c.alternate);
      else {
        count(c, true);
        if (c.child) mountTree(c.child);
      }
    }
  };
  if (!window.__REACT_DEVTOOLS_GLOBAL_HOOK__) {
    window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
      renderers: new Map(),
      supportsFiber: true,
      isDisabled: false,
      inject(r) {
        const id = this.renderers.size + 1;
        this.renderers.set(id, r);
        return id;
      },
      checkDCE() {},
      onScheduleFiberRoot() {},
      onCommitFiberUnmount() {},
      onPostCommitFiberRoot() {},
      onCommitFiberRoot(_id, root) {
        if (!P.countRenders) return;
        commit = { t: now(), byRegion: {}, mounts: 0, names: [] };
        try {
          const cur = root.current;
          if (cur.alternate) update(cur, cur.alternate);
          else mountTree(cur);
        } catch (e) {
          commit.error = String(e);
        }
        if (commit.names.length > 40) commit.names.length = 40;
        P.commits.push(commit);
        cap(P.commits, 2000);
      },
    };
  }
})();
