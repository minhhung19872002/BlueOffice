//! Slide transitions (ECMA-376 Part 1, §19.3.1.50 transition) and the main
//! animation sequence (§19.5 Animation, §19.3.1.48 timing).
//!
//! Both are read into the model and written back verbatim while unchanged;
//! edited (or new) ones are regenerated in the structure §19.5 describes
//! (tmRoot → mainSeq → click group → start group → effect node).

use bo_drawingml::dom::{attr_int, parse_bool, El};
use bo_model::slide::{
    AnimClass, AnimEffect, AnimTrigger, Animation, Extra, Timing, Transition, TransitionKind,
    TransitionSound, XmlElement, XmlNode,
};

const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const NS_P15: &str = "http://schemas.microsoft.com/office/powerpoint/2012/main";
const NS_P159: &str = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";

/// Position key of `p:transition` / `p:timing` in CT_Slide (§19.3.1.38).
pub(crate) const KEY_TRANSITION: u16 = 6;
pub(crate) const KEY_TIMING: u16 = 8;

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

/// The `p:transition` element to model: itself, or inside `mc:AlternateContent`
/// (Choice first, it carries the p14 duration; then Fallback).
fn transition_el(el: &XmlElement) -> Option<&XmlElement> {
    match el.local() {
        "transition" => Some(el),
        "AlternateContent" => el
            .elements()
            .filter(|c| matches!(c.local(), "Choice" | "Fallback"))
            .find_map(|c| c.elements().find(|t| t.local() == "transition")),
        _ => None,
    }
}

/// Is this a transition (possibly wrapped in AlternateContent)?
pub(crate) fn is_transition(el: &XmlElement) -> bool {
    transition_el(el).is_some()
}

/// Parse §19.3.1.50 transition (or its AlternateContent wrapper).
pub fn parse_transition(el: &XmlElement, key: u16) -> Option<Transition> {
    let t = transition_el(el)?;
    let mut out = Transition {
        speed: t.attr("spd").map(str::to_owned),
        dur_ms: t
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(":dur"))
            .and_then(|(_, v)| v.parse().ok()),
        advance_on_click: t.attr("advClick").and_then(parse_bool),
        advance_after_ms: attr_int(t, "advTm"),
        ..Transition::default()
    };
    if let Some(fx) = t
        .elements()
        .find(|c| !matches!(c.local(), "sndAc" | "extLst"))
    {
        out.kind = Some(match fx.name.as_str() {
            "p:cut" => TransitionKind::Cut,
            "p:fade" => TransitionKind::Fade,
            "p:push" => TransitionKind::Push,
            "p:wipe" => TransitionKind::Wipe,
            "p:cover" => TransitionKind::Cover,
            "p:pull" => TransitionKind::Pull,
            "p:split" => TransitionKind::Split,
            "p:random" => TransitionKind::Random,
            other => TransitionKind::Other(other.trim_start_matches("p:").to_owned()),
        });
        out.attrs = fx
            .attrs
            .iter()
            .filter(|(k, _)| !k.starts_with("xmlns"))
            .cloned()
            .collect();
        // A catalog effect reads as its variant (`horzIn`, `black`, `8`, …), others as their `dir`.
        let variant = crate::transitions::recognise(&fx.name, &out.attrs)
            .and_then(|d| crate::transitions::variant_of(d, &out.attrs));
        out.dir = variant.map(str::to_owned).or_else(|| {
            fx.attr("dir")
                .or_else(|| fx.attr("orient"))
                .map(str::to_owned)
        });
    }
    // §19.3.1.42 sndAc: §19.3.1.45 stSnd (§19.3.1.44 snd) or §19.3.1.19 endSnd.
    if let Some(snd) = t.elements().find(|c| c.local() == "sndAc") {
        if snd.elements().any(|c| c.local() == "endSnd") {
            out.sound = Some(TransitionSound::Stop);
        } else if let Some(st) = snd.elements().find(|c| c.local() == "stSnd") {
            if let Some(w) = st.elements().find(|c| c.local() == "snd") {
                out.sound = Some(TransitionSound::Start {
                    embed: w.attr("r:embed").unwrap_or_default().to_owned(),
                    name: w.attr("name").unwrap_or_default().to_owned(),
                    looped: st.attr("loop").and_then(parse_bool).unwrap_or(false),
                });
            }
        }
    }
    out.source = Some(Extra {
        key,
        xml: el.clone(),
    });
    Some(out)
}

fn same_transition(a: &Transition, b: &Transition) -> bool {
    a.kind == b.kind
        && a.dir == b.dir
        && a.speed == b.speed
        && a.dur_ms == b.dur_ms
        && a.advance_on_click == b.advance_on_click
        && a.advance_after_ms == b.advance_after_ms
        && a.attrs == b.attrs
        && a.sound == b.sound
}

/// The namespace an effect element needs (`p14`, `p15`), if not PresentationML's own.
fn effect_ns(t: &Transition) -> Option<&'static str> {
    match &t.kind {
        Some(TransitionKind::Other(n)) if n.starts_with("p159:") => Some("p159"),
        Some(TransitionKind::Other(n)) if n.starts_with("p15:") => Some("p15"),
        Some(TransitionKind::Other(n)) if n.starts_with("p14:") => Some("p14"),
        _ => None,
    }
}

