//! Hyperlinks and actions of shapes and text (SLIDE-ACTION-001).
//!
//! - A shape carries its click and hover action in `p:cNvPr` (ECMA-376 Part 1,
//!   §20.1.2.2.8): `a:hlinkClick` (§21.1.2.3.5) and `a:hlinkHover` (§21.1.2.3.6).
//!   A text run carries them in `a:rPr` (§21.1.2.3.9): `a:hlinkClick` and
//!   `a:hlinkMouseOver` (§21.1.2.3.7).
//! - The target is a relationship of the slide part (ECMA-376 Part 2, §9.3): an
//!   external `hyperlink` one for URLs, mail addresses and files, an internal
//!   `slide` one for `ppaction://hlinksldjump`. Jumps and custom shows have no
//!   target (`r:id=""`).
//! - `a:snd` (§20.1.2.2.32) plays an embedded WAV, `endSnd` stops the one
//!   playing, `highlightClick` flashes the shape, `tooltip` is the screen tip.
//!
//! The elements stay in the model as kept children ([`Extra`]); they are read
//! and rewritten through [`bo_drawingml::hlink`]. Actions this module does not
//! know are reported as [`LinkAction::Unknown`] and never rewritten.
//!
//! **Security.** The engine only reports what an action asks for; it never
//! opens, runs or fetches anything. [`safety`] is the policy the hosts follow:
//! jumps inside the deck are `internal`, `http` / `https` / `mailto` targets are
//! `external` (opened by the host after the user confirms), and everything else
//! — programs, macros, files, other presentations, other URL schemes, unknown
//! actions — is `blocked`. Program and macro actions cannot be created here.

use super::geom::{find, place_slide, Placed, Rect};
use super::ops::{node_at, OpError, SlideOp, Tx};
use super::present::{custom_shows, LinkAction};
use super::show::SoundOut;
use super::{build, clip, refuse, rich, text, EditError, Outcome, SlideEditResult, SlideSel, R};
use crate::{Engine, EngineError, Result};
use bo_drawingml::hlink::{self, ActionKind, HlinkSound, Hyperlink, ShowJump};
use bo_model::slide::{Extra, Presentation, RawPart, Rel, RunProps, ShapeNode, TextRun};
use bo_text::FontManager;
use serde::{Deserialize, Serialize};

/// Sort key of `a:hlinkClick` in CT_NonVisualDrawingProps (§20.1.2.2.8: 1st child).
const KEY_CLICK: u16 = 2;
/// Sort key of `a:hlinkHover` in CT_NonVisualDrawingProps (§20.1.2.2.8: 2nd child).
const KEY_HOVER: u16 = 4;
/// Longest URL, path or tooltip kept (characters).
const MAX_TEXT: usize = 2048;
/// Largest embedded sound (bytes).
const MAX_SOUND: usize = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// What starts the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// A click (`a:hlinkClick`).
    #[default]
    Click,
    /// The pointer moving over it (`a:hlinkHover`, `a:hlinkMouseOver`).
    Hover,
}

/// What a host may do with an action in the slide show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    /// Stays inside the deck: followed at once.
    Internal,
    /// An `http`, `https` or `mailto` target: opened by the host after the user confirms.
    External,
    /// Never run or opened (programs, macros, files, other schemes, unknown actions).
    Blocked,
}

/// A hot area of the slide show: a shape or a piece of linked text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkOut {
    /// Shape id (`cNvPr@id`).
    pub shape_id: u32,
    /// Bounds `[x, y, w, h]` in points.
    pub rect: [f64; 4],
    /// What the link does.
    pub action: LinkAction,
    /// Started by a click or by hovering.
    pub on: Trigger,
    /// The area is a piece of text (its colour shows whether the link was followed).
    pub text: bool,
    /// Screen tip (`tooltip`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Flash the shape when the action runs (`highlightClick`).
    pub highlight: bool,
    /// Sound played with the action (`a:snd`), or `stop` (`endSnd`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundOut>,
    /// What the host may do with it.
    pub safety: Safety,
    /// Identity of the target ([`bo_pptx::link_key`]) for the followed state.
    pub key: String,
    /// The target joins the visited links (`history`, default true).
    pub history: bool,
}

/// The sound of an action being set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SoundSpec {
    /// Leave the sound as it is.
    #[default]
    Keep,
    /// No sound.
    None,
    /// Stop the sound playing (`endSnd`).
    Stop,
    /// Embed a WAV file (§20.1.2.2.32 snd).
    File {
        /// Name shown for the sound.
        name: String,
        /// The file, base64.
        data: String,
    },
}

/// An action being set on the selection.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSpec {
    /// Where it leads (`none`: nowhere — only the sound / highlight, if any).
    pub target: LinkAction,
    /// Screen tip.
    #[serde(default)]
    pub tooltip: Option<String>,
    /// Flash the shape (`highlightClick`).
    #[serde(default)]
    pub highlight: bool,
    /// Sound.
    #[serde(default)]
    pub sound: SoundSpec,
}

/// What the action applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// The selected text when a range is selected, else the selected shapes.
    #[default]
    Auto,
    /// The selected shapes.
    Shape,
}

