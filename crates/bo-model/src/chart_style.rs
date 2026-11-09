//! Chart formatting shared by BlueSheet, BlueSlide and BlueDoc charts (CORE-CHART-007).
//!
//! The formatting is what a DrawingML chart part stores as shape properties (`c:spPr`,
//! ECMA-376 Part 1, §21.2.2.197 → §20.1.2.2.35 CT_ShapeProperties) and text properties
//! (`c:txPr`, §21.2.2.216) on the chart space, plot area, axes, gridlines and series, plus the
//! data labels (§21.2.2.49 dLbls) and gridline switches of a quick layout. Writers emit these
//! elements directly; readers recover them from the same elements, so a formatted chart
//! survives a save / open cycle in every format.

use crate::sheet::ChartColor;
use serde::{Deserialize, Serialize};

/// How the series of filled plots (bars, areas, pies, bubbles) are painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SeriesFill {
    /// §20.1.8.54 solidFill of the series colour.
    #[default]
    Solid,
    /// §20.1.8.33 gradFill: a lighter tint of the series colour to the colour itself.
    Gradient,
    /// solidFill with §20.1.2.3.1 alpha (translucent).
    Soft,
    /// §20.1.8.44 noFill with an outline in the series colour.
    Outline,
}

/// §21.2.2.49 dLbls: what the data labels show (§21.2.2.189 showVal, §21.2.2.187 showPercent,
/// §21.2.2.184 showCatName, §21.2.2.188 showSerName) and §21.2.2.48 dLblPos.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DataLabels {
    /// Show the value.
    pub value: bool,
    /// Show the percentage (pies).
    pub percent: bool,
    /// Show the category name.
    pub category: bool,
    /// Show the series name.
    pub series: bool,
    /// §21.2.2.48 ST_DLblPos (`outEnd`, `inEnd`, `ctr`, `bestFit`, `t`, …); `None` = default.
    pub pos: Option<String>,
}

impl DataLabels {
    /// Shows anything at all.
    #[must_use]
    pub fn any(&self) -> bool {
        self.value || self.percent || self.category || self.series
    }
}

/// Formatting of a chart (style, colours and the parts of a quick layout that are not titles or
/// the legend). Every field defaults to "as the application draws it".
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChartFormat {
    /// Series colours in order (cycled); used for series without their own fill. Empty = the
    /// host's default colours.
    pub palette: Vec<ChartColor>,
    /// Series fill treatment.
    pub series_fill: SeriesFill,
    /// Series outline colour (§20.1.2.2.24 ln on the series spPr); `None` = no outline.
    pub outline: Option<ChartColor>,
    /// Series outline width in EMU (§20.1.2.2.24 `ln@w`); 0 = 9525 (0.75 pt).
    pub outline_w: u32,
    /// Line width of line / scatter / radar series in EMU; `None` = 28575 (2.25 pt).
    pub line_w: Option<u32>,
    /// Markers on line series (§21.2.2.106 marker / §21.2.2.174 symbol `circle`).
    pub markers: bool,
    /// Outer shadow on the series (§20.1.8.49 outerShdw in §20.1.8.25 effectLst).
    pub shadow: bool,
    /// §21.2.2.75 gapWidth of bar groups (percent); `None` = 150.
    pub gap_width: Option<u32>,
    /// Chart area fill (`c:chartSpace/c:spPr`, §21.2.2.29); `None` = white / page.
    pub chart_fill: Option<ChartColor>,
    /// Chart area border; `None` = the application's frame.
    pub chart_line: Option<ChartColor>,
    /// Plot area fill (`c:plotArea/c:spPr`, §21.2.2.145).
    pub plot_fill: Option<ChartColor>,
    /// Gridline colour (`c:majorGridlines/c:spPr/a:ln`, §21.2.2.100).
    pub grid_color: Option<ChartColor>,
    /// Gridline width in EMU; `None` = 9525.
    pub grid_w: Option<u32>,
    /// Axis line colour (`c:catAx/c:spPr/a:ln`).
    pub axis_line: Option<ChartColor>,
    /// Text colour of the chart (`c:chartSpace/c:txPr` §21.2.2.216, `a:defRPr` solidFill).
    pub text_color: Option<ChartColor>,
    /// Base font size in hundredths of a point (`a:defRPr@sz`); `None` = automatic.
    pub font_size: Option<u32>,
    /// Bold chart title (`a:defRPr@b` of the title's text).
    pub title_bold: bool,
    /// §21.2.2.156 roundedCorners of the chart area.
    pub rounded: bool,
    /// The value axis has no major gridlines (§21.2.2.100 majorGridlines absent).
    pub hide_val_grid: bool,
    /// The category axis has major gridlines.
    pub cat_grid: bool,
    /// Data labels of every series; `None` = no labels.
    pub data_labels: Option<DataLabels>,
}

impl ChartFormat {
    /// Nothing differs from the defaults.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == ChartFormat::default()
    }
}