/// `p:transition` with its effect (`fallback`: a fade instead of an effect of a newer namespace).
fn transition_body(t: &Transition, with_dur: bool, fallback: bool) -> XmlElement {
    if fallback {
        let plain = Transition {
            kind: Some(TransitionKind::Fade),
            dir: None,
            attrs: Vec::new(),
            ..t.clone()
        };
        return transition_body(&plain, false, false);
    }
    let mut e = El::new("p:transition")
        .opt("spd", t.speed.as_ref())
        .optb("advClick", t.advance_on_click)
        .opt("advTm", t.advance_after_ms);
    if with_dur {
        e = e.opt("p14:dur", t.dur_ms);
    }
    if let Some(k) = &t.kind {
        let name = match k {
            TransitionKind::Cut => "p:cut".to_owned(),
            TransitionKind::Fade => "p:fade".to_owned(),
            TransitionKind::Push => "p:push".to_owned(),
            TransitionKind::Wipe => "p:wipe".to_owned(),
            TransitionKind::Cover => "p:cover".to_owned(),
            TransitionKind::Pull => "p:pull".to_owned(),
            TransitionKind::Split => "p:split".to_owned(),
            TransitionKind::Random => "p:random".to_owned(),
            TransitionKind::Other(n) if n.contains(':') => n.clone(),
            TransitionKind::Other(n) => format!("p:{n}"),
        };
        let attr = if matches!(k, TransitionKind::Split) {
            "orient"
        } else {
            "dir"
        };
        // The effect's attributes when known (catalog effects, effects as read); else an unmodelled
        // effect keeps its element as read, a modelled one its direction.
        let catalog = crate::transitions::recognise(&name, &[]);
        let fx = if t.attrs.is_empty() {
            let kept = matches!(k, TransitionKind::Other(_))
                .then(|| source_effect(t, &name))
                .flatten();
            kept.unwrap_or_else(|| match catalog {
                // A catalog effect writes the attributes of its variant (`dir` holds the variant).
                Some(d) => {
                    let mut el = El::new(&name);
                    for (a, v) in crate::transitions::attrs_for(d, t.dir.as_deref()) {
                        el = el.a(&a, v);
                    }
                    el.done()
                }
                None => El::new(&name).opt(attr, t.dir.as_ref()).done(),
            })
        } else {
            let mut el = El::new(&name);
            for (a, v) in &t.attrs {
                el = el.a(a, v);
            }
            el.done()
        };
        e = e.child(fx);
    }
    // §19.3.1.42 sndAc (after the effect, CT_SlideTransition).
    match &t.sound {
        Some(TransitionSound::Start {
            embed,
            name,
            looped,
        }) => {
            let snd = El::new("p:snd").a("r:embed", embed).a("name", name).done();
            let st = El::new("p:stSnd")
                .optb("loop", looped.then_some(true))
                .child(snd)
                .done();
            e = e.child(El::new("p:sndAc").child(st).done());
        }
        Some(TransitionSound::Stop) => {
            e = e.child(El::new("p:sndAc").child(El::new("p:endSnd").done()).done());
        }
        None => {}
    }
    e.done()
}

/// The effect child of the transition as read, when it is element `name`.
fn source_effect(t: &Transition, name: &str) -> Option<XmlElement> {
    let src = transition_el(&t.source.as_ref()?.xml)?;
    src.elements()
        .find(|c| !matches!(c.local(), "sndAc" | "extLst"))
        .filter(|c| c.name == name)
        .cloned()
}

/// Serialize a transition: the source when unchanged, otherwise a new element
/// (wrapped in `mc:AlternateContent` with a p14 Choice when it has a duration).
pub fn write_transition(t: &Transition) -> Extra {
    if let Some(src) = &t.source {
        if parse_transition(&src.xml, src.key).is_some_and(|p| same_transition(&p, t)) {
            return src.clone();
        }
    }
    // An effect of [MS-PPTX] (p14 / p15) or a p14:dur needs mc:AlternateContent; the fallback of a
    // newer effect is a fade.
    let ns = effect_ns(t);
    let xml = if t.dur_ms.is_some() || ns.is_some() {
        let mut choice = El::new("mc:Choice").a("xmlns:p14", NS_P14);
        if ns == Some("p15") {
            choice = choice.a("xmlns:p15", NS_P15);
        }
        if ns == Some("p159") {
            choice = choice.a("xmlns:p159", NS_P159);
        }
        El::new("mc:AlternateContent")
            .a("xmlns:mc", NS_MC)
            .child(
                choice
                    .a("Requires", ns.unwrap_or("p14"))
                    .child(transition_body(t, true, false))
                    .done(),
            )
            .child(
                El::new("mc:Fallback")
                    .child(transition_body(t, false, ns.is_some()))
                    .done(),
            )
            .done()
    } else {
        transition_body(t, false, false)
    };
    let key = if xml.name == "p:transition" {
        KEY_TRANSITION
    } else {
        KEY_TRANSITION - 1
    };
    Extra { key, xml }
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

fn descendants<'a>(el: &'a XmlElement, out: &mut Vec<&'a XmlElement>) {
    for c in el.elements() {
        out.push(c);
        descendants(c, out);
    }
}

/// The mainSeq time node (§19.5.29 cTn nodeType="mainSeq").
fn main_seq(timing: &XmlElement) -> Option<&XmlElement> {
    let mut all = Vec::new();
    descendants(timing, &mut all);
    all.into_iter()
        .find(|e| e.name == "p:cTn" && e.attr("nodeType") == Some("mainSeq"))
}

