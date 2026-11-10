//! Hyperlinks and actions of shapes and text runs (SLIDE-ACTION-001).
//!
//! * ECMA-376 Part 1, §21.1.2.3.5 `a:hlinkClick`, §21.1.2.3.6 `a:hlinkHover` (in
//!   §20.1.2.2.8 `cNvPr`) and §21.1.2.3.7 `a:hlinkMouseOver` (in §21.1.2.3.9 `rPr`):
//!   all `CT_Hyperlink` — `r:id`, `action`, `tooltip`, `history`, `highlightClick`,
//!   `endSnd`, `invalidUrl`, `tgtFrame`, and the children `a:snd` (§20.1.2.2.32,
//!   an embedded WAV) and `a:extLst`.
//! * The `action` attribute is an application URI; presentations use the
//!   `ppaction://` scheme ([MS-OE376] part 1, §21.1.2.3.5 lists the values written
//!   by presentation applications). [`ActionKind`] names the ones BlueSlide
//!   understands; everything else is [`ActionKind::Unknown`] and kept as written.
//!
//! The element itself stays in the model as an [`Extra`](bo_model::slide::Extra)
//! (its bytes are preserved); [`Hyperlink`] is the typed view used to read and
//! edit it. Attributes and children this view does not name are carried along
//! in `attrs` / `children`, so nothing is lost when a link is edited.

use crate::dom::{AttrBag, El};
use bo_model::slide::{XmlElement, XmlNode};

/// §20.1.2.2.32 snd: the embedded WAV played with the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlinkSound {
    /// `r:embed`: audio relationship of the owning part.
    pub embed: String,
    /// `name` of the sound.
    pub name: String,
    /// Other attributes, as written.
    pub attrs: Vec<(String, String)>,
}

/// Typed view of a `CT_Hyperlink` element.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hyperlink {
    /// `r:id` (empty when the action has no target part). `None`: not written.
    pub rid: Option<String>,
    /// `action`.
    pub action: Option<String>,
    /// `tooltip`.
    pub tooltip: Option<String>,
    /// `history` (schema default true): the target is added to the list of visited links.
    pub history: Option<bool>,
    /// `highlightClick` (schema default false).
    pub highlight_click: Option<bool>,
    /// `endSnd` (schema default false): stop the sound playing.
    pub end_snd: Option<bool>,
    /// `a:snd`.
    pub sound: Option<HlinkSound>,
    /// Attributes not named above (`invalidUrl`, `tgtFrame`, …), in document order.
    pub attrs: Vec<(String, String)>,
    /// Children other than `a:snd` (`a:extLst`), in document order.
    pub children: Vec<XmlNode>,
}

/// `hlinkshowjump?jump=…` destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowJump {
    /// `nextslide`.
    Next,
    /// `previousslide`.
    Previous,
    /// `firstslide`.
    First,
    /// `lastslide`.
    Last,
    /// `lastslideviewed`.
    LastViewed,
    /// `endshow`.
    EndShow,
}

impl ShowJump {
    /// The `jump` query value.
    pub fn as_str(self) -> &'static str {
        match self {
            ShowJump::Next => "nextslide",
            ShowJump::Previous => "previousslide",
            ShowJump::First => "firstslide",
            ShowJump::Last => "lastslide",
            ShowJump::LastViewed => "lastslideviewed",
            ShowJump::EndShow => "endshow",
        }
    }

    /// Parse a `jump` query value (ASCII case-insensitive).
    pub fn parse(s: &str) -> Option<ShowJump> {
        Some(match s.to_ascii_lowercase().as_str() {
            "nextslide" => ShowJump::Next,
            "previousslide" => ShowJump::Previous,
            "firstslide" => ShowJump::First,
            "lastslide" => ShowJump::Last,
            "lastslideviewed" => ShowJump::LastViewed,
            "endshow" => ShowJump::EndShow,
            _ => return None,
        })
    }
}

/// What the `action` URI of a hyperlink asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    /// No `action`: the relationship is the target (a URL, a mail address, a file).
    Target,
    /// `ppaction://noaction`: only the sound / highlight of the element.
    NoAction,
    /// `ppaction://hlinksldjump`: the slide of the (internal) relationship.
    SlideJump,
    /// `ppaction://hlinkshowjump?jump=…`.
    ShowJump(ShowJump),
    /// `ppaction://customshow?id=…&return=…`.
    CustomShow {
        /// `custShow@id`.
        id: u32,
        /// Come back to the linking slide after the show.
        ret: bool,
    },
    /// `ppaction://hlinkpres?slideindex=…&slidetitle=…`: another presentation.
    OtherPresentation {
        /// 1-based slide of the other deck, when given.
        slide_index: Option<u32>,
    },
    /// `ppaction://hlinkfile`: another file.
    File,
    /// `ppaction://program`: run a program.
    Program,
    /// `ppaction://macro?name=…`: run a macro.
    Macro {
        /// Macro name.
        name: String,
    },
    /// `ppaction://media`: play the clip of the shape.
    Media,
    /// `ppaction://ole?verb=…`: an OLE verb of the object.
    Ole,
    /// Anything else, kept as written and never run.
    Unknown,
}