/// Commands of hyperlinks and actions (JSON `cmd` starting with `action`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum ActionCommand {
    /// The actions of the selection → `action`.
    ActionInfo,
    /// Set (or, with nothing to do, remove) the click or hover action.
    ActionSet {
        /// Click or hover.
        #[serde(default)]
        on: Trigger,
        /// The action.
        spec: ActionSpec,
        /// Text range or shapes.
        #[serde(default)]
        scope: Scope,
    },
    /// Insert an action button (§20.1.10.56 `actionButton*`) with its usual action.
    ActionButton {
        /// Preset geometry name.
        preset: String,
        /// Box in slide points; `None` = a default size centred on the slide.
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// A link was followed in the show: its text takes the followed colour.
    ActionFollowed {
        /// [`LinkOut::key`].
        key: String,
    },
    /// A new show starts: no link is followed yet.
    ActionResetFollowed,
}

impl ActionCommand {
    /// Parse a JSON command whose `cmd` starts with `action`.
    pub fn parse(v: &serde_json::Value) -> Option<Result<ActionCommand>> {
        let cmd = v.get("cmd")?.as_str()?;
        if !cmd.starts_with("action") {
            return None;
        }
        Some(serde_json::from_value(v.clone()).map_err(|e| EngineError::BadCommand(e.to_string())))
    }
}

/// One action of the selection as the dialogs show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOut {
    /// Where it leads.
    pub target: LinkAction,
    /// Screen tip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// `highlightClick`.
    pub highlight: bool,
    /// Sound (`a:snd` / `endSnd`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundOut>,
    /// What a show may do with it.
    pub safety: Safety,
    /// The shape's own action (a media clip, an embedded object): not editable here.
    pub locked: bool,
}

/// Answer of the `action*` commands.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionInfoOut {
    /// `text` (a text range), `shape` or `none` (nothing to link).
    pub scope: &'static str,
    /// Click action.
    pub click: Option<ActionOut>,
    /// Hover action.
    pub hover: Option<ActionOut>,
    /// The selected text, or the text of the selected shape.
    pub text: String,
    /// Title of every slide (the slide picker).
    pub slides: Vec<String>,
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Whether a host may open `url` (after asking): `http`, `https` and `mailto`
/// only, written plainly — no leading space, no control characters.
pub fn allowed_url(url: &str) -> bool {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => rest.len() > 2 && rest.starts_with("//") && !rest.starts_with("///"),
        "mailto" => !rest.is_empty(),
        _ => false,
    }
}

/// The policy of the slide show for an action (see the module documentation).
pub fn safety(a: &LinkAction) -> Safety {
    match a {
        LinkAction::None
        | LinkAction::CustomShow { .. }
        | LinkAction::Slide { .. }
        | LinkAction::Next
        | LinkAction::Prev
        | LinkAction::First
        | LinkAction::Last
        | LinkAction::LastViewed
        | LinkAction::End => Safety::Internal,
        LinkAction::Url { url } if allowed_url(url) => Safety::External,
        LinkAction::Url { .. }
        | LinkAction::Presentation { .. }
        | LinkAction::File { .. }
        | LinkAction::Program { .. }
        | LinkAction::Macro { .. }
        | LinkAction::Unknown { .. } => Safety::Blocked,
    }
}

fn rel_of<'a>(pres: &'a Presentation, slide: usize, h: &Hyperlink) -> Option<&'a Rel> {
    let id = h.rid.as_deref().filter(|i| !i.is_empty())?;
    pres.slides
        .get(slide)?
        .info
        .rels
        .iter()
        .find(|r| r.id == id)
}

/// What a hyperlink element of `slide` does. `None`: the shape's own action
/// (media, OLE verb), which is not a link.
pub fn resolve(pres: &Presentation, slide: usize, h: &Hyperlink) -> Option<LinkAction> {
    let rel = rel_of(pres, slide, h);
    let external = || {
        rel.filter(|r| r.external)
            .map(|r| r.target.clone())
            .unwrap_or_default()
    };
    Some(match h.kind() {
        ActionKind::Media | ActionKind::Ole => return None,
        ActionKind::Target => match rel {
            Some(r) if r.external => LinkAction::Url {
                url: r.target.clone(),
            },
            // An internal part without an action: nothing BlueSlide can follow.
            Some(_) => LinkAction::Unknown {
                action: String::new(),
            },
            None => LinkAction::None,
        },
        ActionKind::NoAction => LinkAction::None,
        ActionKind::SlideJump => {
            let s = pres.slides.get(slide)?;
            let to = rel.filter(|r| !r.external).and_then(|r| {
                let target = bo_opc::resolve_target(&s.info.part_name, &r.target);
                pres.slides
                    .iter()
                    .position(|x| x.info.part_name.eq_ignore_ascii_case(&target))
            });
            // A jump to a slide that is gone does nothing.
            to.map_or(LinkAction::None, |slide| LinkAction::Slide { slide })
        }
        ActionKind::ShowJump(j) => match j {
            ShowJump::Next => LinkAction::Next,
            ShowJump::Previous => LinkAction::Prev,
            ShowJump::First => LinkAction::First,
            ShowJump::Last => LinkAction::Last,
            ShowJump::LastViewed => LinkAction::LastViewed,
            ShowJump::EndShow => LinkAction::End,
        },
        ActionKind::CustomShow { id, ret } => LinkAction::CustomShow { id, ret },
        ActionKind::OtherPresentation { slide_index } => LinkAction::Presentation {
            path: external(),
            slide: slide_index,
        },
        ActionKind::File => LinkAction::File { path: external() },
        ActionKind::Program => LinkAction::Program { path: external() },
        ActionKind::Macro { name } => LinkAction::Macro { name },
        ActionKind::Unknown => LinkAction::Unknown {
            action: h.action.clone().unwrap_or_default(),
        },
    })
}