fn delay_of(ctn: &XmlElement) -> u32 {
    ctn.child("p:stCondLst")
        .and_then(|l| l.child("p:cond"))
        .and_then(|c| attr_int::<u32>(c, "delay"))
        .unwrap_or(0)
}

/// Parse one effect node (cTn with presetClass / an effect nodeType).
fn parse_effect(ctn: &XmlElement) -> Option<Animation> {
    let trigger = match ctn.attr("nodeType")? {
        "clickEffect" => AnimTrigger::OnClick,
        "withEffect" => AnimTrigger::WithPrevious,
        "afterEffect" => AnimTrigger::AfterPrevious,
        _ => return None,
    };
    let mut all = Vec::new();
    descendants(ctn, &mut all);
    let shape_id = all
        .iter()
        .find(|e| e.name == "p:spTgt")
        .and_then(|e| attr_int(e, "spid"))?;
    let class = match ctn.attr("presetClass") {
        Some("entr") | None => AnimClass::Entrance,
        Some("exit") => AnimClass::Exit,
        Some("emph") => AnimClass::Emphasis,
        Some("path") => AnimClass::Path,
        Some(o) => AnimClass::Other(o.to_owned()),
    };
    // Duration: latest end of the behaviour time nodes.
    let dur_ms = all
        .iter()
        .filter(|e| e.name == "p:cTn")
        .filter_map(|e| {
            let d = e.attr("dur")?.parse::<u32>().ok()?;
            Some(delay_of(e) + d)
        })
        .max()
        .unwrap_or(0);
    let filter = all
        .iter()
        .find(|e| e.name == "p:animEffect")
        .and_then(|e| e.attr("filter"));
    let anim_attr = all.iter().filter(|e| e.name == "p:anim").find_map(|a| {
        let mut v = Vec::new();
        descendants(a, &mut v);
        let attr = v.iter().find(|e| e.name == "p:attrName")?.text();
        // The off-slide end: the first value of an entrance, the last of an exit.
        let vals: Vec<&str> = v
            .iter()
            .filter(|e| e.name == "p:strVal")
            .filter_map(|e| e.attr("val"))
            .collect();
        let far = if class == AnimClass::Exit {
            vals.last()
        } else {
            vals.first()
        };
        Some((attr, far.copied().unwrap_or_default().to_owned()))
    });
    let (effect, direction) = if let Some(f) = filter {
        if f.starts_with("fade") {
            (AnimEffect::Fade, None)
        } else if let Some(d) = f.strip_prefix("wipe(") {
            // wipe(down) reveals from the top, etc.
            let dir = match d.trim_end_matches(')') {
                "down" => "t",
                "up" => "b",
                "right" => "l",
                _ => "r",
            };
            (AnimEffect::Wipe, Some(dir.to_owned()))
        } else {
            (AnimEffect::Other, None)
        }
    } else if let Some((attr, first)) = anim_attr {
        let from_far = first.starts_with("1+");
        let dir = match (attr.as_str(), from_far) {
            ("ppt_x", true) => "r",
            ("ppt_x", false) => "l",
            ("ppt_y", true) => "b",
            _ => "t",
        };
        (AnimEffect::FlyIn, Some(dir.to_owned()))
    } else if all.iter().any(|e| e.name == "p:set") {
        (AnimEffect::Appear, None)
    } else {
        (AnimEffect::Other, None)
    };
    // Effects of the catalog by presetClass + presetID (§19.5.29), beyond the four modelled ones.
    // A catalog preset (§19.5.29 presetClass + presetID) names the effect, unless it is one of the
    // four modelled ones read from the behaviours.
    let unmodelled =
        effect == AnimEffect::Other || !matches!(class, AnimClass::Entrance | AnimClass::Exit);
    let named = attr_int::<u32>(ctn, "presetID")
        .and_then(|id| {
            crate::anim_catalog::recognise(ctn.attr("presetClass").unwrap_or("entr"), id)
        })
        .filter(|d| unmodelled || !matches!(d.id, "appear" | "flyIn" | "fade" | "wipe"));
    let (effect, direction) = match named {
        Some(d) => (
            AnimEffect::Named(d.id.to_owned()),
            crate::anim_catalog::dir_of(d, attr_int(ctn, "presetSubtype")).map(str::to_owned),
        ),
        None => (effect, direction),
    };
    Some(Animation {
        shape_id,
        class,
        effect,
        direction,
        trigger,
        delay_ms: delay_of(ctn),
        dur_ms,
        preset_id: attr_int(ctn, "presetID"),
        // §19.5.29 repeatCount: 1000ths, or `indefinite` (until the next click).
        repeat: match ctn.attr("repeatCount") {
            Some("indefinite") => Some(u32::MAX),
            Some(v) => v.parse::<u32>().ok().filter(|r| *r != 1000),
            None => None,
        },
        trigger_shape: None,
        preset_subtype: attr_int(ctn, "presetSubtype"),
        source: None,
    })
}

/// Effects whose behaviours are not modelled: their `p:par` is kept as read.
fn keeps_source(a: &Animation) -> bool {
    matches!(a.effect, AnimEffect::Other | AnimEffect::Named(_))
        || !matches!(a.class, AnimClass::Entrance | AnimClass::Exit)
}