/// Query parameter of an `action` URI (`scheme://name?a=b&c=d`).
pub fn action_query<'a>(action: &'a str, key: &str) -> Option<&'a str> {
    let (_, q) = action.split_once('?')?;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        k.eq_ignore_ascii_case(key).then_some(v)
    })
}

impl ActionKind {
    /// Classify an `action` attribute value.
    pub fn of(action: Option<&str>) -> ActionKind {
        let Some(action) = action.filter(|a| !a.is_empty()) else {
            return ActionKind::Target;
        };
        let lower = action.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("ppaction://") else {
            return ActionKind::Unknown;
        };
        let name = rest.split('?').next().unwrap_or_default();
        match name {
            "noaction" => ActionKind::NoAction,
            "hlinksldjump" => ActionKind::SlideJump,
            "hlinkshowjump" => action_query(action, "jump")
                .and_then(ShowJump::parse)
                .map_or(ActionKind::Unknown, ActionKind::ShowJump),
            "customshow" => match action_query(action, "id").and_then(|v| v.parse().ok()) {
                Some(id) => ActionKind::CustomShow {
                    id,
                    ret: action_query(action, "return")
                        .is_some_and(|v| v.eq_ignore_ascii_case("true")),
                },
                None => ActionKind::Unknown,
            },
            "hlinkpres" => ActionKind::OtherPresentation {
                slide_index: action_query(action, "slideindex").and_then(|v| v.parse().ok()),
            },
            "hlinkfile" => ActionKind::File,
            "program" => ActionKind::Program,
            "macro" => ActionKind::Macro {
                name: action_query(action, "name").unwrap_or_default().to_owned(),
            },
            "media" => ActionKind::Media,
            "ole" => ActionKind::Ole,
            _ => ActionKind::Unknown,
        }
    }

    /// The `action` attribute for this kind (`None`: no attribute). `Unknown`,
    /// `Media` and `Ole` are never built by the editor.
    pub fn uri(&self) -> Option<String> {
        Some(match self {
            ActionKind::Target | ActionKind::Unknown | ActionKind::Media | ActionKind::Ole => {
                return None
            }
            ActionKind::NoAction => "ppaction://noaction".into(),
            ActionKind::SlideJump => "ppaction://hlinksldjump".into(),
            ActionKind::ShowJump(j) => format!("ppaction://hlinkshowjump?jump={}", j.as_str()),
            ActionKind::CustomShow { id, ret } => format!(
                "ppaction://customshow?id={id}{}",
                if *ret { "&return=true" } else { "" }
            ),
            ActionKind::OtherPresentation { slide_index } => match slide_index {
                Some(i) => format!("ppaction://hlinkpres?slideindex={i}"),
                None => "ppaction://hlinkpres".into(),
            },
            ActionKind::File => "ppaction://hlinkfile".into(),
            ActionKind::Program => "ppaction://program".into(),
            ActionKind::Macro { name } => format!("ppaction://macro?name={name}"),
        })
    }
}

/// Whether `el` is one of the hyperlink elements (`hlinkClick`, `hlinkHover`, `hlinkMouseOver`).
pub fn is_hyperlink(el: &XmlElement) -> bool {
    matches!(el.local(), "hlinkClick" | "hlinkHover" | "hlinkMouseOver")
}

/// Read a `CT_Hyperlink` element.
pub fn parse_hyperlink(el: &XmlElement) -> Hyperlink {
    let mut bag = AttrBag::of(el);
    let rid = bag.take("r:id");
    let action = bag.take("action");
    let tooltip = bag.take("tooltip");
    // Booleans that do not parse stay among the other attributes, as written.
    let history = bag.bool("history");
    let highlight_click = bag.bool("highlightClick");
    let end_snd = bag.bool("endSnd");
    let mut sound = None;
    let mut children = Vec::new();
    for c in &el.children {
        match c {
            XmlNode::Element(e) if e.name == "a:snd" && sound.is_none() => {
                let mut b = AttrBag::of(e);
                sound = Some(HlinkSound {
                    embed: b.take("r:embed").unwrap_or_default(),
                    name: b.take("name").unwrap_or_default(),
                    attrs: b.rest(),
                });
            }
            other => children.push(other.clone()),
        }
    }
    Hyperlink {
        rid,
        action,
        tooltip,
        history,
        highlight_click,
        end_snd,
        sound,
        attrs: bag.rest(),
        children,
    }
}

/// Write a `CT_Hyperlink` element named `name` (`a:hlinkClick`, …).
pub fn write_hyperlink(name: &str, h: &Hyperlink) -> XmlElement {
    let mut el = El::new(name)
        .opt("r:id", h.rid.as_ref())
        .rest(&h.attrs)
        .opt("action", h.action.as_ref())
        .opt("tooltip", h.tooltip.as_ref())
        .optb("history", h.history)
        .optb("highlightClick", h.highlight_click)
        .optb("endSnd", h.end_snd)
        .done();
    // §21.1.2.3.5 CT_Hyperlink: snd comes before extLst.
    if let Some(s) = &h.sound {
        el.children.push(XmlNode::Element(
            El::new("a:snd")
                .a("r:embed", &s.embed)
                .a("name", &s.name)
                .rest(&s.attrs)
                .done(),
        ));
    }
    el.children.extend(h.children.iter().cloned());
    el
}

