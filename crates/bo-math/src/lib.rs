//! Equation input for BlueOffice (`docs/parity/BACKLOG.md` CORE-MATH-005..007).
//!
//! * [`unicode_math`]: the UnicodeMath linear format (after the public Unicode Technical Note #28,
//!   "UnicodeMath, a Nearly Plain-Text Encoding of Mathematics") parsed into an expression tree.
//! * [`latex`]: LaTeX math (the common amsmath subset) parsed into the same tree.
//! * [`Node`]: the expression tree, written as Office Math Markup (ECMA-376 Part 1, §22.1) by
//!   [`to_omml`].
#![forbid(unsafe_code)]

pub mod edit;
pub mod latex;
pub mod linear;
pub mod omml;
pub mod read;
pub mod unicode_math;

pub use linear::to_linear;
pub use omml::to_omml;
pub use read::from_omml;

pub use bo_model::slide::XmlElement;

/// An equation as a tree (the shapes of Office Math, §22.1.2).
///
/// Every structure keeps its property element ([`Pr`]: the children of `m:fPr`, `m:dPr`, `m:naryPr`,
/// …, `m:ctrlPr` included) and every run its run properties ([`RunPr`]: `m:rPr`, `w:rPr`), so an
/// equation written by another application is written back as it was (CORE-MATH-010, ADR-0015
/// addendum C).
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Text: italic letters by default, `Style::Upright` for function names, `Style::Normal` for
    /// literal text (§22.1.2.98 r, §22.1.2.90 sty, §22.1.2.73 nor), with its run properties.
    Run(String, Style, RunPr),
    /// Fraction (§22.1.2.36 f); `bar: false` for a stacked fraction without a bar (noBar).
    Frac {
        /// Numerator.
        num: Vec<Node>,
        /// Denominator.
        den: Vec<Node>,
        /// Fraction bar.
        bar: bool,
        /// `m:fPr`.
        pr: Pr,
    },
    /// Base with subscript and/or superscript (§22.1.2.110 sSub, §22.1.2.112 sSup, §22.1.2.111 sSubSup).
    Scripts {
        /// Base.
        base: Vec<Node>,
        /// Subscript.
        sub: Option<Vec<Node>>,
        /// Superscript.
        sup: Option<Vec<Node>>,
        /// `m:sSubPr` / `m:sSupPr` / `m:sSubSupPr`.
        pr: Pr,
    },
    /// Radical (§22.1.2.88 rad) with an optional degree.
    Rad {
        /// Degree (`None`: square root).
        deg: Option<Vec<Node>>,
        /// Radicand.
        body: Vec<Node>,
        /// `m:radPr`.
        pr: Pr,
    },
    /// N-ary operator (§22.1.2.70 nary) with limits.
    Nary {
        /// Operator character.
        chr: char,
        /// Lower limit.
        sub: Option<Vec<Node>>,
        /// Upper limit.
        sup: Option<Vec<Node>>,
        /// Operand.
        body: Vec<Node>,
        /// `m:naryPr`.
        pr: Pr,
    },
    /// Delimiters (§22.1.2.24 d) around one or more parts separated by `sep`.
    Delim {
        /// Opening character (`None`: invisible).
        open: Option<char>,
        /// Closing character (`None`: invisible).
        close: Option<char>,
        /// Parts.
        parts: Vec<Vec<Node>>,
        /// `m:dPr`.
        pr: Pr,
    },
    /// Function application (§22.1.2.39 func): name (upright) and argument.
    Func {
        /// Function name (may carry a limit: `lim` with `LimLow`).
        name: Vec<Node>,
        /// Argument.
        arg: Vec<Node>,
        /// `m:funcPr`.
        pr: Pr,
    },
    /// Limit under or over a base (§22.1.2.54 limLow, §22.1.2.56 limUpp).
    Limit {
        /// Base.
        base: Vec<Node>,
        /// Limit.
        lim: Vec<Node>,
        /// Above the base (limUpp) rather than below.
        upper: bool,
        /// `m:limLowPr` / `m:limUppPr`.
        pr: Pr,
    },
    /// Matrix (§22.1.2.60 m): rows of cells; `m:mPr`.
    Matrix(Vec<Vec<Vec<Node>>>, Pr),
    /// Equation array (§22.1.2.34 eqArr): one equation per row; `m:eqArrPr`.
    EqArray(Vec<Vec<Node>>, Pr),
    /// Accent over the base (§22.1.2.1 acc), a combining character.
    Accent {
        /// The combining accent.
        chr: char,
        /// Base.
        base: Vec<Node>,
        /// `m:accPr`.
        pr: Pr,
    },
    /// Bar over or under the base (§22.1.2.7 bar).
    Bar {
        /// Over the base.
        top: bool,
        /// Base.
        base: Vec<Node>,
        /// `m:barPr`.
        pr: Pr,
    },
    /// Grouping character (a brace) under or over the base (ECMA-376 Part 1, §22.1.2 groupChr; the
    /// character and its position are in `m:groupChrPr`).
    GroupChr {
        /// Base.
        base: Vec<Node>,
        /// `m:groupChrPr`.
        pr: Pr,
    },
    /// Box (§22.1.2 box): the base as one unit (operator emulation, no break, … in `m:boxPr`).
    Boxed {
        /// Base.
        base: Vec<Node>,
        /// `m:boxPr`.
        pr: Pr,
    },
    /// Border box (§22.1.2 borderBox): a frame (or some of its sides, strikes) around the base.
    BorderBox {
        /// Base.
        base: Vec<Node>,
        /// `m:borderBoxPr`.
        pr: Pr,
    },
    /// Phantom (§22.1.2 phant): the base takes its room but is not shown unless `m:show` is on.
    Phantom {
        /// Base.
        base: Vec<Node>,
        /// `m:phantPr`.
        pr: Pr,
    },
    /// Pre-sub-superscript (§22.1.2 sPre): scripts before the base.
    PreScripts {
        /// Subscript.
        sub: Vec<Node>,
        /// Superscript.
        sup: Vec<Node>,
        /// Base.
        base: Vec<Node>,
        /// `m:sPrePr`.
        pr: Pr,
    },
}

