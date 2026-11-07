//! Slide transitions (ECMA-376 Part 1, §19.3.1.50 transition) and the main
//! animation sequence (§19.5 Animation, §19.3.1.48 timing).
//!
//! Both are read into the model and written back verbatim while unchanged;
//! edited (or new) ones are regenerated in the structure §19.5 describes
//! (tmRoot → mainSeq → click group → start group → effect node).

use bo_drawingml::dom::{attr_int, parse_bool, El};
use bo_model::slide::{
    AnimClass, AnimEffect, AnimTrigger, Animation, Extra, Timing, Transition, TransitionKind,
    XmlElement,
};

const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

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
        out.dir = fx
            .attr("dir")
            .or_else(|| fx.attr("orient"))
            .map(str::to_owned);
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
}

fn transition_body(t: &Transition, with_dur: bool) -> XmlElement {
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
        e = e.child(El::new(&name).opt(attr, t.dir.as_ref()).done());
    }
    e.done()
}

/// Serialize a transition: the source when unchanged, otherwise a new element
/// (wrapped in `mc:AlternateContent` with a p14 Choice when it has a duration).
pub fn write_transition(t: &Transition) -> Extra {
    if let Some(src) = &t.source {
        if parse_transition(&src.xml, src.key).is_some_and(|p| same_transition(&p, t)) {
            return src.clone();
        }
    }
    let xml = if t.dur_ms.is_some() {
        El::new("mc:AlternateContent")
            .a("xmlns:mc", NS_MC)
            .child(
                El::new("mc:Choice")
                    .a("xmlns:p14", NS_P14)
                    .a("Requires", "p14")
                    .child(transition_body(t, true))
                    .done(),
            )
            .child(
                El::new("mc:Fallback")
                    .child(transition_body(t, false))
                    .done(),
            )
            .done()
    } else {
        transition_body(t, false)
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
        let first = v
            .iter()
            .find(|e| e.name == "p:strVal")
            .and_then(|e| e.attr("val"))
            .unwrap_or_default()
            .to_owned();
        Some((attr, first))
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
    Some(Animation {
        shape_id,
        class,
        effect,
        direction,
        trigger,
        delay_ms: delay_of(ctn),
        dur_ms,
        preset_id: attr_int(ctn, "presetID"),
        preset_subtype: attr_int(ctn, "presetSubtype"),
    })
}

/// Parse §19.3.1.48 timing: the effects of the main sequence in order.
pub fn parse_timing(el: &XmlElement, key: u16) -> Timing {
    let mut animations = Vec::new();
    if let Some(seq) = main_seq(el) {
        let mut all = Vec::new();
        descendants(seq, &mut all);
        for e in all.into_iter().filter(|e| e.name == "p:cTn") {
            if let Some(a) = parse_effect(e) {
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

/// Allocates `cTn@id` values.
struct Ids(u32);

impl Ids {
    fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

fn cond(delay: &str) -> XmlElement {
    El::new("p:stCondLst")
        .child(El::new("p:cond").a("delay", delay).done())
        .done()
}

fn tgt(spid: u32) -> XmlElement {
    El::new("p:tgtEl")
        .child(El::new("p:spTgt").a("spid", spid).done())
        .done()
}

fn attr_names(name: &str) -> XmlElement {
    El::new("p:attrNameLst")
        .child(El::new("p:attrName").text(name).done())
        .done()
}

/// §19.5.66 set of `style.visibility`.
fn set_visibility(ids: &mut Ids, spid: u32, delay: u32, visible: bool) -> XmlElement {
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
fn anim_effect(ids: &mut Ids, spid: u32, dur: u32, filter: &str, dir: &str) -> XmlElement {
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
fn anim_pos(ids: &mut Ids, spid: u32, dur: u32, attr: &str, from: &str, to: &str) -> XmlElement {
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

fn effect_node(ids: &mut Ids, a: &Animation) -> XmlElement {
    let id = ids.next();
    let dur = a.dur_ms.max(1);
    let exit = a.class == AnimClass::Exit;
    let dir = a.direction.as_deref().unwrap_or("b");
    let mut behaviours = Vec::new();
    if !exit {
        behaviours.push(set_visibility(ids, a.shape_id, 0, true));
    }
    match a.effect {
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
        AnimEffect::Appear | AnimEffect::Other => {}
    }
    if exit {
        let end = if matches!(a.effect, AnimEffect::Appear | AnimEffect::Other) {
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
        AnimEffect::Other => None,
    });
    let subtype = a.preset_subtype.or(match (&a.effect, dir) {
        (AnimEffect::FlyIn | AnimEffect::Wipe, "t") => Some(1),
        (AnimEffect::FlyIn | AnimEffect::Wipe, "r") => Some(2),
        (AnimEffect::FlyIn | AnimEffect::Wipe, "l") => Some(8),
        (AnimEffect::FlyIn | AnimEffect::Wipe, _) => Some(4),
        _ => Some(0),
    });
    let node_type = match a.trigger {
        AnimTrigger::OnClick => "clickEffect",
        AnimTrigger::WithPrevious => "withEffect",
        AnimTrigger::AfterPrevious => "afterEffect",
    };
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
fn build_timing(anims: &[Animation]) -> XmlElement {
    let mut ids = Ids(2);
    // Group into click steps, then into start groups (after-previous starts a new group).
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
    let mut click_pars = Vec::new();
    for (ci, groups) in clicks.iter().enumerate() {
        let first_auto = ci == 0
            && anims
                .first()
                .is_some_and(|a| a.trigger != AnimTrigger::OnClick);
        let mut group_pars = Vec::new();
        let mut t = 0u32;
        for g in groups {
            let effects: Vec<XmlElement> = g.iter().map(|a| effect_node(&mut ids, a)).collect();
            group_pars.push(par(&mut ids, &t.to_string(), effects));
            t += g.iter().map(|a| a.delay_ms + a.dur_ms).max().unwrap_or(0);
        }
        click_pars.push(par(
            &mut ids,
            if first_auto { "0" } else { "indefinite" },
            group_pars,
        ));
    }
    let mut seq_kids = El::new("p:childTnLst");
    for c in click_pars {
        seq_kids = seq_kids.child(c);
    }
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
    let seq = El::new("p:seq")
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
        .done();
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
                                .child(El::new("p:childTnLst").child(seq).done())
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
}