impl Hyperlink {
    /// What the `action` asks for.
    pub fn kind(&self) -> ActionKind {
        ActionKind::of(self.action.as_deref())
    }

    /// `history` with its schema default (true).
    pub fn in_history(&self) -> bool {
        self.history.unwrap_or(true)
    }

    /// `highlightClick` with its schema default (false).
    pub fn highlights(&self) -> bool {
        self.highlight_click.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom;

    fn el(xml: &str) -> XmlElement {
        let doc = format!(
            r#"<x xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{xml}</x>"#
        );
        let root = dom::parse(doc.as_bytes(), "t").expect("xml");
        let first = root.elements().next().cloned();
        first.expect("child")
    }

    /// Attributes sorted by name, children as written.
    fn shape(e: &XmlElement) -> (Vec<(String, String)>, String) {
        let mut attrs = e.attrs.clone();
        attrs.sort();
        let mut kids = String::new();
        for c in e.elements() {
            dom::write_element(c, &mut kids);
        }
        (attrs, kids)
    }

    #[test]
    fn classifies_ppaction_uris() {
        let k = |a: &str| ActionKind::of(Some(a));
        assert_eq!(ActionKind::of(None), ActionKind::Target);
        assert_eq!(k("ppaction://hlinksldjump"), ActionKind::SlideJump);
        assert_eq!(
            k("ppaction://hlinkshowjump?jump=lastslideviewed"),
            ActionKind::ShowJump(ShowJump::LastViewed)
        );
        assert_eq!(
            k("ppaction://customshow?id=3&return=true"),
            ActionKind::CustomShow { id: 3, ret: true }
        );
        assert_eq!(
            k("ppaction://hlinkpres?slideindex=2&slidetitle=Intro"),
            ActionKind::OtherPresentation {
                slide_index: Some(2)
            }
        );
        assert_eq!(k("ppaction://hlinkfile"), ActionKind::File);
        assert_eq!(k("ppaction://program"), ActionKind::Program);
        assert_eq!(
            k("ppaction://macro?name=Run"),
            ActionKind::Macro { name: "Run".into() }
        );
        assert_eq!(k("ppaction://noaction"), ActionKind::NoAction);
        assert_eq!(k("ppaction://media"), ActionKind::Media);
        // Unknown names, unknown jumps and other schemes are never interpreted.
        assert_eq!(
            k("ppaction://hlinkshowjump?jump=sideways"),
            ActionKind::Unknown
        );
        assert_eq!(k("ppaction://frobnicate?x=1"), ActionKind::Unknown);
        assert_eq!(k("vendor://do"), ActionKind::Unknown);
    }

    #[test]
    fn built_uris_classify_back() {
        for k in [
            ActionKind::NoAction,
            ActionKind::SlideJump,
            ActionKind::ShowJump(ShowJump::EndShow),
            ActionKind::CustomShow { id: 7, ret: false },
            ActionKind::CustomShow { id: 7, ret: true },
            ActionKind::File,
            ActionKind::Program,
            ActionKind::OtherPresentation {
                slide_index: Some(4),
            },
        ] {
            assert_eq!(ActionKind::of(k.uri().as_deref()), k);
        }
    }

    #[test]
    fn keeps_every_attribute_and_child() {
        for xml in [
            r#"<a:hlinkClick r:id="rId2"/>"#,
            r#"<a:hlinkClick r:id="" action="ppaction://hlinkshowjump?jump=nextslide" highlightClick="1"/>"#,
            r#"<a:hlinkHover r:id="" action="ppaction://noaction"><a:snd r:embed="rId3" name="chime.wav"/></a:hlinkHover>"#,
            r#"<a:hlinkClick r:id="rId4" invalidUrl="" tgtFrame="_blank" action="vendor://x?y=1" tooltip="Tip" history="0" endSnd="1"><a:extLst><a:ext uri="{A}"><a:q/></a:ext></a:extLst></a:hlinkClick>"#,
            r#"<a:hlinkMouseOver r:id="" action="ppaction://macro?name=M" highlightClick="maybe"/>"#,
        ] {
            let e = el(xml);
            let h = parse_hyperlink(&e);
            let back = write_hyperlink(&e.name, &h);
            assert_eq!(back.name, e.name);
            assert_eq!(shape(&back), shape(&e), "{xml}");
        }
    }

    #[test]
    fn reads_sound_and_flags() {
        let h = parse_hyperlink(&el(
            r#"<a:hlinkClick r:id="" action="ppaction://noaction" highlightClick="true"><a:snd r:embed="rId9" name="click.wav"/></a:hlinkClick>"#,
        ));
        assert!(h.highlights());
        assert!(h.in_history());
        let s = h.sound.clone().expect("sound");
        assert_eq!((s.embed.as_str(), s.name.as_str()), ("rId9", "click.wav"));
        assert_eq!(h.kind(), ActionKind::NoAction);
    }
}