/// The sound of a hyperlink element: its embedded WAV, or "stop".
fn sound_out(pres: &Presentation, slide: usize, h: &Hyperlink) -> Option<SoundOut> {
    if let Some(snd) = &h.sound {
        let s = pres.slides.get(slide)?;
        return Some(SoundOut {
            stop: false,
            part: s
                .info
                .rels
                .iter()
                .find(|r| r.id == snd.embed && !r.external)
                .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target)),
            name: snd.name.clone(),
            looped: false,
        });
    }
    h.end_snd.unwrap_or(false).then(|| SoundOut {
        stop: true,
        part: None,
        name: String::new(),
        looped: false,
    })
}

fn action_out(pres: &Presentation, slide: usize, h: &Hyperlink) -> ActionOut {
    let resolved = resolve(pres, slide, h);
    let target = resolved.clone().unwrap_or(LinkAction::Unknown {
        action: h.action.clone().unwrap_or_default(),
    });
    ActionOut {
        safety: safety(&target),
        target,
        tooltip: h.tooltip.clone().filter(|t| !t.is_empty()),
        highlight: h.highlights(),
        sound: sound_out(pres, slide, h),
        locked: resolved.is_none(),
    }
}

/// Element names of the click and hover action of a shape / of a text run.
fn names(text: bool, on: Trigger) -> (&'static str, u16) {
    match (text, on) {
        (false, Trigger::Click) => ("a:hlinkClick", KEY_CLICK),
        (false, Trigger::Hover) => ("a:hlinkHover", KEY_HOVER),
        (true, Trigger::Click) => ("a:hlinkClick", bo_drawingml::text::rpr_key("a:hlinkClick")),
        (true, Trigger::Hover) => (
            "a:hlinkMouseOver",
            bo_drawingml::text::rpr_key("a:hlinkMouseOver"),
        ),
    }
}

fn link_in<'a>(extra: &'a [Extra], name: &str) -> Option<&'a Extra> {
    extra.iter().find(|x| x.xml.name == name)
}

fn hot(
    pres: &Presentation,
    slide: usize,
    shape_id: u32,
    rect: [f64; 4],
    on: Trigger,
    text: bool,
    x: &Extra,
) -> Option<LinkOut> {
    let h = hlink::parse_hyperlink(&x.xml);
    let action = resolve(pres, slide, &h)?;
    let sound = sound_out(pres, slide, &h);
    if action == LinkAction::None && sound.is_none() && !h.highlights() {
        return None;
    }
    let s = pres.slides.get(slide)?;
    Some(LinkOut {
        shape_id,
        rect,
        safety: safety(&action),
        action,
        on,
        text,
        tooltip: h.tooltip.clone().filter(|t| !t.is_empty()),
        highlight: h.highlights(),
        sound,
        key: bo_pptx::link_key(&s.info.part_name, &s.info.rels, &x.xml),
        history: h.in_history(),
    })
}

/// The hot areas of `slide` for the slide show, bottom to top: every shape with
/// a click or hover action (top-level and grouped), then its linked text.
pub fn links_out(pres: &Presentation, slide: usize, fonts: &mut FontManager) -> Vec<LinkOut> {
    let placed = place_slide(pres, slide);
    let mut flat: Vec<&Placed> = Vec::new();
    super::connect::walk(&placed, &mut |p| flat.push(p));
    let mut out = Vec::new();
    for p in flat {
        let Some(node) = node_at(pres, slide, &p.path) else {
            continue;
        };
        let Some(nv) = node.nv() else {
            continue;
        };
        let r = p.b.bounds();
        let rect = [r.x, r.y, r.w, r.h];
        for on in [Trigger::Click, Trigger::Hover] {
            let (name, _) = names(false, on);
            if let Some(x) = link_in(&nv.cnv_extra, name) {
                out.extend(hot(pres, slide, p.id, rect, on, false, x));
            }
        }
        let ShapeNode::Shape(s) = node else {
            continue;
        };
        let Some(tb) = &s.text else {
            continue;
        };
        let linked = |r: &TextRun| match r {
            TextRun::Run { props, .. } | TextRun::Field { props, .. } => {
                props.extra.iter().any(|x| hlink::is_hyperlink(&x.xml))
            }
            _ => false,
        };
        if !tb.paragraphs.iter().any(|p| p.runs.iter().any(linked)) {
            continue;
        }
        let b = rich::text_box(p, node);
        let Some(g) = rich::geometry(pres, slide, s, &b, fonts) else {
            continue;
        };
        for (para, (start, _)) in tb.paragraphs.iter().zip(text::para_ranges(tb)) {
            let mut at = start;
            for run in &para.runs {
                let len = text::run_len(run);
                let (from, to) = (at, at + len);
                at = to;
                let (TextRun::Run { props, .. } | TextRun::Field { props, .. }) = run else {
                    continue;
                };
                for on in [Trigger::Click, Trigger::Hover] {
                    let (name, _) = names(true, on);
                    let Some(x) = link_in(&props.extra, name) else {
                        continue;
                    };
                    let (_, boxes) = rich::selection_shapes(&g, &b, from, to);
                    for bx in boxes {
                        let r = bx.bounds();
                        out.extend(hot(pres, slide, p.id, [r.x, r.y, r.w, r.h], on, true, x));
                    }
                }
            }
        }
    }
    out
}

