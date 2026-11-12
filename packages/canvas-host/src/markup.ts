import type { Balloon, MarkKind, PageMarkup } from '@blueoffice/platform';

/** A balloon or comment icon was clicked. */
export interface MarkupSelect {
  kind: MarkKind;
  id: number;
}

export interface MarkupDrawOptions {
  /** CSS pixels per point (zoom included). */
  k: number;
  /** Page width in points (the markup area starts there). */
  pageWidth: number;
  /** Page height in points. */
  pageHeight: number;
  /** Localised label of a change kind (of change `id`), shown before its balloon text. */
  label?: (kind: MarkKind, id: number) => string;
  /** Icons (simple markup) whose balloon is open, by `kind:id`. */
  open: Set<string>;
  onSelect?: (m: MarkupSelect) => void;
  /** Redraw after an icon was opened or closed. */
  onToggle?: () => void;
  /** Reply to comment `id` from its balloon (absent: no reply box). */
  onReply?: (id: number, text: string) => void;
  /** Localised labels of the reply box. */
  replyLabels?: { reply: string; send: string; cancel: string; placeholder: string };
}

/** Space between stacked balloons (points), as the engine stacks them. */
const GAP = 4;

const SVG = 'http://www.w3.org/2000/svg';

export const markupKey = (b: { kind: MarkKind; id: number }) => `${b.kind}:${b.id}`;

/** Keep clicks on markup from moving the caret (the scroller handles mousedown). */
function stopPointer(el: HTMLElement): void {
  el.addEventListener('mousedown', (e) => {
    e.stopPropagation();
    e.preventDefault();
  });
}

function balloonEl(
  b: Balloon,
  o: MarkupDrawOptions,
  x: number,
  y: number,
  w: number,
  relayout: () => void,
) {
  const { k } = o;
  const el = document.createElement('div');
  el.className = 'bo-balloon';
  el.dataset.testid = 'markup-balloon';
  el.dataset.kind = b.kind;
  el.dataset.id = String(b.id);
  // The balloon is as tall as its text really is (measured once drawn, then the
  // balloons are stacked again: no estimate, nothing clipped).
  el.style.cssText =
    `position:absolute;left:${x * k}px;top:${y * k}px;width:${w * k}px;` +
    `min-height:${b.icon ? 0 : b.h * k}px;` +
    `box-sizing:border-box;padding:${4 * k}px;` +
    `font:${7.5 * k}px/${10 * k}px system-ui,sans-serif;color:#222;background:#fff;` +
    `border:1px solid ${b.color};border-left:${3 * k}px solid ${b.color};border-radius:${3 * k}px;` +
    `pointer-events:auto;cursor:pointer;opacity:${b.done ? 0.6 : 1};` +
    'box-shadow:0 1px 2px rgba(0,0,0,.15);';
  // The author and text: a click there goes to the mark.
  const main = document.createElement('div');
  main.setAttribute('role', 'button');
  main.tabIndex = -1;
  const head = document.createElement('div');
  head.style.cssText = 'font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
  head.textContent = b.author;
  const body = document.createElement('div');
  body.style.cssText = 'white-space:pre-wrap;word-break:break-word;';
  const label = b.kind === 'comment' ? '' : (o.label?.(b.kind, b.id) ?? '');
  body.textContent = label ? `${label}: ${b.text}` : b.text;
  main.title = `${b.author}\n${body.textContent}`;
  main.append(head, body);
  el.appendChild(main);
  stopPointer(el);
  el.addEventListener('click', () => o.onSelect?.({ kind: b.kind, id: b.id }));
  if (b.kind === 'comment' && !b.done && o.onReply && o.replyLabels) {
    el.appendChild(replyBox(b, o, relayout));
  }
  return el;
}

/** "Reply" in a comment balloon: opens a box typed in place; Ctrl+Enter or the send
 * button replies (the reply is a comment of its own, §17.13.4.2 with
 * `w15:paraIdParent`), Esc or cancel closes it. */