/// The effect's `p:par` (§19.5.53) as it is written: the source of an effect kept as read,
/// otherwise generated from the model.
#[must_use]
pub fn effect_xml(a: &Animation) -> XmlElement {
    effect_node(&mut Ids(0), a)
}

/// The interactive sequences (§19.5.65 seq, cTn nodeType `interactiveSeq`) with their trigger
/// shape (the §19.5.25 cond `onClick` target).
fn interactive_seqs(timing: &XmlElement) -> Vec<(u32, &XmlElement)> {
    let mut all = Vec::new();
    descendants(timing, &mut all);
    all.into_iter()
        .filter(|e| e.name == "p:cTn" && e.attr("nodeType") == Some("interactiveSeq"))
        .filter_map(|c| {
            let mut d = Vec::new();
            descendants(c.child("p:stCondLst")?, &mut d);
            let spid = d
                .iter()
                .find(|e| e.name == "p:spTgt")
                .and_then(|e| attr_int(e, "spid"))?;
            Some((spid, c))
        })
        .collect()
}

/// Parse §19.3.1.48 timing: the effects of the main sequence in order, then those of the
/// interactive sequences.
pub fn parse_timing(el: &XmlElement, key: u16) -> Timing {
    let mut animations = Vec::new();
    let mut seqs: Vec<(Option<u32>, &XmlElement)> =
        main_seq(el).map(|s| (None, s)).into_iter().collect();
    seqs.extend(interactive_seqs(el).into_iter().map(|(t, s)| (Some(t), s)));
    for (trigger, seq) in seqs {
        let mut all = Vec::new();
        descendants(seq, &mut all);
        // §19.5.53 par → §19.5.29 cTn of each effect, in document order.
        for par in all.into_iter().filter(|e| e.name == "p:par") {
            let Some(ctn) = par.child("p:cTn") else {
                continue;
            };
            if let Some(mut a) = parse_effect(ctn) {
                a.trigger_shape = trigger;
                if keeps_source(&a) {
                    a.source = Some(par.clone());
                }
                // A motion path of the catalog, recognised by its path.
                if a.class == AnimClass::Path && a.effect == AnimEffect::Other {
                    if let Some(d) = path_by_play(&a, par) {
                        a.effect = AnimEffect::Named(d.to_owned());
                    }
                }
                if let AnimEffect::Named(n) = &a.effect {
                    if let Some(v) =
                        crate::anim_catalog::by_id(n).and_then(|d| variant_by_play(d, &a, par))
                    {
                        a.direction = Some(v.to_owned());
                    }
                }
                animations.push(a);
            }
        }
    }
    Timing {
        animations,
        source: Some(Extra {
            key,
            xml: el.clone(),
        }),
    }
}

/// The variant of catalog effect `d` whose behaviours do what `par` does (for variants without a
/// presetSubtype of their own, e.g. a spin's direction).
fn variant_by_play(
    d: &crate::anim_catalog::EffectDef,
    a: &Animation,
    par: &XmlElement,
) -> Option<&'static str> {
    use crate::anim_play::{effect_play, ShapeBase};
    if d.dirs.iter().all(|(_, s)| s.is_some()) {
        return None;
    }
    let base = ShapeBase {
        x: 0.4,
        y: 0.45,
        w: 0.2,
        h: 0.1,
    };
    // What the behaviours do, without their timing.
    let sig = |p: &crate::anim_play::EffectPlay| -> Vec<String> {
        p.tracks.iter().map(|t| format!("{:?}", t.kind)).collect()
    };
    let have = sig(&effect_play(par, &base));
    d.dirs.iter().map(|(v, _)| *v).find(|v| {
        let probe = Animation {
            direction: Some((*v).to_owned()),
            source: None,
            preset_id: None,
            preset_subtype: None,
            ..a.clone()
        };
        sig(&effect_play(&effect_node(&mut Ids(0), &probe), &base)) == have
    })
}

/// The catalog motion path that `par` plays.
fn path_by_play(a: &Animation, par: &XmlElement) -> Option<&'static str> {
    use crate::anim_play::{effect_play, ShapeBase, TrackKind};
    let base = ShapeBase {
        x: 0.5,
        y: 0.5,
        w: 0.1,
        h: 0.1,
    };
    let points = |x: &XmlElement| {
        effect_play(x, &base)
            .tracks
            .into_iter()
            .find_map(|t| match t.kind {
                TrackKind::Motion { points } => Some(points),
                _ => None,
            })
    };
    let have = points(par)?;
    crate::anim_catalog::EFFECTS
        .iter()
        .filter(|d| d.class == crate::anim_catalog::Class::Path)
        .find(|d| {
            let probe = Animation {
                effect: AnimEffect::Named(d.id.to_owned()),
                source: None,
                preset_id: None,
                preset_subtype: None,
                ..a.clone()
            };
            points(&effect_node(&mut Ids(0), &probe)).as_ref() == Some(&have)
        })
        .map(|d| d.id)
}

/// Allocates `cTn@id` values.
pub(crate) struct Ids(pub(crate) u32);