/// Whether any shape or text run of `slide` carries a hyperlink element.
fn has_links(pres: &Presentation, slide: usize) -> bool {
    let Some(s) = pres.slides.get(slide) else {
        return false;
    };
    let mut found = false;
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if found {
            return;
        }
        let on_shape = n
            .nv()
            .is_some_and(|nv| nv.cnv_extra.iter().any(|x| hlink::is_hyperlink(&x.xml)));
        let in_text = match n {
            ShapeNode::Shape(sh) => sh.text.as_ref().is_some_and(|tb| {
                tb.paragraphs.iter().any(|p| {
                    p.runs.iter().any(|r| match r {
                        TextRun::Run { props, .. } | TextRun::Field { props, .. } => {
                            props.extra.iter().any(|x| hlink::is_hyperlink(&x.xml))
                        }
                        _ => false,
                    })
                })
            }),
            _ => false,
        };
        found = on_shape || in_text;
    });
    found
}

/// Where a link of an exported page leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportTarget {
    /// A URL the reader may open (`http`, `https`, `mailto`).
    Url(String),
    /// A slide of the deck (0-based).
    Slide(usize),
}

/// Click links of `slide` an exported document can keep: allowed URLs and jumps
/// whose slide is known without running the show, with their boxes in points.
pub fn export_links(
    pres: &Presentation,
    slide: usize,
    fonts: &mut FontManager,
) -> Vec<([f64; 4], ExportTarget)> {
    let n = pres.slides.len();
    links_out(pres, slide, fonts)
        .into_iter()
        .filter(|l| l.on == Trigger::Click)
        .filter_map(|l| {
            let t = match l.action {
                LinkAction::Url { url } if allowed_url(&url) => ExportTarget::Url(url),
                LinkAction::Slide { slide } if slide < n => ExportTarget::Slide(slide),
                LinkAction::First if n > 0 => ExportTarget::Slide(0),
                LinkAction::Last if n > 0 => ExportTarget::Slide(n - 1),
                LinkAction::Next if slide + 1 < n => ExportTarget::Slide(slide + 1),
                LinkAction::Prev if slide > 0 => ExportTarget::Slide(slide - 1),
                _ => return None,
            };
            Some((l.rect, t))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn clean(s: &str) -> R<String> {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > MAX_TEXT || t.chars().any(char::is_control) {
        return refuse("badLink");
    }
    Ok(t.to_owned())
}

/// The relationship `(type, target, external)` of the slide, added when missing.
fn ensure_rel(
    tx: &mut Tx<'_>,
    slide: usize,
    kind: &str,
    target: &str,
    external: bool,
) -> R<String> {
    let rel_type = format!("{}{kind}", bo_pptx::RELATIONSHIP_NS);
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    if let Some(r) = s
        .info
        .rels
        .iter()
        .find(|r| r.rel_type == rel_type && r.target == target && r.external == external)
    {
        return Ok(r.id.clone());
    }
    let id = clip::free_rel_id(&s.info.rels);
    let rel = Rel {
        id: id.clone(),
        rel_type,
        target: target.to_owned(),
        external,
    };
    tx.fields(slide, |f| f.rels.push(rel))?;
    Ok(id)
}

/// `(r:id, action)` of an element leading to `target` on `slide`.
fn link_parts(tx: &mut Tx<'_>, slide: usize, target: &LinkAction) -> R<(String, Option<String>)> {
    let jump = |j: ShowJump| Ok((String::new(), ActionKind::ShowJump(j).uri()));
    match target {
        LinkAction::None => Ok((String::new(), ActionKind::NoAction.uri())),
        LinkAction::Next => jump(ShowJump::Next),
        LinkAction::Prev => jump(ShowJump::Previous),
        LinkAction::First => jump(ShowJump::First),
        LinkAction::Last => jump(ShowJump::Last),
        LinkAction::LastViewed => jump(ShowJump::LastViewed),
        LinkAction::End => jump(ShowJump::EndShow),
        LinkAction::CustomShow { id, ret } => {
            if !custom_shows(tx.pres).iter().any(|s| s.id == *id) {
                return refuse("noCustomShow");
            }
            Ok((
                String::new(),
                ActionKind::CustomShow { id: *id, ret: *ret }.uri(),
            ))
        }
        LinkAction::Slide { slide: to } => {
            let from = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
            let Some(dest) = tx.pres.slides.get(*to) else {
                return refuse("noSlide");
            };
            let rel = clip::relative_target(&from.info.part_name, &dest.info.part_name);
            // §21.1.2.3.5: the relationship names the slide, the action says "jump".
            let id = ensure_rel(tx, slide, "slide", &rel, false)?;
            Ok((id, ActionKind::SlideJump.uri()))
        }
        LinkAction::Url { url } => {
            let id = ensure_rel(tx, slide, "hyperlink", &clean(url)?, true)?;
            Ok((id, None))
        }
        LinkAction::File { path } => {
            let id = ensure_rel(tx, slide, "hyperlink", &clean(path)?, true)?;
            Ok((id, ActionKind::File.uri()))
        }
        LinkAction::Presentation { path, slide: index } => {
            let id = ensure_rel(tx, slide, "hyperlink", &clean(path)?, true)?;
            let kind = ActionKind::OtherPresentation {
                slide_index: *index,
            };
            Ok((id, kind.uri()))
        }
        // Programs and macros are never created; unknown actions cannot be written.
        LinkAction::Program { .. } | LinkAction::Macro { .. } | LinkAction::Unknown { .. } => {
            refuse("unsafeAction")
        }
    }
}

/// A WAV file (RIFF … WAVE), the only audio §20.1.2.2.32 snd embeds.
fn is_wav(b: &[u8]) -> bool {
    b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WAVE"
}

/// The audio relationship of `slide` to the WAV `bytes` (one part per sound).
fn ensure_sound(tx: &mut Tx<'_>, slide: usize, data: &str) -> R<String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| EditError::Refused("badSound"))?;
    if !is_wav(&bytes) || bytes.len() > MAX_SOUND {
        return refuse("badSound");
    }
    let known = tx
        .pres
        .parts
        .iter()
        .find(|p| p.name.to_ascii_lowercase().ends_with(".wav") && p.data == bytes)
        .map(|p| p.name.clone());
    let part = if let Some(p) = known {
        p
    } else {
        let part = build::unique_part(tx.pres, "/ppt/media/audio", "wav");
        let at = tx.pres.parts.len();
        tx.op(SlideOp::Parts {
            index: at,
            remove: vec![],
            insert: vec![RawPart {
                name: part.clone(),
                content_type: Some("audio/wav".into()),
                data: bytes,
            }],
        })?;
        part
    };
    let from = tx
        .pres
        .slides
        .get(slide)
        .ok_or(OpError::NoSlide(slide))?
        .info
        .part_name
        .clone();
    ensure_rel(
        tx,
        slide,
        "audio",
        &clip::relative_target(&from, &part),
        false,
    )
}

/// What a command writes, resolved once for the slide.
struct Built {
    rid: String,
    action: Option<String>,
    tooltip: Option<String>,
    highlight: bool,
    sound: BuiltSound,
    /// Nothing to keep: the element is removed.
    empty_target: bool,
}

enum BuiltSound {
    Keep,
    None,
    Stop,
    File(HlinkSound),
}

impl Built {
    /// The element replacing `old` (`None`: no element).
    fn apply(&self, old: Option<&Hyperlink>) -> Option<Hyperlink> {
        let mut h = old.cloned().unwrap_or_default();
        match &self.sound {
            BuiltSound::Keep => {}
            BuiltSound::None => {
                h.sound = None;
                h.end_snd = None;
            }
            BuiltSound::Stop => {
                h.sound = None;
                h.end_snd = Some(true);
            }
            BuiltSound::File(s) => {
                h.sound = Some(s.clone());
                h.end_snd = None;
            }
        }
        h.highlight_click = self.highlight.then_some(true);
        if self.empty_target && h.sound.is_none() && h.end_snd != Some(true) && !self.highlight {
            return None;
        }
        h.rid = Some(self.rid.clone());
        h.action.clone_from(&self.action);
        h.tooltip.clone_from(&self.tooltip);
        // The target changed: what was said about the old one no longer holds.
        h.attrs.retain(|(k, _)| k != "invalidUrl");
        Some(h)
    }
}

/// Put (or remove) hyperlink element `name` among kept children.
fn put(extra: &mut Vec<Extra>, name: &str, key: u16, h: Option<&Hyperlink>) {
    extra.retain(|x| x.xml.name != name);
    if let Some(h) = h {
        let at = extra
            .iter()
            .position(|x| x.key > key)
            .unwrap_or(extra.len());
        extra.insert(
            at,
            Extra {
                key,
                xml: hlink::write_hyperlink(name, h),
            },
        );
    }
}

fn nv_mut(n: &mut ShapeNode) -> Option<&mut bo_model::slide::NonVisual> {
    match n {
        ShapeNode::Shape(s) => Some(&mut s.nv),
        ShapeNode::Picture(p) => Some(&mut p.nv),
        ShapeNode::Group(g) => Some(&mut g.nv),
        ShapeNode::Connector(c) => Some(&mut c.nv),
        ShapeNode::Frame(f) => Some(&mut f.nv),
        ShapeNode::Other(_) => None,
    }
}

fn old_link(extra: &[Extra], name: &str) -> Option<Hyperlink> {
    link_in(extra, name).map(|x| hlink::parse_hyperlink(&x.xml))
}

/// The shape's own action (media, OLE verb): left alone.
fn own_action(h: Option<&Hyperlink>) -> bool {
    h.is_some_and(|h| matches!(h.kind(), ActionKind::Media | ActionKind::Ole))
}

/// Relationships of `slide` that only hyperlinks use and nothing refers to any more.
fn drop_unused_rels(tx: &mut Tx<'_>, slide: usize, candidates: &[String]) -> R<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let mut used: Vec<String> = Vec::new();
    for n in &s.data.shapes {
        used.extend(clip::rel_ids(n));
    }
    let link_type = |t: &str| t.ends_with("/hyperlink") || t.ends_with("/slide");
    let gone: Vec<String> = s
        .info
        .rels
        .iter()
        .filter(|r| candidates.contains(&r.id) && link_type(&r.rel_type) && !used.contains(&r.id))
        .map(|r| r.id.clone())
        .collect();
    if !gone.is_empty() {
        tx.fields(slide, |f| f.rels.retain(|r| !gone.contains(&r.id)))?;
    }
    Ok(())
}

/// Set the click or hover action of the selected shapes, or — with `text_sel` —
/// of a range of the text being edited. One undo step.
pub fn set_action(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    text_sel: Option<rich::TextSel>,
    on: Trigger,
    spec: &ActionSpec,
) -> R<()> {
    if sel.ids.is_empty() && text_sel.is_none() {
        return refuse("noSelection");
    }
    let slide = sel.slide;
    let tooltip = match spec.tooltip.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => Some(clean(t)?),
        _ => None,
    };
    let sound = match &spec.sound {
        SoundSpec::Keep => BuiltSound::Keep,
        SoundSpec::None => BuiltSound::None,
        SoundSpec::Stop => BuiltSound::Stop,
        SoundSpec::File { name, data } => {
            let embed = ensure_sound(tx, slide, data)?;
            let name = name.trim();
            BuiltSound::File(HlinkSound {
                embed,
                name: if name.is_empty() {
                    "sound.wav".into()
                } else {
                    clean(name)?
                },
                attrs: Vec::new(),
            })
        }
    };
    let (rid, action) = link_parts(tx, slide, &spec.target)?;
    let built = Built {
        rid,
        action,
        tooltip,
        highlight: spec.highlight,
        sound,
        empty_target: spec.target == LinkAction::None,
    };
    let placed = place_slide(tx.pres, slide);
    let mut old_rids: Vec<String> = Vec::new();
    let mut note = |h: Option<&Hyperlink>| {
        if let Some(id) = h.and_then(|h| h.rid.clone()).filter(|i| !i.is_empty()) {
            old_rids.push(id);
        }
    };
    if let Some(ts) = text_sel {
        // A range of the text of one shape (§21.1.2.3.9 rPr).
        if ts.cell.is_some() {
            return refuse("unsupportedLink");
        }
        let Some(p) = find(&placed, ts.id) else {
            return refuse("noSelection");
        };
        let Some(ShapeNode::Shape(s)) = node_at(tx.pres, slide, &p.path) else {
            return refuse("noSelection");
        };
        let mut s = s.clone();
        let Some(tb) = s.text.as_mut() else {
            return refuse("noSelection");
        };
        let (name, key) = names(true, on);
        text::map_runs_range(tb, ts.start, ts.end, &mut |rp: &mut RunProps| {
            let old = old_link(&rp.extra, name);
            note(old.as_ref());
            put(&mut rp.extra, name, key, built.apply(old.as_ref()).as_ref());
        });
        tx.replace_node(slide, &p.path, ShapeNode::Shape(s))?;
    } else {
        let (name, key) = names(false, on);
        for &id in &sel.ids {
            let Some(p) = find(&placed, id) else {
                continue;
            };
            let Some(mut node) = node_at(tx.pres, slide, &p.path).cloned() else {
                continue;
            };
            let Some(nv) = nv_mut(&mut node) else {
                continue;
            };
            let old = old_link(&nv.cnv_extra, name);
            if own_action(old.as_ref()) {
                continue;
            }
            note(old.as_ref());
            put(
                &mut nv.cnv_extra,
                name,
                key,
                built.apply(old.as_ref()).as_ref(),
            );
            tx.replace_node(slide, &p.path, node)?;
        }
    }
    // The relationship made above may have been left unused (nothing was changed).
    if !built.rid.is_empty() {
        old_rids.push(built.rid.clone());
    }
    drop_unused_rels(tx, slide, &old_rids)
}

/// Before slide `index` is deleted: remove the links of the other slides to it
/// (their `slide` relationship and the elements using it).
pub fn drop_links_to(tx: &mut Tx<'_>, index: usize) -> R<()> {
    let Some(target) = tx.pres.slides.get(index).map(|s| s.info.part_name.clone()) else {
        return Ok(());
    };
    for i in 0..tx.pres.slides.len() {
        if i == index {
            continue;
        }
        let s = &tx.pres.slides[i];
        let gone: Vec<String> = s
            .info
            .rels
            .iter()
            .filter(|r| {
                !r.external
                    && r.rel_type.ends_with("/slide")
                    && bo_opc::resolve_target(&s.info.part_name, &r.target)
                        .eq_ignore_ascii_case(&target)
            })
            .map(|r| r.id.clone())
            .collect();
        if gone.is_empty() {
            continue;
        }
        let nodes: Vec<(usize, ShapeNode)> = s
            .data
            .shapes
            .iter()
            .enumerate()
            .map(|(k, n)| (k, clip::strip_rel_refs(n, &gone)))
            .collect();
        for (k, n) in nodes {
            tx.replace_node(i, &[k], n)?;
        }
        tx.fields(i, |f| f.rels.retain(|r| !gone.contains(&r.id)))?;
    }
    Ok(())
}

/// The action an action button starts with (§20.1.10.56 `actionButton*`).
pub fn button_action(preset: &str) -> Option<LinkAction> {
    Some(match preset {
        "actionButtonBackPrevious" => LinkAction::Prev,
        "actionButtonForwardNext" => LinkAction::Next,
        "actionButtonBeginning" | "actionButtonHome" => LinkAction::First,
        "actionButtonEnd" => LinkAction::Last,
        "actionButtonReturn" => LinkAction::LastViewed,
        "actionButtonBlank"
        | "actionButtonDocument"
        | "actionButtonHelp"
        | "actionButtonInformation"
        | "actionButtonMovie"
        | "actionButtonSound" => LinkAction::None,
        _ => return None,
    })
}

fn cmd_action_button(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    preset: &str,
    rect: Option<Rect>,
) -> R<()> {
    let Some(target) = button_action(preset) else {
        return refuse("unknownPreset");
    };
    if bo_drawingml::preset(preset).is_none() {
        return refuse("unknownPreset");
    }
    let r = super::place_new(tx.pres, rect, 72.0, 72.0);
    let id = super::next_shape_id(tx.pres, sel.slide);
    let n = build::preset_shape(id, preset, r.x, r.y, r.w, r.h);
    super::insert_top(tx, sel, n)?;
    if target == LinkAction::None {
        return Ok(());
    }
    let spec = ActionSpec {
        target,
        tooltip: None,
        // Buttons look pressed when clicked.
        highlight: true,
        sound: SoundSpec::None,
    };
    set_action(tx, sel, None, Trigger::Click, &spec)
}

/// The text range a command applies to, when a range of a shape's text is selected.
fn text_scope(ts: Option<rich::TextSel>) -> Option<rich::TextSel> {
    ts.filter(|t| t.start != t.end && t.cell.is_none())
}

fn info(pres: &Presentation, sel: &SlideSel, ts: Option<rich::TextSel>) -> ActionInfoOut {
    let slide = sel.slide;
    let slides = pres
        .slides
        .iter()
        .map(|s| super::outline::slide_outline(s).title.trim().to_owned())
        .collect();
    let placed = place_slide(pres, slide);
    let node_of = |id: u32| find(&placed, id).and_then(|p| node_at(pres, slide, &p.path));
    let read = |extra: &[Extra], text: bool, on: Trigger| {
        old_link(extra, names(text, on).0).map(|h| action_out(pres, slide, &h))
    };
    let none = ActionInfoOut {
        scope: "none",
        click: None,
        hover: None,
        text: String::new(),
        slides,
    };
    if let Some(ts) = text_scope(ts) {
        let Some(ShapeNode::Shape(s)) = node_of(ts.id) else {
            return none;
        };
        let Some(tb) = &s.text else {
            return none;
        };
        let (from, to) = (ts.start.min(ts.end), ts.start.max(ts.end));
        let (_, props) = text::props_at_offset(tb, from, true);
        let body = text::body_text(tb);
        return ActionInfoOut {
            scope: "text",
            click: read(&props.extra, true, Trigger::Click),
            hover: read(&props.extra, true, Trigger::Hover),
            text: body.chars().skip(from).take(to - from).collect(),
            ..none
        };
    }
    let Some(node) = sel.ids.first().and_then(|&id| node_of(id)) else {
        return none;
    };
    let Some(nv) = node.nv() else {
        return none;
    };
    ActionInfoOut {
        scope: "shape",
        click: read(&nv.cnv_extra, false, Trigger::Click),
        hover: read(&nv.cnv_extra, false, Trigger::Hover),
        text: match node {
            ShapeNode::Shape(s) => s.text.as_ref().map(text::body_text).unwrap_or_default(),
            _ => String::new(),
        },
        ..none
    }
}

impl Engine {
    /// Runs a hyperlink / action command.
    pub fn slide_action_command(&mut self, cmd: ActionCommand) -> Result<SlideEditResult> {
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            let editing = matches!(
                cmd,
                ActionCommand::ActionSet { .. } | ActionCommand::ActionButton { .. }
            );
            if editing && ed.in_any_master_view() {
                out.error = Some("masterView".into());
            } else {
                match cmd {
                    ActionCommand::ActionInfo => {}
                    ActionCommand::ActionSet { on, spec, scope } => {
                        let ts = match scope {
                            Scope::Auto => text_scope(ed.text),
                            Scope::Shape => None,
                        };
                        ed.mutate(pres, &mut out, |tx, sel| set_action(tx, sel, ts, on, &spec));
                    }
                    ActionCommand::ActionButton { preset, rect } => {
                        ed.text = None;
                        ed.table = None;
                        ed.mutate(pres, &mut out, |tx, sel| {
                            cmd_action_button(tx, sel, &preset, rect)
                        });
                    }
                    ActionCommand::ActionFollowed { key } => {
                        if !key.is_empty() && !ed.followed.contains(&key) {
                            ed.followed.push(key);
                        }
                    }
                    ActionCommand::ActionResetFollowed => ed.followed.clear(),
                }
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        if let Some(pres) = self.pres.as_ref() {
            r.action = Some(info(pres, &self.slides.sel, self.slides.text));
        }
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// Recompute the links an exported PDF keeps, for one slide or (`None`) all.
    /// Slides without any hyperlink cost a walk over their shapes.
    pub(crate) fn refresh_slide_links(&mut self, only: Option<usize>) {
        let Some(pres) = self.pres.as_ref() else {
            self.slide_links.clear();
            return;
        };
        let n = pres.slides.len();
        self.slide_links.resize_with(n, Vec::new);
        let range = match only {
            Some(i) if i < n => i..i + 1,
            Some(_) => 0..0,
            None => 0..n,
        };
        for i in range {
            self.slide_links[i] = if has_links(pres, i) {
                export_links(pres, i, &mut self.fonts)
            } else {
                Vec::new()
            };
        }
    }

    /// Click links of a slide that an exported document keeps (PDF link annotations).
    pub fn slide_export_links(&self, slide: usize) -> &[([f64; 4], ExportTarget)] {
        self.slide_links.get(slide).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_urls_may_be_opened() {
        for ok in [
            "http://example.vn",
            "https://example.vn/a?b=c#d",
            "HTTPS://EXAMPLE.VN",
            "mailto:a@example.vn?subject=Hi",
        ] {
            assert!(allowed_url(ok), "{ok}");
        }
        for bad in [
            "",
            "example.vn",
            "file:///C:/Windows/System32/calc.exe",
            "FILE://server/share/run.bat",
            "C:\\Windows\\System32\\calc.exe",
            "\\\\server\\share\\run.bat",
            "javascript:alert(1)",
            "data:text/html,<script>1</script>",
            "vbscript:msgbox(1)",
            "ms-msdt:/id PCWDiagnostic",
            "search-ms:query=x",
            "ftp://example.vn/x",
            "bluedoc://open?file=x",
            " https://example.vn",
            "https://exa mple.vn",
            "java\tscript:alert(1)",
            "https:example.vn",
            "https:///etc/passwd",
            "http://",
            "mailto:",
        ] {
            assert!(!allowed_url(bad), "{bad}");
        }
    }

    #[test]
    fn programs_macros_files_and_unknown_actions_are_blocked() {
        let blocked = [
            LinkAction::Program {
                path: "file:///C:/calc.exe".into(),
            },
            LinkAction::Macro { name: "Run".into() },
            LinkAction::File {
                path: "report.docx".into(),
            },
            LinkAction::Presentation {
                path: "other.pptx".into(),
                slide: Some(2),
            },
            LinkAction::Unknown {
                action: "ppaction://frobnicate".into(),
            },
            LinkAction::Url {
                url: "file:///C:/calc.exe".into(),
            },
            LinkAction::Url {
                url: "javascript:alert(1)".into(),
            },
        ];
        for a in blocked {
            assert_eq!(safety(&a), Safety::Blocked, "{a:?}");
        }
        assert_eq!(
            safety(&LinkAction::Url {
                url: "https://example.vn".into()
            }),
            Safety::External
        );
        for a in [
            LinkAction::Next,
            LinkAction::LastViewed,
            LinkAction::Slide { slide: 2 },
            LinkAction::CustomShow { id: 0, ret: true },
            LinkAction::End,
        ] {
            assert_eq!(safety(&a), Safety::Internal, "{a:?}");
        }
    }

    #[test]
    fn action_buttons_start_with_their_jump() {
        assert_eq!(
            button_action("actionButtonForwardNext"),
            Some(LinkAction::Next)
        );
        assert_eq!(button_action("actionButtonEnd"), Some(LinkAction::Last));
        assert_eq!(
            button_action("actionButtonReturn"),
            Some(LinkAction::LastViewed)
        );
        assert_eq!(button_action("actionButtonBlank"), Some(LinkAction::None));
        assert_eq!(button_action("rect"), None);
    }
}
