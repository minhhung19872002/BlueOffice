//! Equation input for BlueOffice (`docs/parity/BACKLOG.md` CORE-MATH-005..007).
//!
//! * [`unicode_math`]: the UnicodeMath linear format (after the public Unicode Technical Note #28,
//!   "UnicodeMath, a Nearly Plain-Text Encoding of Mathematics") parsed into an expression tree.
//! * [`latex`]: LaTeX math (the common amsmath subset) parsed into the same tree.
//! * [`Node`]: the expression tree, written as Office Math Markup (ECMA-376 Part 1, §22.1) by
//!   [`to_omml`].
#![forbid(unsafe_code)]

pub mod latex;
pub mod omml;
pub mod unicode_math;

pub use omml::to_omml;

/// An equation as a tree (the shapes of Office Math, §22.1.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Text: italic letters by default, `Style::Upright` for function names, `Style::Normal` for
    /// literal text (§22.1.2.98 r, §22.1.2.90 sty, §22.1.2.73 nor).
    Run(String, Style),
    /// Fraction (§22.1.2.36 f); `bar: false` for a stacked fraction without a bar (noBar).
    Frac {
        /// Numerator.
        num: Vec<Node>,
        /// Denominator.
        den: Vec<Node>,
        /// Fraction bar.
        bar: bool,
    },
    /// Base with subscript and/or superscript (§22.1.2.110 sSub, §22.1.2.112 sSup, §22.1.2.111 sSubSup).
    Scripts {
        /// Base.
        base: Vec<Node>,
        /// Subscript.
        sub: Option<Vec<Node>>,
        /// Superscript.
        sup: Option<Vec<Node>>,
    },
    /// Radical (§22.1.2.88 rad) with an optional degree.
    Rad {
        /// Degree (`None`: square root).
        deg: Option<Vec<Node>>,
        /// Radicand.
        body: Vec<Node>,
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
    },
    /// Delimiters (§22.1.2.24 d) around one or more parts separated by `sep`.
    Delim {
        /// Opening character (`None`: invisible).
        open: Option<char>,
        /// Closing character (`None`: invisible).
        close: Option<char>,
        /// Parts.
        parts: Vec<Vec<Node>>,
    },
    /// Function application (§22.1.2.39 func): name (upright) and argument.
    Func {
        /// Function name (may carry a limit: `lim` with `LimLow`).
        name: Vec<Node>,
        /// Argument.
        arg: Vec<Node>,
    },
    /// Limit under or over a base (§22.1.2.54 limLow, §22.1.2.56 limUpp).
    Limit {
        /// Base.
        base: Vec<Node>,
        /// Limit.
        lim: Vec<Node>,
        /// Above the base (limUpp) rather than below.
        upper: bool,
    },
    /// Matrix (§22.1.2.60 m): rows of cells.
    Matrix(Vec<Vec<Vec<Node>>>),
    /// Equation array (§22.1.2.34 eqArr): one equation per row.
    EqArray(Vec<Vec<Node>>),
    /// Accent over the base (§22.1.2.1 acc), a combining character.
    Accent {
        /// The combining accent.
        chr: char,
        /// Base.
        base: Vec<Node>,
    },
    /// Bar over or under the base (§22.1.2.7 bar).
    Bar {
        /// Over the base.
        top: bool,
        /// Base.
        base: Vec<Node>,
    },
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
