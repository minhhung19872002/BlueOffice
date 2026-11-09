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
}

const SVG = 'http://www.w3.org/2000/svg';

export const markupKey = (b: { kind: MarkKind; id: number }) => `${b.kind}:${b.id}`;

/** Keep clicks on markup from moving the caret (the scroller handles mousedown). */
function stopPointer(el: HTMLElement): void {
  el.addEventListener('mousedown', (e) => {
    e.stopPropagation();
    e.preventDefault();
  });
}

function balloonEl(b: Balloon, o: MarkupDrawOptions, x: number, y: number, w: number) {
  const { k } = o;
  const el = document.createElement('div');
  el.className = 'bo-balloon';
  el.dataset.testid = 'markup-balloon';
  el.dataset.kind = b.kind;
  el.dataset.id = String(b.id);
  el.setAttribute('role', 'button');
  el.tabIndex = -1;
  el.style.cssText =
    `position:absolute;left:${x * k}px;top:${y * k}px;width:${w * k}px;` +
    `min-height:${b.icon ? 0 : b.h * k}px;${b.icon ? '' : `height:${b.h * k}px;`}` +
    `box-sizing:border-box;overflow:hidden;padding:${4 * k}px;` +
    `font:${7.5 * k}px/${10 * k}px system-ui,sans-serif;color:#222;background:#fff;` +
    `border:1px solid ${b.color};border-left:${3 * k}px solid ${b.color};border-radius:${3 * k}px;` +
    `pointer-events:auto;cursor:pointer;opacity:${b.done ? 0.6 : 1};` +
    'box-shadow:0 1px 2px rgba(0,0,0,.15);';
  const head = document.createElement('div');
  head.style.cssText = 'font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
  head.textContent = b.author;
  const body = document.createElement('div');
  body.style.cssText = 'white-space:pre-wrap;word-break:break-word;';
  const label = b.kind === 'comment' ? '' : (o.label?.(b.kind, b.id) ?? '');
  body.textContent = label ? `${label}: ${b.text}` : b.text;
  el.title = `${b.author}\n${body.textContent}`;
  el.append(head, body);
  stopPointer(el);
  el.addEventListener('click', () => o.onSelect?.({ kind: b.kind, id: b.id }));
  return el;
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
  if (lines.length) {
    const svg = document.createElementNS(SVG, 'svg');
    svg.setAttribute('width', String((pageWidth + markup.width) * k));
    svg.setAttribute('height', String(pageHeight * k));
    svg.style.cssText = 'position:absolute;left:0;top:0;overflow:visible;pointer-events:none;';
    for (const b of lines) {
      const path = document.createElementNS(SVG, 'path');
      const y = (b.y + 6) * k;
      path.setAttribute('d', `M${b.anchorX * k} ${b.anchorY * k}H${pageWidth * k}L${b.x * k} ${y}`);
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke', b.color);
      path.setAttribute('stroke-width', '1');
      path.setAttribute('stroke-dasharray', '3 2');
      path.dataset.testid = 'balloon-connector';
      svg.appendChild(path);
    }
    layer.appendChild(svg);
  }
  for (const b of markup.balloons) {
    if (!b.icon) {
      layer.appendChild(balloonEl(b, o, b.x, b.y, b.w));
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
      layer.appendChild(balloonEl(b, o, b.x + b.w + 4, b.y, 160));
    }
  }
}