impl Ids {
    pub(crate) fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

pub(crate) fn cond(delay: &str) -> XmlElement {
    El::new("p:stCondLst")
        .child(El::new("p:cond").a("delay", delay).done())
        .done()
}

pub(crate) fn tgt(spid: u32) -> XmlElement {
    El::new("p:tgtEl")
        .child(El::new("p:spTgt").a("spid", spid).done())
        .done()
}

pub(crate) fn attr_names(name: &str) -> XmlElement {
    El::new("p:attrNameLst")
        .child(El::new("p:attrName").text(name).done())
        .done()
}

/// §19.5.66 set of `style.visibility`.
pub(crate) fn set_visibility(ids: &mut Ids, spid: u32, delay: u32, visible: bool) -> XmlElement {
    El::new("p:set")
        .child(
            El::new("p:cBhvr")
                .child(
                    El::new("p:cTn")
                        .a("id", ids.next())
                        .a("dur", 1)
                        .a("fill", "hold")
                        .child(cond(&delay.to_string()))
                        .done(),
                )
                .child(tgt(spid))
                .child(attr_names("style.visibility"))
                .done(),
        )
        .child(
            El::new("p:to")
                .child(
                    El::new("p:strVal")
                        .a("val", if visible { "visible" } else { "hidden" })
                        .done(),
                )
                .done(),
        )
        .done()
}

/// §19.5.3 animEffect (transition filter).
pub(crate) fn anim_effect(
    ids: &mut Ids,
    spid: u32,
    dur: u32,
    filter: &str,
    dir: &str,
) -> XmlElement {
    El::new("p:animEffect")
        .a("transition", dir)
        .a("filter", filter)
        .child(
            El::new("p:cBhvr")
                .child(El::new("p:cTn").a("id", ids.next()).a("dur", dur).done())
                .child(tgt(spid))
                .done(),
        )
        .done()
}

/// §19.5.1 anim of one position attribute between two formula values.
pub(crate) fn anim_pos(
    ids: &mut Ids,
    spid: u32,
    dur: u32,
    attr: &str,
    from: &str,
    to: &str,
) -> XmlElement {
    let tav = |tm: &str, v: &str| {
        El::new("p:tav")
            .a("tm", tm)
            .child(
                El::new("p:val")
                    .child(El::new("p:strVal").a("val", v).done())
                    .done(),
            )
            .done()
    };
    El::new("p:anim")
        .a("calcmode", "lin")
        .a("valueType", "num")
        .child(
            El::new("p:cBhvr")
                .a("additive", "base")
                .child(
                    El::new("p:cTn")
                        .a("id", ids.next())
                        .a("dur", dur)
                        .a("fill", "hold")
                        .done(),
                )
                .child(tgt(spid))
                .child(attr_names(attr))
                .done(),
        )
        .child(
            El::new("p:tavLst")
                .child(tav("0", from))
                .child(tav("100000", to))
                .done(),
        )
        .done()
}

/// Give every time node of a reused effect a fresh `cTn@id` (§19.5.29: unique in the slide).
fn renumber(el: &mut XmlElement, ids: &mut Ids) {
    if el.name == "p:cTn" {
        let id = ids.next().to_string();
        match el.attrs.iter_mut().find(|(k, _)| k == "id") {
            Some((_, v)) => *v = id,
            None => el.attrs.insert(0, ("id".into(), id)),
        }
    }
    for c in &mut el.children {
        if let XmlNode::Element(e) = c {
            renumber(e, ids);
        }
    }
}

/// An unmodelled effect as read, with the trigger and delay of the model
/// (§19.5.29 cTn@nodeType, §19.5.73 stCondLst).
fn reused_effect(ids: &mut Ids, a: &Animation, src: &XmlElement) -> XmlElement {
    let mut par = src.clone();
    renumber(&mut par, ids);
    let node_type = node_type(a.trigger);
    for c in &mut par.children {
        let XmlNode::Element(ctn) = c else { continue };
        if ctn.name != "p:cTn" {
            continue;
        }
        match ctn.attrs.iter_mut().find(|(k, _)| k == "nodeType") {
            Some((_, v)) => *v = node_type.to_owned(),
            None => ctn.attrs.push(("nodeType".into(), node_type.to_owned())),
        }
        // §19.5.29 repeatCount of the model.
        ctn.attrs.retain(|(k, _)| k != "repeatCount");
        if let Some(r) = repeat_attr(a.repeat) {
            ctn.attrs.push(("repeatCount".into(), r));
        }
        let delay = a.delay_ms.to_string();
        let at = ctn
            .children
            .iter()
            .position(|c| matches!(c, XmlNode::Element(e) if e.name == "p:stCondLst"));
        let Some(at) = at else {
            ctn.children.insert(0, XmlNode::Element(cond(&delay)));
            break;
        };
        if let XmlNode::Element(l) = &mut ctn.children[at] {
            let first = l.children.iter_mut().find_map(|c| match c {
                XmlNode::Element(e) if e.name == "p:cond" => Some(e),
                XmlNode::Element(_) | XmlNode::Text(_) => None,
            });
            if let Some(cnd) = first {
                match cnd.attrs.iter_mut().find(|(k, _)| k == "delay") {
                    Some((_, v)) => *v = delay,
                    None => cnd.attrs.push(("delay".into(), delay)),
                }
            }
        }
        break;
    }
    par
}

/// §19.5.29 repeatCount of a repeat (1000ths, `indefinite`).
fn repeat_attr(r: Option<u32>) -> Option<String> {
    match r {
        Some(u32::MAX) => Some("indefinite".into()),
        Some(n) if n != 1000 => Some(n.to_string()),
        _ => None,
    }
}

fn node_type(t: AnimTrigger) -> &'static str {
    match t {
        AnimTrigger::OnClick => "clickEffect",
        AnimTrigger::WithPrevious => "withEffect",
        AnimTrigger::AfterPrevious => "afterEffect",
    }
}

