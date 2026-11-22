//! Drag-and-drop text editing of the main text (BlueDoc round 9): the selection dragged to
//! another place moves there (or is copied with Ctrl), as one undo step; the dropped text is
//! selected afterwards, as on the desktop word processor.
use crate::edit::{Command, EditResult};
use crate::Engine;
use bo_edit::{Pos, Selection};

impl Engine {
    /// Move (or copy) the selected main text to `to`. Nothing happens when the selection is
    /// empty or `to` lies inside it.
    pub(crate) fn move_text(
        &mut self,
        to: Pos,
        copy: bool,
        now: Option<i64>,
    ) -> crate::Result<EditResult> {
        let Some(s) = self.session.as_ref() else {
            return self.command_inner(Command::State, now);
        };
        let sel = s.selection();
        let (a, b) = sel.ordered();
        if sel.is_collapsed() || (to >= a && to <= b) {
            return self.command_inner(Command::State, now);
        }
        let depth = s.history_len();
        let clip = s.copy();
        let tracked = s.track_changes();
        let mut dirty = Vec::new();
        let mut target = to;
        if !copy {
            dirty.extend(self.command_inner(Command::DeleteForward, now)?.dirty);
            // The deletion joins `a` and `b`: positions after it shift (unless the deleted text
            // stays in place as a tracked deletion).
            let gone = self
                .session
                .as_ref()
                .is_some_and(|s| s.selection().focus == a);
            if to > b && gone && !tracked {
                target = if to.para == b.para {
                    Pos::new(a.para, a.offset + (to.offset - b.offset))
                } else {
                    Pos::new(to.para - (b.para - a.para), to.offset)
                };
            }
            // Smart cut: the space left doubled (or before punctuation) goes too.
            if gone && !self.smart_off {
                if let Some(s) = self.session.as_mut() {
                    if crate::smart_cut::fix_after_cut(s, &clip.text)
                        .map_err(crate::smart_cut::bad)?
                    {
                        let at = s.selection().focus;
                        let removed = at.offset.min(a.offset);
                        if target.para == a.para && target.offset > removed {
                            target.offset -= 1;
                        }
                    }
                }
            }
        }
        self.command_inner(
            Command::SetSelection {
                selection: Selection {
                    anchor: target,
                    focus: target,
                },
            },
            now,
        )?;
        let r = self.command_inner(
            Command::Paste {
                internal: Some(clip.internal),
                html: None,
                text: None,
                mode: None,
            },
            now,
        )?;
        dirty.extend(r.dirty);
        if let Some(s) = self.session.as_mut() {
            let end = s.selection().focus;
            s.set_selection(Selection {
                anchor: target,
                focus: end,
            });
            // Smart paste: the dropped words get the spaces they need (still selected).
            if !self.smart_off {
                s.set_selection(Selection::caret(end));
                if !crate::smart_cut::fix_after_paste(s, target, true)
                    .map_err(crate::smart_cut::bad)?
                {
                    s.set_selection(Selection {
                        anchor: target,
                        focus: end,
                    });
                }
            }
            s.merge_history_since(depth);
        }
        dirty.extend(self.relayout()?);
        dirty.sort_unstable();
        dirty.dedup();
        self.edit_result(dirty, (None, None, None), None)
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;
    use bo_edit::{Pos, Selection};

    fn engine(text: &[&str]) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
        for (i, t) in text.iter().enumerate() {
            if i > 0 {
                e.command(Command::SplitParagraph).expect("enter");
            }
            e.command(Command::InsertText {
                text: (*t).to_owned(),
            })
            .expect("type");
        }
        e
    }

    fn select(e: &mut Engine, a: Pos, b: Pos) {
        e.command(Command::SetSelection {
            selection: Selection {
                anchor: a,
                focus: b,
            },
        })
        .expect("select");
    }

    fn text(e: &Engine) -> Vec<String> {
        let s = e.session.as_ref().expect("session");
        (0..s.paragraph_count())
            .map(|i| s.para_text(i).unwrap_or_default())
            .collect()
    }

    #[test]
    fn moving_a_word_forward_and_undoing_it_in_one_step() {
        let mut e = engine(&["one two three"]);
        select(&mut e, Pos::new(0, 0), Pos::new(0, 4));
        let r = e
            .command(Command::MoveText {
                to: Pos::new(0, 13),
                copy: false,
            })
            .expect("move");
        // Smart paste: the space travels to the other side ("two three one").
        assert_eq!(text(&e), ["two three one"]);
        let (a, b) = r.selection.ordered();
        assert_eq!((a, b), (Pos::new(0, 10), Pos::new(0, 13)));
        e.command(Command::Undo).expect("undo");
        assert_eq!(text(&e), ["one two three"]);
    }

    #[test]
    fn copying_across_paragraphs_and_dropping_inside_does_nothing() {
        let mut e = engine(&["alpha", "beta gamma", "delta"]);
        select(&mut e, Pos::new(1, 5), Pos::new(1, 10));
        e.command(Command::MoveText {
            to: Pos::new(0, 0),
            copy: true,
        })
        .expect("copy");
        assert_eq!(text(&e), ["gamma alpha", "beta gamma", "delta"]);
        select(&mut e, Pos::new(0, 0), Pos::new(1, 2));
        e.command(Command::MoveText {
            to: Pos::new(2, 5),
            copy: false,
        })
        .expect("move lines");
        assert_eq!(text(&e), ["ta gamma", "deltagamma alpha", "be"]);
        let before = text(&e);
        select(&mut e, Pos::new(0, 0), Pos::new(0, 4));
        e.command(Command::MoveText {
            to: Pos::new(0, 2),
            copy: false,
        })
        .expect("inside");
        assert_eq!(text(&e), before);
    }
}
