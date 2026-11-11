import type { SlideSmartArtNode } from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';

/**
 * Edits the text of one node of a diagram in place (CORE-SMART-004): a text box over the shape
 * that shows the node. Enter or leaving the box commits (the data model is rewritten: one undo
 * step), Escape gives up. A diagram's shapes are drawn from its drawing part and are not
 * shapes of the slide, so this is its own small editor rather than the slide's text editing.
 */
export function SmartArtNodeEditor({
  node,
  scale,
  label,
  onDone,
}: {
  node: SlideSmartArtNode;
  /** CSS pixels per point. */
  scale: number;
  label: string;
  /** The new text, or `null` when the edit was given up. */
  onDone: (text: string | null) => void;
}) {
  const [text, setText] = useState(node.text);
  const box = useRef<HTMLTextAreaElement>(null);
  const ended = useRef(false);
  useEffect(() => {
    box.current?.focus();
    box.current?.select();
  }, []);
  const end = (value: string | null) => {
    if (ended.current) return;
    ended.current = true;
    onDone(value);
  };
  return (
    <textarea
      ref={box}
      data-testid="smartart-node-editor"
      aria-label={label}
      value={text}
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => end(text)}
      onPointerDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        // The slide's shortcuts (Delete, arrows, typing to start an edit) stay out of the box.
        e.stopPropagation();
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          end(text);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          end(null);
        }
      }}
      style={{
        position: 'absolute',
        left: node.x * scale,
        top: node.y * scale,
        width: Math.max(48, node.w * scale),
        height: Math.max(28, node.h * scale),
        zIndex: 6,
        boxSizing: 'border-box',
        resize: 'none',
        padding: 4,
        textAlign: 'center',
        font: `${Math.max(11, Math.min(18, node.h * scale * 0.28))}px system-ui, sans-serif`,
        border: '2px solid #1f6fb2',
        borderRadius: 4,
        background: '#fff',
        color: '#111',
      }}
    />
  );
}