fn effect_node(ids: &mut Ids, a: &Animation) -> XmlElement {
    if let Some(src) = a.source.as_ref().filter(|_| keeps_source(a)) {
        return reused_effect(ids, a, src);
    }
    let id = ids.next();
    let dur = a.dur_ms.max(1);
    let exit = a.class == AnimClass::Exit;
    let dir = a.direction.as_deref().unwrap_or("b");
    let mut behaviours = Vec::new();
    let named = match &a.effect {
        AnimEffect::Named(n) => crate::anim_catalog::by_id(n),
        _ => None,
    };
    // Emphasis and motion paths act on a visible shape: no visibility sets.
    let shows = matches!(a.class, AnimClass::Entrance | AnimClass::Exit);
    if !exit && shows {
        behaviours.push(set_visibility(ids, a.shape_id, 0, true));
    }
    match &a.effect {
        AnimEffect::Fade => behaviours.push(anim_effect(
            ids,
            a.shape_id,
            dur,
            "fade",
            if exit { "out" } else { "in" },
        )),
        AnimEffect::Wipe => {
            let f = match dir {
                "t" => "wipe(down)",
                "b" => "wipe(up)",
                "l" => "wipe(right)",
                _ => "wipe(left)",
            };
            behaviours.push(anim_effect(
                ids,
                a.shape_id,
                dur,
                f,
                if exit { "out" } else { "in" },
            ));
        }
        AnimEffect::FlyIn => {
            let (attr, from, to) = match dir {
                "t" => ("ppt_y", "0-#ppt_h/2", "#ppt_y"),
                "l" => ("ppt_x", "0-#ppt_w/2", "#ppt_x"),
                "r" => ("ppt_x", "1+#ppt_w/2", "#ppt_x"),
                _ => ("ppt_y", "1+#ppt_h/2", "#ppt_y"),
            };
            let (from, to) = if exit { (to, from) } else { (from, to) };
            behaviours.push(anim_pos(ids, a.shape_id, dur, attr, from, to));
        }
        AnimEffect::Named(_) => {
            if let Some(d) = named {
                behaviours.extend(crate::anim_catalog::behaviours(
                    ids,
                    d,
                    a.direction.as_deref(),
                    a.shape_id,
                    dur,
                    exit,
                ));
            }
        }
        AnimEffect::Appear | AnimEffect::Other => {}
    }
    if exit && shows {
        let end = if matches!(a.effect, AnimEffect::Appear | AnimEffect::Other)
            || named.is_some_and(|d| d.id == "appear")
        {
            0
        } else {
            dur.saturating_sub(1)
        };
        behaviours.push(set_visibility(ids, a.shape_id, end, false));
    }
    let class = match &a.class {
        AnimClass::Entrance => "entr",
        AnimClass::Exit => "exit",
        AnimClass::Emphasis => "emph",
        AnimClass::Path => "path",
        AnimClass::Other(o) => o.as_str(),
    };
    // Preset ids as written by common producers (entrance: 1 appear, 2 fly, 10 fade, 22 wipe).
    let preset = a.preset_id.or(match a.effect {
        AnimEffect::Appear => Some(1),
        AnimEffect::FlyIn => Some(2),
        AnimEffect::Fade => Some(10),
        AnimEffect::Wipe => Some(22),
        AnimEffect::Named(_) => named.map(|d| d.preset),
        AnimEffect::Other => None,
    });
    let subtype = a.preset_subtype.or(match (&a.effect, dir) {
        (AnimEffect::Named(_), _) => {
            named.and_then(|d| crate::anim_catalog::subtype(d, a.direction.as_deref()))
        }
        (AnimEffect::FlyIn | AnimEffect::Wipe, "t") => Some(1),
        (AnimEffect::FlyIn | AnimEffect::Wipe, "r") => Some(2),
        (AnimEffect::FlyIn | AnimEffect::Wipe, "l") => Some(8),
        (AnimEffect::FlyIn | AnimEffect::Wipe, _) => Some(4),
        _ => Some(0),
    });
    let node_type = node_type(a.trigger);
    let mut kids = El::new("p:childTnLst");
    for b in behaviours {
        kids = kids.child(b);
    }
    El::new("p:par")
        .child(
            El::new("p:cTn")
                .a("id", id)
                .opt("presetID", preset)
                .a("presetClass", class)
                .opt("presetSubtype", subtype)
                .opt("repeatCount", repeat_attr(a.repeat))
                .a("fill", "hold")
                .a("nodeType", node_type)
                .child(cond(&a.delay_ms.to_string()))
                .child(kids.done())
                .done(),
        )
        .done()
}

fn par(ids: &mut Ids, delay: &str, children: Vec<XmlElement>) -> XmlElement {
    let mut kids = El::new("p:childTnLst");
    for c in children {
        kids = kids.child(c);
    }
    El::new("p:par")
        .child(
            El::new("p:cTn")
                .a("id", ids.next())
                .a("fill", "hold")
                .child(cond(delay))
                .child(kids.done())
                .done(),
        )
        .done()
}