function replyBox(b: Balloon, o: MarkupDrawOptions, relayout: () => void): HTMLElement {
  const { k } = o;
  const labels = o.replyLabels!;
  const wrap = document.createElement('div');
  wrap.style.cssText = `margin-top:${2 * k}px;`;
  const keep = (e: Event) => e.stopPropagation();
  wrap.addEventListener('click', keep);
  const small = `font:inherit;min-height:24px;padding:0 ${4 * k}px;cursor:pointer;`;
  const open = document.createElement('button');
  open.type = 'button';
  open.dataset.testid = 'balloon-reply';
  open.textContent = labels.reply;
  open.style.cssText = `${small}border:none;background:none;color:#1a4f8a;text-decoration:underline;`;
  const form = document.createElement('div');
  form.hidden = true;
  const input = document.createElement('textarea');
  input.dataset.testid = 'balloon-reply-text';
  input.setAttribute('aria-label', labels.reply);
  input.placeholder = labels.placeholder;
  input.rows = 2;
  input.style.cssText = `display:block;width:100%;box-sizing:border-box;font:inherit;resize:vertical;`;
  const send = document.createElement('button');
  send.type = 'button';
  send.dataset.testid = 'balloon-reply-send';
  send.textContent = labels.send;
  send.style.cssText = small;
  const cancel = document.createElement('button');
  cancel.type = 'button';
  cancel.textContent = labels.cancel;
  cancel.style.cssText = small;
  const show = (on: boolean) => {
    form.hidden = !on;
    open.hidden = on;
    if (on) input.focus();
    else input.value = '';
    relayout();
  };
  const submit = () => {
    const text = input.value.trim();
    if (!text) return;
    show(false);
    o.onReply?.(b.id, text);
  };
  open.addEventListener('click', () => show(true));
  cancel.addEventListener('click', () => show(false));
  send.addEventListener('click', submit);
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Escape') show(false);
    else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submit();
    }
  });
  // Typing in the box is not typing in the document.
  for (const t of ['mousedown', 'keyup', 'keypress', 'input'] as const) {
    input.addEventListener(t, keep);
  }
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:4px;justify-content:flex-end;';
  row.append(cancel, send);
  form.append(input, row);
  wrap.append(open, form);
  return wrap;
}

/** Stack balloons top to bottom from their anchors with their drawn heights (points),
 * pushed up from the page bottom when they overflow (as the engine stacks them). */
export function stackHeights(
  items: { anchorY: number; h: number }[],
  pageHeight: number,
): number[] {
  const ys: number[] = [];
  let bottom = -Infinity;
  for (const it of items) {
    const y = Math.max(it.anchorY - 8, bottom + GAP, 0);
    ys.push(y);
    bottom = y + it.h;
  }
  let limit = pageHeight - GAP;
  for (let i = items.length - 1; i >= 0; i--) {
    const h = items[i]!.h;
    if (ys[i]! + h > limit) ys[i] = Math.max(0, limit - h);
    limit = ys[i]! - GAP;
  }
  bottom = -Infinity;
  for (let i = 0; i < items.length; i++) {
    ys[i] = Math.max(ys[i]!, bottom + GAP);
    bottom = ys[i]! + items[i]!.h;
  }
  return ys;
}

/**
 * Draw a page's review markup into `layer` (positioned over the page, top-left at the
 * page's top-left): the markup area right of the page, change bars, balloons with
 * dashed connectors to their anchors, or comment icons (simple markup) that open
 * their balloon.
 */