impl Node {
    /// A run of `text` without run properties.
    #[must_use]
    pub fn run(text: impl Into<String>, style: Style) -> Node {
        Node::Run(text.into(), style, RunPr::default())
    }

    /// The property element of a structure (`None` for a run).
    #[must_use]
    pub fn pr(&self) -> Option<&Pr> {
        match self {
            Node::Run(..) => None,
            Node::Frac { pr, .. }
            | Node::Scripts { pr, .. }
            | Node::Rad { pr, .. }
            | Node::Nary { pr, .. }
            | Node::Delim { pr, .. }
            | Node::Func { pr, .. }
            | Node::Limit { pr, .. }
            | Node::Matrix(_, pr)
            | Node::EqArray(_, pr)
            | Node::Accent { pr, .. }
            | Node::Bar { pr, .. }
            | Node::GroupChr { pr, .. }
            | Node::Boxed { pr, .. }
            | Node::BorderBox { pr, .. }
            | Node::Phantom { pr, .. }
            | Node::PreScripts { pr, .. } => Some(pr),
        }
    }

    /// [`Node::pr`], mutable.
    pub fn pr_mut(&mut self) -> Option<&mut Pr> {
        match self {
            Node::Run(..) => None,
            Node::Frac { pr, .. }
            | Node::Scripts { pr, .. }
            | Node::Rad { pr, .. }
            | Node::Nary { pr, .. }
            | Node::Delim { pr, .. }
            | Node::Func { pr, .. }
            | Node::Limit { pr, .. }
            | Node::Matrix(_, pr)
            | Node::EqArray(_, pr)
            | Node::Accent { pr, .. }
            | Node::Bar { pr, .. }
            | Node::GroupChr { pr, .. }
            | Node::Boxed { pr, .. }
            | Node::BorderBox { pr, .. }
            | Node::Phantom { pr, .. }
            | Node::PreScripts { pr, .. } => Some(pr),
        }
    }
}

/// A property element as read from a file: its children verbatim, in document order (properties the
/// tree has no value for and `m:ctrlPr` — the formatting of the structure's operator characters,
/// ECMA-376 Part 1, §22.1.2 ctrlPr — included).
///
/// The tree also holds some properties as values (a fraction's bar, a delimiter's characters, the
/// limits of an n-ary operator shown or hidden, a run's style). `read` keeps those values as they
/// were read: unchanged, the children are written back verbatim; changed, only the property elements
/// of the changed values are set (in schema order), the others kept. `Pr::default()` — a structure
/// made here — writes BlueOffice's canonical properties.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pr {
    /// Children of the property element.
    pub kids: Vec<XmlElement>,
    /// The modelled values as read (one signature per modelled property); `None`: made here.
    pub read: Option<Vec<String>>,
}

impl Pr {
    /// The properties of a structure made while editing next to text formatted with `rpr` (a
    /// `w:rPr` element): its control properties carry that formatting, as Office Math structures
    /// do (§22.1.2 ctrlPr).
    #[must_use]
    pub fn with_ctrl(rpr: Option<&XmlElement>) -> Pr {
        Pr {
            kids: rpr
                .map(|r| {
                    vec![XmlElement {
                        name: "m:ctrlPr".into(),
                        attrs: Vec::new(),
                        children: vec![bo_model::slide::XmlNode::Element(r.clone())],
                    }]
                })
                .unwrap_or_default(),
            read: None,
        }
    }

    /// The `w:rPr` of the control properties (`m:ctrlPr`), if any.
    #[must_use]
    pub fn ctrl_rpr(&self) -> Option<&XmlElement> {
        self.kids
            .iter()
            .find(|k| k.local() == "ctrlPr")
            .and_then(|c| c.elements().find(|e| e.local() == "rPr"))
    }
}

/// Run properties as read from a file: the math run properties (`m:rPr`, ECMA-376 Part 1, §22.1.2
/// rPr — literal, normal text, script, style, break, alignment) and the WordprocessingML run
/// properties (`w:rPr`, §17.3.2.28 rPr: font, size, colour, …), kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunPr {
    /// Children of `m:rPr` (the style, `m:sty` / `m:nor`, is the run's [`Style`]).
    pub math: Pr,
    /// The `w:rPr` element.
    pub text: Option<XmlElement>,
}

impl RunPr {
    /// The properties of a new run made from text typed into a run with these: the same
    /// formatting, the style set anew (`m:sty` / `m:nor` follow the new run's [`Style`]).
    #[must_use]
    pub fn inherit(&self) -> RunPr {
        RunPr {
            math: Pr {
                kids: self
                    .math
                    .kids
                    .iter()
                    .filter(|k| !matches!(k.local(), "sty" | "nor"))
                    .cloned()
                    .collect(),
                read: None,
            },
            text: self.text.clone(),
        }
    }

    /// Only the formatting (`w:rPr`) of these, for a run typed where no run is next to the caret.
    #[must_use]
    pub fn text_only(text: Option<XmlElement>) -> RunPr {
        RunPr {
            math: Pr::default(),
            text,
        }
    }
}

/// Style of a [`Node::Run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Math italic letters (the default).
    Math,
    /// Upright math (function names: sty = p).
    Upright,
    /// Ordinary text (nor).
    Normal,
}