/// Build `p:timing` for a list of animations (§19.5 structure).
/// The click steps of a sequence: start groups of effects (after-previous starts a new group).
fn click_pars(ids: &mut Ids, anims: &[&Animation], auto_first: bool) -> Vec<XmlElement> {
    let mut clicks: Vec<Vec<Vec<&Animation>>> = Vec::new();
    for a in anims {
        match a.trigger {
            AnimTrigger::OnClick => clicks.push(vec![vec![a]]),
            AnimTrigger::WithPrevious => match clicks.last_mut().and_then(|c| c.last_mut()) {
                Some(g) => g.push(a),
                None => clicks.push(vec![vec![a]]),
            },
            AnimTrigger::AfterPrevious => match clicks.last_mut() {
                Some(c) => c.push(vec![a]),
                None => clicks.push(vec![vec![a]]),
            },
        }
    }
    let mut out = Vec::new();
    for (ci, groups) in clicks.iter().enumerate() {
        let first_auto = ci == 0 && auto_first;
        let mut group_pars = Vec::new();
        let mut t = 0u32;
        for g in groups {
            let effects: Vec<XmlElement> = g.iter().map(|a| effect_node(ids, a)).collect();
            group_pars.push(par(ids, &t.to_string(), effects));
            t += g
                .iter()
                .map(|a| {
                    let runs = match a.repeat {
                        Some(r) if r != u32::MAX => u64::from(a.dur_ms) * u64::from(r) / 1000,
                        _ => u64::from(a.dur_ms),
                    };
                    a.delay_ms
                        .saturating_add(u32::try_from(runs).unwrap_or(u32::MAX))
                })
                .max()
                .unwrap_or(0);
        }
        out.push(par(
            ids,
            if first_auto { "0" } else { "indefinite" },
            group_pars,
        ));
    }
    out
}

fn build_timing(anims: &[Animation]) -> XmlElement {
    let mut ids = Ids(2);
    let cond_lst = |name: &str, evt: &str| {
        El::new(name)
            .child(
                El::new("p:cond")
                    .a("evt", evt)
                    .child(El::new("p:tgtEl").child(El::new("p:sldTgt").done()).done())
                    .done(),
            )
            .done()
    };
    let mut root = El::new("p:childTnLst");
    let main: Vec<&Animation> = anims.iter().filter(|a| a.trigger_shape.is_none()).collect();
    if !main.is_empty() {
        let first_auto = main
            .first()
            .is_some_and(|a| a.trigger != AnimTrigger::OnClick);
        let mut seq_kids = El::new("p:childTnLst");
        for c in click_pars(&mut ids, &main, first_auto) {
            seq_kids = seq_kids.child(c);
        }
        root = root.child(
            El::new("p:seq")
                .a("concurrent", 1)
                .a("nextAc", "seek")
                .child(
                    El::new("p:cTn")
                        .a("id", 2)
                        .a("dur", "indefinite")
                        .a("nodeType", "mainSeq")
                        .child(seq_kids.done())
                        .done(),
                )
                .child(cond_lst("p:prevCondLst", "onPrev"))
                .child(cond_lst("p:nextCondLst", "onNext"))
                .done(),
        );
    }
    // §19.5.65 interactive sequences: started by a click on their trigger shape.
    let mut triggers: Vec<u32> = Vec::new();
    for a in anims {
        if let Some(t) = a.trigger_shape {
            if !triggers.contains(&t) {
                triggers.push(t);
            }
        }
    }
    for t in triggers {
        let group: Vec<&Animation> = anims
            .iter()
            .filter(|a| a.trigger_shape == Some(t))
            .collect();
        let on_click = || {
            El::new("p:cond")
                .a("evt", "onClick")
                .a("delay", 0)
                .child(tgt(t))
                .done()
        };
        let mut kids = El::new("p:childTnLst");
        for c in click_pars(&mut ids, &group, true) {
            kids = kids.child(c);
        }
        let id = ids.next();
        root = root.child(
            El::new("p:seq")
                .a("concurrent", 1)
                .a("nextAc", "seek")
                .child(
                    El::new("p:cTn")
                        .a("id", id)
                        .a("restart", "whenNotActive")
                        .a("fill", "hold")
                        .a("evtFilter", "cancelBubble")
                        .a("nodeType", "interactiveSeq")
                        .child(El::new("p:stCondLst").child(on_click()).done())
                        .child(
                            El::new("p:endSync")
                                .a("evt", "end")
                                .a("delay", 0)
                                .child(El::new("p:rtn").a("val", "all").done())
                                .done(),
                        )
                        .child(kids.done())
                        .done(),
                )
                .child(El::new("p:nextCondLst").child(on_click()).done())
                .done(),
        );
    }
    El::new("p:timing")
        .child(
            El::new("p:tnLst")
                .child(
                    El::new("p:par")
                        .child(
                            El::new("p:cTn")
                                .a("id", 1)
                                .a("dur", "indefinite")
                                .a("restart", "never")
                                .a("nodeType", "tmRoot")
                                .child(root.done())
                                .done(),
                        )
                        .done(),
                )
                .done(),
        )
        .done()
}

