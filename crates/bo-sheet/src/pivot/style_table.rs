//! Built-in PivotTable styles as the reference spreadsheet draws them (black-box: the
//! DisplayFormat of a report with two row fields, round 14), as theme references so another
//! theme restyles them (ECMA-376 Part 1, §18.8.40 `tableStyleElement`, §18.8.19 `tint`).
//!
//! Each entry: style name, then twelve cells "fill font bold top bottom" — header (label
//! column, value column), first row subheading, plain data row, banded data row, grand total
//! row, last data row. Colours: `-` none, `a` the style's colour (dark 1 or accent 1–6 by
//! number), `d` dark 1, `l` light 1, each with an optional tint (`a+0.8`, `a-0.25`); borders
//! add `/t` thin, `/m` medium, `/k` thick, `/d` double.

pub(crate) const TABLE: &[(&str, &str)] = &[
    (
        "PivotStyleLight1",
        "- a 1 a+0.5/t a+0.5/t | - a 1 a+0.5/t a+0.5/t | - a 1 a+0.5/t a+0.85/t | - a 1 a+0.5/t a+0.85/t | - a 0 a+0.85/t a+0.85/t | - a 0 a+0.85/t a+0.85/t | a+0.85 a 0 a+0.65/t a+0.65/t | a+0.85 a 0 a+0.65/t a+0.65/t | l a 1 a+0.5/t a+0.5/t | l a 1 a+0.5/t a+0.5/t | - a 0 a+0.65/t a+0.5/t | - a 0 a+0.65/t a+0.5/t",
    ),
    (
        "PivotStyleLight2",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight3",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight4",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight5",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight6",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight7",
        "- d 1 a/t a/t | - d 1 a/t a/t | - d 1 a/t a+0.8/t | - d 1 a/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | l d 1 a/t a/t | l d 1 a/t a/t | - d 0 a+0.6/t a/t | - d 0 a+0.6/t a/t",
    ),
    (
        "PivotStyleLight8",
        "- a 1 a+0.5/m a+0.5/m | - a 1 a+0.5/m a+0.5/m | a+0.85 a 1 a+0.5/m a+0.85/t | a+0.85 a 1 a+0.5/m a+0.85/t | - a 0 a+0.85/t a+0.85/t | - a 0 a+0.85/t a+0.85/t | - a 0 a+0.65/t a+0.65/t | - a 0 a+0.65/t a+0.65/t | - a 1 a+0.5/m a+0.5/m | - a 1 a+0.5/m a+0.5/m | - a 0 a+0.65/t a+0.5/m | - a 0 a+0.65/t a+0.5/m",
    ),
    (
        "PivotStyleLight9",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight10",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight11",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight12",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight13",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight14",
        "- d 1 a/m a/m | - d 1 a/m a/m | a+0.8 d 1 a/m a+0.8/t | a+0.8 d 1 a/m a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.8/t a+0.8/t | - a-0.25 0 a+0.6/t a+0.6/t | - a-0.25 0 a+0.6/t a+0.6/t | - d 1 a/m a/m | - d 1 a/m a/m | - a-0.25 0 a+0.6/t a/m | - a-0.25 0 a+0.6/t a/m",
    ),
    (
        "PivotStyleLight15",
        "a+0.85 a 1 - a+0.65/t | a+0.85 a 1 - a+0.65/t | - a 1 a+0.65/t a+0.65/t | - a 1 a+0.65/t a+0.65/t | - a 0 - - | - a 0 - - | a+0.85 a 0 - - | a+0.85 a 0 - - | a+0.85 a 1 a+0.65/t - | a+0.85 a 1 a+0.65/t - | - a 0 - a+0.65/t | - a 0 - a+0.65/t",
    ),
    (
        "PivotStyleLight16",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight17",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight18",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight19",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight20",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight21",
        "a+0.8 d 1 - a+0.4/t | a+0.8 d 1 - a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 1 a+0.4/t a+0.4/t | - d 0 - - | - d 0 - - | d+0.85 d 0 - - | d+0.85 d 0 - - | a+0.8 d 1 a+0.4/t - | a+0.8 d 1 a+0.4/t - | - d 0 - a+0.4/t | - d 0 - a+0.4/t",
    ),
    (
        "PivotStyleLight22",
        "- a 1 a+0.5/t a+0.5/t | - a 1 a+0.5/t a+0.5/t | - a 1 a+0.5/t - | - a 0 a+0.5/t - | - a 1 - - | - a 0 - - | a+0.85 a 1 a+0.5/t a+0.5/t | a+0.85 a 0 a+0.5/t a+0.5/t | a+0.85 a 1 a+0.5/t a+0.5/t | a+0.85 a 1 a+0.5/t a+0.5/t | - a 1 a+0.5/t a+0.5/t | - a 0 a+0.5/t a+0.5/t",
    ),
    (
        "PivotStyleLight23",
        "- a-0.25 0 a/t a/t | - a-0.25 0 a/t a/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleLight24",
        "- a-0.25 0 a/t a/t | - a-0.25 0 a/t a/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleLight25",
        "- a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleLight26",
        "- a-0.25 0 a/t a/t | - a-0.25 0 a/t a/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleLight27",
        "- a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleLight28",
        "- a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t d+0.5/t | - a-0.25 0 a/t - | - a-0.25 0 a/t - | - a-0.25 0 - - | - a-0.25 0 - - | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 d+0.5/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | a+0.8 a-0.25 0 a/t d+0.5/t | - a-0.25 0 d+0.5/t a/t | - a-0.25 0 d+0.5/t a/t",
    ),
    (
        "PivotStyleMedium1",
        "a+0.5 l 0 - a+0.5/t | a+0.5 l 0 - a+0.5/t | a+0.65 l 0 a+0.85/t a+0.85/t | a+0.65 l 0 a+0.85/t a+0.85/t | - a 0 a+0.85/t a+0.85/t | - a 0 a+0.85/t a+0.85/t | - a 0 a+0.5/t a+0.5/t | - a 0 a+0.5/t a+0.5/t | - a 1 a+0.5/d a+0.5/t | - a 1 a+0.5/d a+0.5/t | - a 0 a+0.5/t a+0.5/d | - a 0 a+0.5/t a+0.5/d",
    ),
    (
        "PivotStyleMedium2",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium3",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium4",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium5",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium6",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium7",
        "a-0.25 l 0 - a-0.25/t | a-0.25 l 0 - a-0.25/t | a+0.4 l 0 a+0.8/t a+0.8/t | a+0.4 l 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a+0.8/t a+0.8/t | - d 0 a-0.25/t a-0.25/t | - d 0 a-0.25/t a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 1 a-0.25/d a-0.25/t | - d 0 a-0.25/t a-0.25/d | - d 0 a-0.25/t a-0.25/d",
    ),
    (
        "PivotStyleMedium8",
        "a+0.5 l 1 a+0.5/m a+0.75/t | a+0.5 l 1 a+0.5/m a+0.75/t | a+0.85 a 1 - - | a+0.85 a 1 - - | - a 0 - - | - a 0 - - | - a 0 a+0.75/t a+0.75/t | - a 0 a+0.75/t a+0.75/t | - a 1 a/t a+0.5/m | - a 1 a/t a+0.5/m | - a 0 a+0.75/t a/t | - a 0 a+0.75/t a/t",
    ),
    (
        "PivotStyleMedium9",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium10",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium11",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium12",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium13",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium14",
        "a l 1 a-0.25/m a+0.4/t | a l 1 a-0.25/m a+0.4/t | a+0.8 d 1 - - | a+0.8 d 1 - - | - d 0 - - | - d 0 - - | - d 0 a+0.4/t a+0.4/t | - d 0 a+0.4/t a+0.4/t | - d 1 a-0.25/t a-0.25/m | - d 1 a-0.25/t a-0.25/m | - d 0 a+0.4/t a-0.25/t | - d 0 a+0.4/t a-0.25/t",
    ),
    (
        "PivotStyleMedium15",
        "a l 0 - a+0.75/t | a l 0 - a+0.75/t | a+0.95 a 1 - - | a+0.95 a 1 - - | a+0.95 a 0 - - | a+0.95 a 0 - - | a+0.85 a 0 a+0.75/t a+0.75/t | a+0.85 a 0 a+0.75/t a+0.75/t | a l 0 a+0.75/t a+0.75/t | a l 0 a+0.75/t a+0.75/t | a+0.95 a 0 a+0.75/t a+0.75/t | a+0.95 a 0 a+0.75/t a+0.75/t",
    ),
    (
        "PivotStyleMedium16",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium17",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium18",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium19",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium20",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium21",
        "d l 0 - a+0.6/t | d l 0 - a+0.6/t | a+0.8 d 1 - - | a+0.8 d 1 - - | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | d l 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleMedium22",
        "a+0.75 a+0.5 1 - l/t | a+0.85 a+0.5 1 - l/t | a+0.75 a 1 l/t - | a+0.85 a 1 l/t - | a+0.75 a+0.5 1 - - | a+0.85 a+0.5 0 - - | a+0.75 a+0.5 1 - - | a+0.85 a+0.5 0 - - | a+0.75 a+0.5 1 - - | a+0.85 a+0.5 1 - - | a+0.75 a+0.5 1 - - | a+0.75 a+0.5 0 - -",
    ),
    (
        "PivotStyleMedium23",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleMedium24",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleMedium25",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleMedium26",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleMedium27",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleMedium28",
        "a+0.6 a-0.25 1 - l/t | a+0.8 a-0.25 1 - l/t | a+0.6 d 1 l/t - | a+0.8 d 1 l/t - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 0 - - | a+0.6 a-0.25 1 - - | a+0.8 a-0.25 1 - - | a+0.6 a-0.25 1 - - | a+0.6 a-0.25 0 - -",
    ),
    (
        "PivotStyleDark1",
        "a+0.5 l 1 - a+0.65/t | a+0.5 l 1 - a+0.65/t | a+0.85 a 1 a+0.65/t a+0.65/t | a+0.85 a 1 a+0.65/t a+0.65/t | a+0.75 a 0 a+0.85/t a+0.85/t | a+0.75 a 0 a+0.85/t a+0.85/t | a+0.75 a 0 a+0.85/t a+0.85/t | a+0.75 a 0 a+0.85/t a+0.85/t | a+0.5 l 1 a+0.85/t - | a+0.5 l 1 a+0.85/t - | a+0.65 a 0 a+0.85/t a+0.85/t | a+0.65 a 0 a+0.85/t a+0.85/t",
    ),
    (
        "PivotStyleDark2",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark3",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark4",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark5",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark6",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark7",
        "a-0.5 l 1 - a/t | a-0.5 l 1 - a/t | a+0.8 d 1 a/t a+0.4/t | a+0.8 d 1 a/t a+0.4/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a+0.6 d 0 a+0.8/t a+0.8/t | a-0.5 l 1 a+0.8/t - | a-0.5 l 1 a+0.8/t - | a+0.4 d 0 a+0.8/t a+0.8/t | a+0.4 d 0 a+0.8/t a+0.8/t",
    ),
    (
        "PivotStyleDark8",
        "a+0.25 l 1 a+0.5/m - | a+0.25 l 1 a+0.5/m - | a+0.75 a 1 - a+0.85/m | a+0.75 a 1 - a+0.85/m | a+0.85 a 0 - - | a+0.85 a 0 - - | a+0.85 a 0 a+0.75/t a+0.75/t | a+0.85 a 0 a+0.75/t a+0.75/t | a+0.25 l 1 a+0.75/t a+0.5/m | a+0.25 l 1 a+0.75/t a+0.5/m | a+0.85 a 0 a+0.75/t a+0.75/t | a+0.85 a 0 a+0.75/t a+0.75/t",
    ),
    (
        "PivotStyleDark9",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark10",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark11",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark12",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark13",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark14",
        "d+0.25 l 1 d+0.5/m - | d+0.25 l 1 d+0.5/m - | a+0.6 d 1 - a+0.8/m | a+0.6 d 1 - a+0.8/m | a+0.8 d 0 - - | a+0.8 d 0 - - | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t | d+0.25 l 1 a+0.6/t d+0.5/m | d+0.25 l 1 a+0.6/t d+0.5/m | a+0.8 d 0 a+0.6/t a+0.6/t | a+0.8 d 0 a+0.6/t a+0.6/t",
    ),
    (
        "PivotStyleDark15",
        "a l 1 - a+0.65/t | a l 1 - a+0.65/t | a+0.5 l 1 - a+0.8/t | a+0.5 l 1 - a+0.8/t | a+0.55 a+0.85 0 - - | a+0.55 a+0.85 0 - - | a+0.55 a+0.85 0 a+0.65/t a+0.65/t | a+0.55 a+0.85 0 a+0.65/t a+0.65/t | a l 1 a+0.65/t a+0.65/t | a l 1 a+0.65/t a+0.65/t | a+0.55 a+0.85 0 a+0.65/t a+0.65/t | a+0.55 a+0.85 0 a+0.65/t a+0.65/t",
    ),
    (
        "PivotStyleDark16",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark17",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark18",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark19",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark20",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark21",
        "d l 1 - a+0.4/t | d l 1 - a+0.4/t | a-0.25 l 1 - - | a-0.25 l 1 - - | a a+0.8 0 - - | a a+0.8 0 - - | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | d l 1 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t | a a+0.8 0 a+0.4/t a+0.4/t",
    ),
    (
        "PivotStyleDark22",
        "a+0.5 a+0.85 0 - l/m | a+0.5 a+0.85 0 - l/m | a+0.5 l 1 l/m - | a+0.5 l 1 l/m - | a+0.5 a+0.85 0 - - | a+0.5 a+0.85 0 - - | a+0.5 a+0.85 0 - - | a+0.5 a+0.85 0 - - | a+0.5 l 1 l/m - | a+0.5 l 1 l/m - | a+0.5 a+0.85 0 - l/m | a+0.55 a+0.85 0 - l/m",
    ),
    (
        "PivotStyleDark23",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
    (
        "PivotStyleDark24",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
    (
        "PivotStyleDark25",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
    (
        "PivotStyleDark26",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
    (
        "PivotStyleDark27",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
    (
        "PivotStyleDark28",
        "a-0.25 a+0.8 0 - l/m | a-0.25 a+0.8 0 - l/m | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 a+0.8 0 - - | a a+0.8 0 - - | a-0.25 l 1 l/m - | a l 1 l/m - | a-0.25 a+0.8 0 - l/m | a+0.4 a+0.8 0 - l/m",
    ),
];