export function drawMarkup(
  layer: HTMLElement,
  markup: PageMarkup | undefined,
  o: MarkupDrawOptions,
): void {
  layer.replaceChildren();
  if (!markup) return;
  const { k, pageWidth, pageHeight } = o;
  if (markup.width > 0) {
    const area = document.createElement('div');
    area.dataset.testid = 'markup-area';
    area.style.cssText = `position:absolute;left:${pageWidth * k}px;top:0;width:${markup.width * k}px;height:${pageHeight * k}px;background:rgba(0,0,0,.035);border-left:1px solid rgba(0,0,0,.08);`;
    layer.appendChild(area);
  }
  for (const bar of markup.bars) {
    const d = document.createElement('div');
    d.dataset.testid = 'change-bar';
    d.style.cssText = `position:absolute;left:${bar.x * k}px;top:${bar.top * k}px;width:${Math.max(1.5, k)}px;height:${(bar.bottom - bar.top) * k}px;background:${bar.color};`;
    layer.appendChild(d);
  }
  const lines = markup.balloons.filter((b) => !b.icon);
  const paths: SVGPathElement[] = [];
  if (lines.length) {
    const svg = document.createElementNS(SVG, 'svg');
    svg.setAttribute('width', String((pageWidth + markup.width) * k));
    svg.setAttribute('height', String(pageHeight * k));
    svg.style.cssText = 'position:absolute;left:0;top:0;overflow:visible;pointer-events:none;';
    for (const b of lines) {
      const path = document.createElementNS(SVG, 'path');
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke', b.color);
      path.setAttribute('stroke-width', '1');
      path.setAttribute('stroke-dasharray', '3 2');
      path.dataset.testid = 'balloon-connector';
      svg.appendChild(path);
      paths.push(path);
    }
    layer.appendChild(svg);
  }
  const drawn: HTMLElement[] = [];
  /** Place the balloons by their drawn heights and point the connectors at them. */
  const relayout = () => {
    const ys = stackHeights(
      lines.map((b, i) => {
        const px = drawn[i]?.offsetHeight ?? 0;
        return { anchorY: b.anchorY, h: px > 0 ? px / k : b.h };
      }),
      pageHeight,
    );
    lines.forEach((b, i) => {
      const y = ys[i] ?? b.y;
      const el = drawn[i];
      if (el) el.style.top = `${y * k}px`;
      paths[i]?.setAttribute(
        'd',
        `M${b.anchorX * k} ${b.anchorY * k}H${pageWidth * k}L${b.x * k} ${(y + 6) * k}`,
      );
    });
  };
  for (const b of markup.balloons) {
    if (!b.icon) {
      const el = balloonEl(b, o, b.x, b.y, b.w, relayout);
      drawn.push(el);
      layer.appendChild(el);
      continue;
    }
    const key = markupKey(b);
    const icon = document.createElement('button');
    icon.type = 'button';
    icon.className = 'bo-markup-icon';
    icon.dataset.testid = 'markup-icon';
    icon.dataset.id = String(b.id);
    icon.setAttribute('aria-label', b.author);
    icon.setAttribute('aria-expanded', o.open.has(key) ? 'true' : 'false');
    icon.title = `${b.author}: ${b.text}`;
    icon.style.cssText = `position:absolute;left:${b.x * k}px;top:${b.y * k}px;width:${b.w * k}px;height:${b.h * k}px;padding:0;border:1px solid ${b.color};border-radius:${2 * k}px;background:#fff;pointer-events:auto;cursor:pointer;`;
    const dot = document.createElement('span');
    dot.style.cssText = `display:block;margin:${2 * k}px;height:${(b.h - 6) * k}px;border-radius:${1.5 * k}px;background:${b.color};opacity:.75;`;
    icon.appendChild(dot);
    stopPointer(icon);
    icon.addEventListener('click', () => {
      if (o.open.has(key)) o.open.delete(key);
      else o.open.add(key);
      o.onToggle?.();
    });
    layer.appendChild(icon);
    if (o.open.has(key)) {
      // The opened balloon floats right of the icon strip.
      layer.appendChild(balloonEl(b, o, b.x + b.w + 4, b.y, 160, () => {}));
    }
  }
  // Heights are known once the balloons are in the page (else the engine's sizes).
  if (drawn.some((el) => el.offsetHeight > 0)) relayout();
  else {
    lines.forEach((b, i) =>
      paths[i]?.setAttribute(
        'd',
        `M${b.anchorX * k} ${b.anchorY * k}H${pageWidth * k}L${b.x * k} ${(b.y + 6) * k}`,
      ),
    );
    if (typeof requestAnimationFrame === 'function' && lines.length) {
      requestAnimationFrame(() => {
        if (drawn.some((el) => el.isConnected && el.offsetHeight > 0)) relayout();
      });
    }
  }
}