/// Serialize timing: the source when the animations are unchanged.
pub fn write_timing(t: &Timing) -> Option<Extra> {
    if let Some(src) = &t.source {
        if parse_timing(&src.xml, src.key).animations == t.animations {
            return Some(src.clone());
        }
    }
    if t.animations.is_empty() {
        return None;
    }
    Some(Extra {
        key: KEY_TIMING,
        xml: build_timing(&t.animations),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_drawingml::dom::parse;

    fn anim(effect: AnimEffect, dir: Option<&str>, trigger: AnimTrigger) -> Animation {
        Animation {
            shape_id: 4,
            class: AnimClass::Entrance,
            effect,
            direction: dir.map(str::to_owned),
            trigger,
            delay_ms: 0,
            dur_ms: 500,
            preset_id: None,
            preset_subtype: None,
            repeat: None,
            trigger_shape: None,
            source: None,
        }
    }

    #[test]
    fn transition_alternate_content() {
        let xml = r#"<mc:AlternateContent xmlns:mc="m"><mc:Choice xmlns:p14="x" Requires="p14"><p:transition spd="slow" p14:dur="1200" advTm="3000"><p:push dir="u"/></p:transition></mc:Choice><mc:Fallback><p:transition spd="slow" advTm="3000"><p:push dir="u"/></p:transition></mc:Fallback></mc:AlternateContent>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let t = parse_transition(&el, 5).unwrap();
        assert_eq!(t.kind, Some(TransitionKind::Push));
        assert_eq!(t.dir.as_deref(), Some("u"));
        assert_eq!(t.dur_ms, Some(1200));
        assert_eq!(t.advance_after_ms, Some(3000));
        assert_eq!(write_transition(&t).xml, el);
        let mut edited = t.clone();
        edited.kind = Some(TransitionKind::Wipe);
        let w = write_transition(&edited);
        let back = parse_transition(&w.xml, w.key).unwrap();
        assert!(same_transition(&back, &edited));
    }

    #[test]
    fn generated_timing_parses_back() {
        let mut a2 = anim(AnimEffect::FlyIn, Some("l"), AnimTrigger::AfterPrevious);
        a2.shape_id = 5;
        let mut a4 = anim(AnimEffect::Fade, None, AnimTrigger::OnClick);
        a4.class = AnimClass::Exit;
        let anims = vec![
            anim(AnimEffect::Fade, None, AnimTrigger::OnClick),
            a2,
            anim(AnimEffect::Wipe, Some("t"), AnimTrigger::WithPrevious),
            anim(AnimEffect::Appear, None, AnimTrigger::OnClick),
            a4,
        ];
        let t = Timing {
            animations: anims.clone(),
            source: None,
        };
        let x = write_timing(&t).unwrap();
        let mut back = parse_timing(&x.xml, x.key);
        for a in &mut back.animations {
            a.preset_id = None;
            a.preset_subtype = None;
        }
        // Appear has a 1 ms visibility set.
        back.animations[3].dur_ms = 500;
        assert_eq!(back.animations, anims);
    }

    #[test]
    fn unknown_effects_survive_a_regenerated_sequence() {
        // An emphasis (grow, §19.5.4 animScale) after a fade: editing the fade
        // regenerates the sequence; the emphasis is written as read.
        let xml = r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst><p:par><p:cTn id="3" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="4" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="5" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" nodeType="clickEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="6" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="4"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set><p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="7" dur="500"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par><p:par><p:cTn id="8" presetID="6" presetClass="emph" presetSubtype="0" fill="hold" nodeType="withEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animScale><p:cBhvr><p:cTn id="9" dur="2000" fill="hold"/><p:tgtEl><p:spTgt spid="5"/></p:tgtEl></p:cBhvr><p:by x="150000" y="150000"/></p:animScale></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let t = parse_timing(&el, KEY_TIMING);
        assert_eq!(t.animations.len(), 2);
        assert_eq!(t.animations[1].class, AnimClass::Emphasis);
        assert!(t.animations[1].source.is_some());
        assert!(t.animations[0].source.is_none());
        assert_eq!(write_timing(&t).unwrap().xml, el);

        let mut edited = t.clone();
        edited.animations[0].dur_ms = 900;
        edited.animations[1].trigger = AnimTrigger::AfterPrevious;
        edited.animations[1].delay_ms = 250;
        let w = write_timing(&edited).unwrap();
        let mut text = String::new();
        bo_drawingml::dom::write_element(&w.xml, &mut text);
        assert!(text.contains(r#"<p:by x="150000" y="150000"/>"#), "{text}");
        let back = parse_timing(&w.xml, w.key);
        assert_eq!(back.animations.len(), 2);
        assert_eq!(back.animations[0].dur_ms, 900);
        let b = &back.animations[1];
        assert_eq!(
            (b.class.clone(), b.trigger, b.delay_ms, b.shape_id),
            (AnimClass::Emphasis, AnimTrigger::AfterPrevious, 250, 5)
        );
        // Time node ids stay unique.
        let mut all = Vec::new();
        descendants(&w.xml, &mut all);
        let mut ids: Vec<&str> = all
            .iter()
            .filter(|e| e.name == "p:cTn")
            .filter_map(|e| e.attr("id"))
            .collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn unknown_transition_keeps_its_element() {
        let xml = r#"<p:transition spd="med"><p:dissolve thruBlk="1"/></p:transition>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let mut t = parse_transition(&el, KEY_TRANSITION).unwrap();
        assert_eq!(t.kind, Some(TransitionKind::Other("dissolve".into())));
        t.speed = Some("slow".into());
        t.advance_after_ms = Some(2000);
        let w = write_transition(&t);
        let mut text = String::new();
        bo_drawingml::dom::write_element(&w.xml, &mut text);
        assert!(text.contains(r#"<p:dissolve thruBlk="1"/>"#), "{text}");
        assert!(text.contains(r#"spd="slow""#), "{text}");
    }
}
