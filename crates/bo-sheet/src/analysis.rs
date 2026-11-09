//! Analysis tools for BlueSheet: descriptive statistics, correlation and covariance matrices,
//! ANOVA (single factor, two factor with and without replication), F-, t- and z-tests,
//! exponential smoothing, moving averages, histograms, rank and percentile, random numbers,
//! sampling, linear regression and Fourier analysis.
//!
//! Every tool is a pure function from input grids (the cell values of the input ranges) to an
//! output [`Table`] of labels and values that the engine writes into a sheet. The statistics are
//! the standard textbook ones; p-values and critical values come from the same kernels as the
//! worksheet functions (`T.DIST`, `F.DIST.RT`, `NORM.S.INV`, … — `bo_formula::stat`, ECMA-376
//! Part 1, §18.17.7), and regression reuses the `LINEST` least-squares fit.

use bo_formula::stat;
use serde::Deserialize;

/// One input cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Blank.
    Empty,
    /// A number.
    Num(f64),
    /// Text (labels, complex numbers for Fourier analysis).
    Text(String),
}

/// Input cells, row-major.
pub type Grid = Vec<Vec<Item>>;

/// An error value written into an output cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutError {
    /// `#N/A`: not available (no mode, no forecast yet).
    NA,
    /// `#DIV/0!`: undefined (too few values, zero variance).
    Div0,
    /// `#NUM!`: out of range (k-th largest beyond the count).
    Num,
}

/// One output cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// Left blank.
    Empty,
    /// A label.
    Text(String),
    /// A number.
    Num(f64),
    /// A fraction shown as a percentage (`0.00%`).
    Percent(f64),
    /// An error value.
    Error(OutError),
}

/// A tool's output: rows of cells (rows may differ in length).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    /// The rows, top to bottom.
    pub rows: Vec<Vec<Out>>,
}

impl Table {
    /// Number of columns (the longest row).
    #[must_use]
    pub fn width(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// The cell at `(row, col)` (`Out::Empty` beyond a short row).
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> &Out {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .unwrap_or(&Out::Empty)
    }

    fn push(&mut self, row: Vec<Out>) {
        self.rows.push(row);
    }

    fn blank(&mut self) {
        self.rows.push(Vec::new());
    }

    /// Places `other` to the right of this table, starting at column `col`.
    fn beside(&mut self, other: Table, col: usize) {
        for (i, row) in other.rows.into_iter().enumerate() {
            if self.rows.len() <= i {
                self.rows.resize(i + 1, Vec::new());
            }
            let r = &mut self.rows[i];
            if r.len() < col {
                r.resize(col, Out::Empty);
            }
            r.extend(row);
        }
    }
}

/// Why a tool could not run (the code is what the UI translates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisError {
    /// The input holds text where numbers are needed.
    NonNumeric,
    /// Not enough values (or groups) for the tool.
    TooFew,
    /// The inputs do not fit together (different lengths, missing second range, …).
    Shape,
    /// Fourier analysis needs 2ⁿ values (at most 4096).
    PowerOfTwo,
    /// A parameter is out of range.
    Param,
    /// The output would be too large.
    TooLarge,
}

impl AnalysisError {
    /// Stable code (`nonNumeric`, `tooFew`, `shape`, `powerOfTwo`, `param`, `tooLarge`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::NonNumeric => "nonNumeric",
            Self::TooFew => "tooFew",
            Self::Shape => "shape",
            Self::PowerOfTwo => "powerOfTwo",
            Self::Param => "param",
            Self::TooLarge => "tooLarge",
        }
    }
}

type Res<T> = Result<T, AnalysisError>;

/// Largest number of cells a random-number or sampling run may produce.
pub const MAX_OUTPUT_CELLS: usize = 1_048_576;

/// Longest input for Fourier analysis.
pub const MAX_FOURIER: usize = 4096;

fn default_alpha() -> f64 {
    0.05
}
fn default_confidence() -> f64 {
    0.95
}
fn default_damping() -> f64 {
    0.3
}
fn default_interval() -> usize {
    3
}
fn default_true() -> bool {
    true
}

/// Distribution for random-number generation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RandomDist {
    /// Uniform on `[lo, hi)`.
    Uniform {
        /// Lower bound.
        lo: f64,
        /// Upper bound.
        hi: f64,
    },
    /// Normal (Box–Muller).
    Normal {
        /// Mean.
        mean: f64,
        /// Standard deviation (≥ 0).
        sd: f64,
    },
    /// 1 with probability `p`, else 0.
    Bernoulli {
        /// Probability of a 1.
        p: f64,
    },
    /// Successes in `trials` Bernoulli trials.
    Binomial {
        /// Success probability.
        p: f64,
        /// Number of trials (≤ 10 000).
        trials: u32,
    },
    /// Poisson with mean `lambda`.
    Poisson {
        /// Mean (> 0).
        lambda: f64,
    },
    /// Values with probabilities from the input range: two columns, value then probability.
    Discrete,
}

/// How to sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SamplingMethod {
    /// Every `period`-th value, starting with the `period`-th.
    Periodic {
        /// Period (≥ 1).
        period: usize,
    },
    /// `count` values drawn at random with replacement.
    Random {
        /// Number of values.
        count: usize,
    },
}

/// An analysis tool with its options. `labels` means the first row (or column) of each input
/// holds names; `byRows` groups the input by rows instead of columns.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "tool",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Tool {
    /// Summary statistics of each series.
    Descriptive {
        /// Series are rows.
        #[serde(default)]
        by_rows: bool,
        /// First row / column holds names.
        #[serde(default)]
        labels: bool,
        /// Confidence level for the mean (e.g. 0.95).
        #[serde(default)]
        confidence: Option<f64>,
        /// Also report the k-th largest value.
        #[serde(default)]
        kth_largest: Option<usize>,
        /// Also report the k-th smallest value.
        #[serde(default)]
        kth_smallest: Option<usize>,
    },
    /// Pearson correlation matrix (lower triangle).
    Correlation {
        /// Series are rows.
        #[serde(default)]
        by_rows: bool,
        /// First row / column holds names.
        #[serde(default)]
        labels: bool,
    },
    /// Population covariance matrix (lower triangle).
    Covariance {
        /// Series are rows.
        #[serde(default)]
        by_rows: bool,
        /// First row / column holds names.
        #[serde(default)]
        labels: bool,
    },
    /// One-way analysis of variance.
    AnovaSingle {
        /// Groups are rows.
        #[serde(default)]
        by_rows: bool,
        /// First row / column holds names.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
    },
    /// Two-way analysis of variance with one observation per cell.
    AnovaTwoFactor {
        /// First row and column hold names.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
    },
    /// Two-way analysis of variance with replication; the input always has a label row (column
    /// names) and a label column (sample names, one per block of `rows_per_sample` rows).
    AnovaTwoFactorRep {
        /// Observations per sample and column.
        rows_per_sample: usize,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
    },
    /// F-test for the variances of two samples (second input = second sample).
    FTest {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
    },
    /// Paired two-sample t-test for means.
    TTestPaired {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
        /// Hypothesized mean difference.
        #[serde(default)]
        hypothesized: f64,
    },
    /// Two-sample t-test assuming equal variances (pooled).
    TTestEqual {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
        /// Hypothesized mean difference.
        #[serde(default)]
        hypothesized: f64,
    },
    /// Two-sample t-test assuming unequal variances (Welch; df rounded to a whole number).
    TTestUnequal {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
        /// Hypothesized mean difference.
        #[serde(default)]
        hypothesized: f64,
    },
    /// Two-sample z-test for means with known variances.
    ZTest {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Significance level.
        #[serde(default = "default_alpha")]
        alpha: f64,
        /// Hypothesized mean difference.
        #[serde(default)]
        hypothesized: f64,
        /// Known variance of the first sample.
        var_a: f64,
        /// Known variance of the second sample.
        var_b: f64,
    },
    /// Simple exponential smoothing, `F(t+1) = α·y(t) + (1−α)·F(t)` with `α = 1 − damping`.
    ExpSmoothing {
        /// First cell is a name.
        #[serde(default)]
        labels: bool,
        /// Damping factor `1 − α` (0…1).
        #[serde(default = "default_damping")]
        damping: f64,
        /// Also output standard errors.
        #[serde(default)]
        std_errors: bool,
    },
    /// Moving average over `interval` values.
    MovingAverage {
        /// First cell is a name.
        #[serde(default)]
        labels: bool,
        /// Number of values averaged (≥ 2).
        #[serde(default = "default_interval")]
        interval: usize,
        /// Also output standard errors.
        #[serde(default)]
        std_errors: bool,
    },
    /// Frequency table over bins (second input; automatic when absent).
    Histogram {
        /// First cell of each input is a name.
        #[serde(default)]
        labels: bool,
        /// Also output the table sorted by frequency.
        #[serde(default)]
        pareto: bool,
        /// Add cumulative percentages.
        #[serde(default)]
        cumulative: bool,
    },
    /// Ordinal and percentage rank of each value.
    RankPercentile {
        /// Series are rows.
        #[serde(default)]
        by_rows: bool,
        /// First row / column holds names.
        #[serde(default)]
        labels: bool,
    },
    /// Random numbers (seeded, reproducible).
    Random {
        /// Number of columns (variables).
        columns: usize,
        /// Numbers per column.
        count: usize,
        /// Seed; the same seed gives the same numbers.
        #[serde(default)]
        seed: Option<u64>,
        /// The distribution.
        dist: RandomDist,
    },
    /// Sample of the input values.
    Sampling {
        /// First cell is a name.
        #[serde(default)]
        labels: bool,
        /// Periodic or random.
        method: SamplingMethod,
        /// Seed for random sampling.
        #[serde(default)]
        seed: Option<u64>,
    },
    /// Linear least-squares regression of the first input (y, one column) on the second (x, one
    /// or more columns).
    Regression {
        /// First row holds names.
        #[serde(default)]
        labels: bool,
        /// Fit a constant (intercept).
        #[serde(default = "default_true")]
        constant: bool,
        /// Confidence level of the extra coefficient interval (95% is always shown).
        #[serde(default = "default_confidence")]
        confidence: f64,
        /// Output predicted values and residuals.
        #[serde(default)]
        residuals: bool,
        /// Output standardized residuals.
        #[serde(default)]
        std_residuals: bool,
    },
    /// Discrete Fourier transform (radix-2 FFT) of 2ⁿ real or complex values.
    Fourier {
        /// First cell is a name.
        #[serde(default)]
        labels: bool,
        /// Inverse transform.
        #[serde(default)]
        inverse: bool,
    },
}

impl Tool {
    /// Whether the tool reads a second input range (required unless noted in the tool).
    #[must_use]
    pub fn uses_second_input(&self) -> bool {
        matches!(
            self,
            Self::FTest { .. }
                | Self::TTestPaired { .. }
                | Self::TTestEqual { .. }
                | Self::TTestUnequal { .. }
                | Self::ZTest { .. }
                | Self::Histogram { .. }
                | Self::Regression { .. }
        )
    }

    /// Whether the tool reads an input range at all.
    #[must_use]
    pub fn uses_input(&self) -> bool {
        !matches!(
            self,
            Self::Random {
                dist: RandomDist::Uniform { .. }
                    | RandomDist::Normal { .. }
                    | RandomDist::Bernoulli { .. }
                    | RandomDist::Binomial { .. }
                    | RandomDist::Poisson { .. },
                ..
            }
        )
    }
}

/// Runs `tool` on `input` (and `second`, for the tools that take two ranges). `vi` selects
/// Vietnamese labels.
pub fn run(tool: &Tool, input: &Grid, second: Option<&Grid>, vi: bool) -> Res<Table> {
    let l = Labels { vi };
    match tool {
        Tool::Descriptive {
            by_rows,
            labels,
            confidence,
            kth_largest,
            kth_smallest,
        } => {
            let s = split(input, *by_rows, *labels, l)?;
            descriptive(&s, *confidence, *kth_largest, *kth_smallest, l)
        }
        Tool::Correlation { by_rows, labels } => {
            matrix(&split(input, *by_rows, *labels, l)?, false)
        }
        Tool::Covariance { by_rows, labels } => matrix(&split(input, *by_rows, *labels, l)?, true),
        Tool::AnovaSingle {
            by_rows,
            labels,
            alpha,
        } => anova_single(&split(input, *by_rows, *labels, l)?, *alpha, l),
        Tool::AnovaTwoFactor { labels, alpha } => anova_two(input, *labels, *alpha, l),
        Tool::AnovaTwoFactorRep {
            rows_per_sample,
            alpha,
        } => anova_two_rep(input, *rows_per_sample, *alpha, l),
        Tool::FTest { labels, alpha } => {
            let (a, b) = pair(input, second, *labels, l)?;
            f_test(&a, &b, *alpha, l)
        }
        Tool::TTestPaired {
            labels,
            alpha,
            hypothesized,
        } => {
            let (a, b) = pair(input, second, *labels, l)?;
            t_test(&a, &b, TKind::Paired, *alpha, *hypothesized, l)
        }
        Tool::TTestEqual {
            labels,
            alpha,
            hypothesized,
        } => {
            let (a, b) = pair(input, second, *labels, l)?;
            t_test(&a, &b, TKind::Equal, *alpha, *hypothesized, l)
        }
        Tool::TTestUnequal {
            labels,
            alpha,
            hypothesized,
        } => {
            let (a, b) = pair(input, second, *labels, l)?;
            t_test(&a, &b, TKind::Unequal, *alpha, *hypothesized, l)
        }
        Tool::ZTest {
            labels,
            alpha,
            hypothesized,
            var_a,
            var_b,
        } => {
            let (a, b) = pair(input, second, *labels, l)?;
            z_test(&a, &b, *alpha, *hypothesized, (*var_a, *var_b), l)
        }
        Tool::ExpSmoothing {
            labels,
            damping,
            std_errors,
        } => {
            let s = single(input, *labels, l)?;
            exp_smoothing(&s.v, *damping, *std_errors, *labels, l)
        }
        Tool::MovingAverage {
            labels,
            interval,
            std_errors,
        } => {
            let s = single(input, *labels, l)?;
            moving_average(&s.v, *interval, *std_errors, *labels, l)
        }
        Tool::Histogram {
            labels,
            pareto,
            cumulative,
        } => {
            let data = single(input, *labels, l)?;
            let bins = match second {
                Some(g) if !is_blank(g) => Some(single(g, *labels, l)?.v),
                _ => None,
            };
            Ok(histogram(&data.v, bins, *pareto, *cumulative, l))
        }
        Tool::RankPercentile { by_rows, labels } => {
            Ok(rank_percentile(&split(input, *by_rows, *labels, l)?, l))
        }
        Tool::Random {
            columns,
            count,
            seed,
            dist,
        } => random(*columns, *count, seed.unwrap_or(DEFAULT_SEED), dist, input),
        Tool::Sampling {
            labels,
            method,
            seed,
        } => {
            let s = single(input, *labels, l)?;
            sampling(&s.v, *method, seed.unwrap_or(DEFAULT_SEED))
        }
        Tool::Regression {
            labels,
            constant,
            confidence,
            residuals,
            std_residuals,
        } => regression(
            input,
            second.ok_or(AnalysisError::Shape)?,
            &RegOptions {
                labels: *labels,
                constant: *constant,
                confidence: *confidence,
                residuals: *residuals,
                std_residuals: *std_residuals,
            },
            l,
        ),
        Tool::Fourier { labels, inverse } => fourier(input, *labels, *inverse, l),
    }
}

// ------------------------------------------------------------------ labels

#[derive(Clone, Copy)]
struct Labels {
    vi: bool,
}

impl Labels {
    /// The label text for `key` (English, or Vietnamese with `vi`).
    fn t(self, key: &'static str) -> String {
        let (en, vi) = match key {
            "mean" => ("Mean", "Trung bình"),
            "stdErr" => ("Standard Error", "Sai số chuẩn"),
            "median" => ("Median", "Trung vị"),
            "mode" => ("Mode", "Yếu vị"),
            "stdDev" => ("Standard Deviation", "Độ lệch chuẩn"),
            "sampleVar" => ("Sample Variance", "Phương sai mẫu"),
            "kurtosis" => ("Kurtosis", "Độ nhọn"),
            "skewness" => ("Skewness", "Độ lệch"),
            "range" => ("Range", "Khoảng biến thiên"),
            "min" => ("Minimum", "Nhỏ nhất"),
            "max" => ("Maximum", "Lớn nhất"),
            "sum" => ("Sum", "Tổng"),
            "count" => ("Count", "Số lượng"),
            "largest" => ("Largest", "Lớn thứ"),
            "smallest" => ("Smallest", "Nhỏ thứ"),
            "confLevel" => ("Confidence Level", "Mức tin cậy"),
            "column" => ("Column", "Cột"),
            "row" => ("Row", "Hàng"),
            "anovaSingle" => ("Anova: Single Factor", "Phân tích phương sai một yếu tố"),
            "anovaTwo" => (
                "Anova: Two-Factor Without Replication",
                "Phân tích phương sai hai yếu tố không lặp",
            ),
            "anovaTwoRep" => (
                "Anova: Two-Factor With Replication",
                "Phân tích phương sai hai yếu tố có lặp",
            ),
            "summary" => ("SUMMARY", "TÓM TẮT"),
            "groups" => ("Groups", "Nhóm"),
            "average" => ("Average", "Trung bình"),
            "variance" => ("Variance", "Phương sai"),
            "anova" => ("ANOVA", "ANOVA"),
            "source" => ("Source of Variation", "Nguồn biến thiên"),
            "ss" => ("SS", "SS"),
            "df" => ("df", "df"),
            "ms" => ("MS", "MS"),
            "f" => ("F", "F"),
            "pValue" => ("P-value", "Giá trị P"),
            "fCrit" => ("F crit", "F tới hạn"),
            "between" => ("Between Groups", "Giữa các nhóm"),
            "within" => ("Within Groups", "Trong nhóm"),
            "total" => ("Total", "Tổng cộng"),
            "rows" => ("Rows", "Hàng"),
            "columns" => ("Columns", "Cột"),
            "error" => ("Error", "Sai số"),
            "sample" => ("Sample", "Mẫu"),
            "interaction" => ("Interaction", "Tương tác"),
            "withinRep" => ("Within", "Trong"),
            "fTest" => (
                "F-Test Two-Sample for Variances",
                "Kiểm định F hai mẫu cho phương sai",
            ),
            "observations" => ("Observations", "Số quan sát"),
            "pFOne" => ("P(F<=f) one-tail", "P(F<=f) một phía"),
            "fCritOne" => ("F Critical one-tail", "F tới hạn một phía"),
            "tPaired" => (
                "t-Test: Paired Two Sample for Means",
                "Kiểm định t: hai mẫu cặp đôi cho trung bình",
            ),
            "tEqual" => (
                "t-Test: Two-Sample Assuming Equal Variances",
                "Kiểm định t: hai mẫu giả định phương sai bằng nhau",
            ),
            "tUnequal" => (
                "t-Test: Two-Sample Assuming Unequal Variances",
                "Kiểm định t: hai mẫu giả định phương sai khác nhau",
            ),
            "pearson" => ("Pearson Correlation", "Hệ số tương quan Pearson"),
            "pooledVar" => ("Pooled Variance", "Phương sai gộp"),
            "hypDiff" => (
                "Hypothesized Mean Difference",
                "Chênh lệch trung bình giả thuyết",
            ),
            "tStat" => ("t Stat", "Thống kê t"),
            "pTOne" => ("P(T<=t) one-tail", "P(T<=t) một phía"),
            "tCritOne" => ("t Critical one-tail", "t tới hạn một phía"),
            "pTTwo" => ("P(T<=t) two-tail", "P(T<=t) hai phía"),
            "tCritTwo" => ("t Critical two-tail", "t tới hạn hai phía"),
            "zTest" => (
                "z-Test: Two Sample for Means",
                "Kiểm định z: hai mẫu cho trung bình",
            ),
            "knownVar" => ("Known Variance", "Phương sai đã biết"),
            "z" => ("z", "z"),
            "pZOne" => ("P(Z<=z) one-tail", "P(Z<=z) một phía"),
            "zCritOne" => ("z Critical one-tail", "z tới hạn một phía"),
            "pZTwo" => ("P(Z<=z) two-tail", "P(Z<=z) hai phía"),
            "zCritTwo" => ("z Critical two-tail", "z tới hạn hai phía"),
            "forecast" => ("Forecast", "Dự báo"),
            "stdErrors" => ("Standard Errors", "Sai số chuẩn"),
            "bin" => ("Bin", "Khoảng"),
            "frequency" => ("Frequency", "Tần số"),
            "cumulative" => ("Cumulative %", "% Tích lũy"),
            "more" => ("More", "Lớn hơn"),
            "point" => ("Point", "Vị trí"),
            "rank" => ("Rank", "Thứ hạng"),
            "percent" => ("Percent", "Phần trăm"),
            "summaryOutput" => ("SUMMARY OUTPUT", "KẾT QUẢ TÓM TẮT"),
            "regStats" => ("Regression Statistics", "Thống kê hồi quy"),
            "multipleR" => ("Multiple R", "Hệ số tương quan bội"),
            "rSquare" => ("R Square", "R bình phương"),
            "adjRSquare" => ("Adjusted R Square", "R bình phương hiệu chỉnh"),
            "regression" => ("Regression", "Hồi quy"),
            "residual" => ("Residual", "Phần dư"),
            "sigF" => ("Significance F", "Ý nghĩa F"),
            "coefficients" => ("Coefficients", "Hệ số"),
            "lower" => ("Lower", "Cận dưới"),
            "upper" => ("Upper", "Cận trên"),
            "intercept" => ("Intercept", "Hệ số chặn"),
            "xVariable" => ("X Variable", "Biến X"),
            "residualOutput" => ("RESIDUAL OUTPUT", "KẾT QUẢ PHẦN DƯ"),
            "observation" => ("Observation", "Quan sát"),
            "predicted" => ("Predicted", "Dự đoán"),
            "residuals" => ("Residuals", "Phần dư"),
            "stdResiduals" => ("Standard Residuals", "Phần dư chuẩn hóa"),
            "fourier" => ("Fourier Transform", "Biến đổi Fourier"),
            "inverseFourier" => ("Inverse Fourier Transform", "Biến đổi Fourier ngược"),
            _ => (key, key),
        };
        (if self.vi { vi } else { en }).to_owned()
    }

    fn cell(self, key: &'static str) -> Out {
        Out::Text(self.t(key))
    }

    /// `Lower 95%`-style label for a confidence level (a fraction).
    fn pct(self, key: &'static str, level: f64) -> Out {
        Out::Text(format!("{} {}%", self.t(key), fmt_pct(level)))
    }
}

/// `95` / `97.5` for 0.95 / 0.975.
fn fmt_pct(level: f64) -> String {
    let p = (level * 1000.0).round() / 10.0;
    if p.fract() == 0.0 {
        format!("{p:.0}")
    } else {
        format!("{p:.1}")
    }
}

/// A number as label text.
fn fmt_num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{x:.0}")
    } else {
        format!("{x}")
    }
}

// ------------------------------------------------------------------ input helpers

/// A named series of numbers.
#[derive(Debug, Clone)]
struct Series {
    name: String,
    v: Vec<f64>,
}

fn is_blank(g: &Grid) -> bool {
    g.iter().flatten().all(|c| matches!(c, Item::Empty))
}

fn label_of(c: &Item) -> String {
    match c {
        Item::Empty => String::new(),
        Item::Num(n) => fmt_num(*n),
        Item::Text(t) => t.clone(),
    }
}

/// Numbers of a sequence of cells (blanks skipped, text refused).
fn numbers<'a>(cells: impl Iterator<Item = &'a Item>) -> Res<Vec<f64>> {
    let mut v = Vec::new();
    for c in cells {
        match c {
            Item::Empty => {}
            Item::Num(n) => v.push(*n),
            Item::Text(t) if t.trim().is_empty() => {}
            Item::Text(_) => return Err(AnalysisError::NonNumeric),
        }
    }
    Ok(v)
}

/// The series of a grid: its columns (or rows), the first cell of each a name with `labels`.
fn split(g: &Grid, by_rows: bool, labels: bool, l: Labels) -> Res<Vec<Series>> {
    let rows = g.len();
    let cols = g.iter().map(Vec::len).max().unwrap_or(0);
    let empty = Item::Empty;
    let at = |r: usize, c: usize| g.get(r).and_then(|x| x.get(c)).unwrap_or(&empty);
    let (outer, inner) = if by_rows { (rows, cols) } else { (cols, rows) };
    let mut out = Vec::with_capacity(outer);
    for i in 0..outer {
        let cell = |j: usize| if by_rows { at(i, j) } else { at(j, i) };
        let start = usize::from(labels);
        let name = if labels {
            label_of(cell(0))
        } else {
            format!("{} {}", l.t(if by_rows { "row" } else { "column" }), i + 1)
        };
        let v = numbers((start..inner).map(cell))?;
        out.push(Series { name, v });
    }
    if out.is_empty() || out.iter().all(|s| s.v.is_empty()) {
        return Err(AnalysisError::TooFew);
    }
    Ok(out)
}

/// One series from all cells of a grid (row-major), the first a name with `labels`.
fn single(g: &Grid, labels: bool, l: Labels) -> Res<Series> {
    let mut cells = g.iter().flatten();
    let name = if labels {
        cells.next().map(label_of).unwrap_or_default()
    } else {
        format!("{} 1", l.t("column"))
    };
    let v = numbers(cells)?;
    if v.is_empty() {
        return Err(AnalysisError::TooFew);
    }
    Ok(Series { name, v })
}

/// The two samples of a two-sample test.
fn pair(a: &Grid, b: Option<&Grid>, labels: bool, l: Labels) -> Res<(Series, Series)> {
    let b = b.ok_or(AnalysisError::Shape)?;
    let mut sa = single(a, labels, l)?;
    let mut sb = single(b, labels, l)?;
    if !labels {
        sa.name = format!("{} 1", l.t("sample"));
        sb.name = format!("{} 2", l.t("sample"));
    }
    Ok((sa, sb))
}

// ------------------------------------------------------------------ basic statistics

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Sample variance (n − 1); NaN below two values.
fn var_s(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64
}

fn sum_sq_dev(v: &[f64]) -> f64 {
    let m = mean(v);
    v.iter().map(|x| (x - m) * (x - m)).sum()
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        f64::midpoint(s[n / 2 - 1], s[n / 2])
    }
}

/// The most frequent value occurring at least twice (the first such in data order on ties).
fn mode(v: &[f64]) -> Option<f64> {
    let mut best: Option<(f64, usize)> = None;
    for (i, x) in v.iter().enumerate() {
        if v[..i].contains(x) {
            continue;
        }
        let c = v.iter().filter(|y| *y == x).count();
        if c >= 2 && best.is_none_or(|(_, b)| c > b) {
            best = Some((*x, c));
        }
    }
    best.map(|(x, _)| x)
}

/// A number cell; non-finite values are `#DIV/0!`.
fn num(x: f64) -> Out {
    if x.is_finite() {
        Out::Num(x)
    } else {
        Out::Error(OutError::Div0)
    }
}

/// Right-tail p-value of |t| (one tail).
fn t_tail(t: f64, df: f64) -> f64 {
    1.0 - stat::t_cdf(t.abs(), df)
}

// ------------------------------------------------------------------ descriptive statistics

fn descriptive(
    series: &[Series],
    confidence: Option<f64>,
    kth_largest: Option<usize>,
    kth_smallest: Option<usize>,
    l: Labels,
) -> Res<Table> {
    if let Some(c) = confidence {
        if !(c > 0.0 && c < 1.0) {
            return Err(AnalysisError::Param);
        }
    }
    if kth_largest == Some(0) || kth_smallest == Some(0) {
        return Err(AnalysisError::Param);
    }
    let mut table = Table::default();
    for (i, s) in series.iter().enumerate() {
        let v = &s.v;
        let n = v.len() as f64;
        let mut t = Table::default();
        t.push(vec![Out::Text(s.name.clone())]);
        t.blank();
        if v.is_empty() {
            table.beside(t, i * 2);
            continue;
        }
        let m = mean(v);
        let var = var_s(v);
        let sd = var.sqrt();
        let min = v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // Sample kurtosis and skewness (the KURT / SKEW estimators).
        let z = |p: i32| v.iter().map(|x| ((x - m) / sd).powi(p)).sum::<f64>();
        let kurt = if n >= 4.0 && sd > 0.0 {
            n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0)) * z(4)
                - 3.0 * (n - 1.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0))
        } else {
            f64::NAN
        };
        let skew = if n >= 3.0 && sd > 0.0 {
            n / ((n - 1.0) * (n - 2.0)) * z(3)
        } else {
            f64::NAN
        };
        let row = |key: &'static str, x: Out| vec![l.cell(key), x];
        t.push(row("mean", num(m)));
        t.push(row("stdErr", num(sd / n.sqrt())));
        t.push(row("median", num(median(v))));
        t.push(row("mode", mode(v).map_or(Out::Error(OutError::NA), num)));
        t.push(row("stdDev", num(sd)));
        t.push(row("sampleVar", num(var)));
        t.push(row("kurtosis", num(kurt)));
        t.push(row("skewness", num(skew)));
        t.push(row("range", num(max - min)));
        t.push(row("min", num(min)));
        t.push(row("max", num(max)));
        t.push(row("sum", num(v.iter().sum())));
        t.push(row("count", num(n)));
        let mut sorted = v.clone();
        sorted.sort_by(f64::total_cmp);
        if let Some(k) = kth_largest {
            let x = (k <= sorted.len()).then(|| sorted[sorted.len() - k]);
            t.push(vec![
                Out::Text(format!("{}({k})", l.t("largest"))),
                x.map_or(Out::Error(OutError::Num), num),
            ]);
        }
        if let Some(k) = kth_smallest {
            let x = (k <= sorted.len()).then(|| sorted[k - 1]);
            t.push(vec![
                Out::Text(format!("{}({k})", l.t("smallest"))),
                x.map_or(Out::Error(OutError::Num), num),
            ]);
        }
        if let Some(c) = confidence {
            let half = stat::t_inv(1.0 - (1.0 - c) / 2.0, n - 1.0) * sd / n.sqrt();
            let p = (c * 1000.0).round() / 10.0;
            t.push(vec![
                Out::Text(format!("{}({p:.1}%)", l.t("confLevel"))),
                num(half),
            ]);
        }
        table.beside(t, i * 2);
    }
    Ok(table)
}

// ------------------------------------------------------------------ correlation / covariance

fn matrix(series: &[Series], covariance: bool) -> Res<Table> {
    let n = series[0].v.len();
    if series.iter().any(|s| s.v.len() != n) {
        return Err(AnalysisError::Shape);
    }
    if n < 1 || (!covariance && n < 2) {
        return Err(AnalysisError::TooFew);
    }
    let means: Vec<f64> = series.iter().map(|s| mean(&s.v)).collect();
    let cross = |i: usize, j: usize| -> f64 {
        series[i]
            .v
            .iter()
            .zip(&series[j].v)
            .map(|(a, b)| (a - means[i]) * (b - means[j]))
            .sum()
    };
    let mut t = Table::default();
    let mut head = vec![Out::Empty];
    head.extend(series.iter().map(|s| Out::Text(s.name.clone())));
    t.push(head);
    for (i, s) in series.iter().enumerate() {
        let mut row = vec![Out::Text(s.name.clone())];
        for j in 0..=i {
            let x = if covariance {
                // Population covariance, as the Covariance tool reports it.
                cross(i, j) / n as f64
            } else if i == j {
                1.0
            } else {
                cross(i, j) / (cross(i, i) * cross(j, j)).sqrt()
            };
            row.push(num(x));
        }
        t.push(row);
    }
    Ok(t)
}

// ------------------------------------------------------------------ ANOVA

fn check_alpha(alpha: f64) -> Res<()> {
    if alpha > 0.0 && alpha < 1.0 {
        Ok(())
    } else {
        Err(AnalysisError::Param)
    }
}

/// `[label, SS, df, MS, F, P-value, F crit]` of one source of variation.
fn source_row(label: Out, ss: f64, df: f64, test: Option<(f64, f64, f64)>) -> Vec<Out> {
    let mut row = vec![label, num(ss), num(df), num(ss / df)];
    if let Some((ms_err, df_err, alpha)) = test {
        let f = (ss / df) / ms_err;
        row.push(num(f));
        row.push(num(stat::f_rt(f, df, df_err)));
        row.push(num(stat::f_inv_rt(alpha, df, df_err)));
    }
    row
}

fn anova_header(l: Labels) -> Vec<Out> {
    ["source", "ss", "df", "ms", "f", "pValue", "fCrit"]
        .into_iter()
        .map(|k| l.cell(k))
        .collect()
}

/// `[name, Count, Sum, Average, Variance]`.
fn summary_row(name: Out, v: &[f64]) -> Vec<Out> {
    vec![
        name,
        num(v.len() as f64),
        num(v.iter().sum()),
        num(mean(v)),
        num(var_s(v)),
    ]
}

fn anova_single(groups: &[Series], alpha: f64, l: Labels) -> Res<Table> {
    check_alpha(alpha)?;
    let groups: Vec<&Series> = groups.iter().filter(|g| !g.v.is_empty()).collect();
    let k = groups.len();
    let all: Vec<f64> = groups.iter().flat_map(|g| g.v.iter().copied()).collect();
    let n = all.len();
    if k < 2 || n <= k {
        return Err(AnalysisError::TooFew);
    }
    let grand = mean(&all);
    let ss_b: f64 = groups
        .iter()
        .map(|g| g.v.len() as f64 * (mean(&g.v) - grand).powi(2))
        .sum();
    let ss_w: f64 = groups.iter().map(|g| sum_sq_dev(&g.v)).sum();
    let (df_b, df_w) = ((k - 1) as f64, (n - k) as f64);
    let mut t = Table::default();
    t.push(vec![l.cell("anovaSingle")]);
    t.blank();
    t.push(vec![l.cell("summary")]);
    t.push(
        ["groups", "count", "sum", "average", "variance"]
            .into_iter()
            .map(|k| l.cell(k))
            .collect(),
    );
    for g in &groups {
        t.push(summary_row(Out::Text(g.name.clone()), &g.v));
    }
    t.blank();
    t.blank();
    t.push(vec![l.cell("anova")]);
    t.push(anova_header(l));
    t.push(source_row(
        l.cell("between"),
        ss_b,
        df_b,
        Some((ss_w / df_w, df_w, alpha)),
    ));
    t.push(source_row(l.cell("within"), ss_w, df_w, None));
    t.blank();
    t.push(vec![l.cell("total"), num(ss_b + ss_w), num(df_b + df_w)]);
    Ok(t)
}

/// A full numeric matrix (no blanks) of `g` without its label row and column.
fn full_matrix(g: &Grid, skip_row: bool, skip_col: bool) -> Res<Vec<Vec<f64>>> {
    let cols = g.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::new();
    for row in g.iter().skip(usize::from(skip_row)) {
        let mut r = Vec::with_capacity(cols);
        for c in usize::from(skip_col)..cols {
            match row.get(c) {
                Some(Item::Num(n)) => r.push(*n),
                _ => return Err(AnalysisError::NonNumeric),
            }
        }
        out.push(r);
    }
    Ok(out)
}

fn anova_two(g: &Grid, labels: bool, alpha: f64, l: Labels) -> Res<Table> {
    check_alpha(alpha)?;
    let m = full_matrix(g, labels, labels)?;
    let r = m.len();
    let c = m.first().map_or(0, Vec::len);
    if r < 2 || c < 2 {
        return Err(AnalysisError::TooFew);
    }
    let empty = Item::Empty;
    let at = |i: usize, j: usize| g.get(i).and_then(|x| x.get(j)).unwrap_or(&empty);
    let row_name = |i: usize| {
        if labels {
            label_of(at(i + 1, 0))
        } else {
            format!("{} {}", l.t("row"), i + 1)
        }
    };
    let col_name = |j: usize| {
        if labels {
            label_of(at(0, j + 1))
        } else {
            format!("{} {}", l.t("column"), j + 1)
        }
    };
    let cols: Vec<Vec<f64>> = (0..c)
        .map(|j| m.iter().map(|row| row[j]).collect())
        .collect();
    let all: Vec<f64> = m.iter().flatten().copied().collect();
    let grand = mean(&all);
    let ss_rows: f64 = m
        .iter()
        .map(|row| c as f64 * (mean(row) - grand).powi(2))
        .sum();
    let ss_cols: f64 = cols
        .iter()
        .map(|col| r as f64 * (mean(col) - grand).powi(2))
        .sum();
    let ss_tot = sum_sq_dev(&all);
    let ss_err = ss_tot - ss_rows - ss_cols;
    let (df_r, df_c) = ((r - 1) as f64, (c - 1) as f64);
    let df_e = df_r * df_c;
    let ms_e = ss_err / df_e;
    let mut t = Table::default();
    t.push(vec![l.cell("anovaTwo")]);
    t.blank();
    t.push(
        ["summary", "count", "sum", "average", "variance"]
            .into_iter()
            .map(|k| l.cell(k))
            .collect(),
    );
    for (i, row) in m.iter().enumerate() {
        t.push(summary_row(Out::Text(row_name(i)), row));
    }
    t.blank();
    for (j, col) in cols.iter().enumerate() {
        t.push(summary_row(Out::Text(col_name(j)), col));
    }
    t.blank();
    t.blank();
    t.push(vec![l.cell("anova")]);
    t.push(anova_header(l));
    t.push(source_row(
        l.cell("rows"),
        ss_rows,
        df_r,
        Some((ms_e, df_e, alpha)),
    ));
    t.push(source_row(
        l.cell("columns"),
        ss_cols,
        df_c,
        Some((ms_e, df_e, alpha)),
    ));
    t.push(source_row(l.cell("error"), ss_err, df_e, None));
    t.blank();
    t.push(vec![l.cell("total"), num(ss_tot), num((r * c - 1) as f64)]);
    Ok(t)
}

/// A statistic of a group of values.
type StatFn = fn(&[f64]) -> f64;

fn anova_two_rep(g: &Grid, per: usize, alpha: f64, l: Labels) -> Res<Table> {
    check_alpha(alpha)?;
    if per < 2 {
        return Err(AnalysisError::Param);
    }
    let m = full_matrix(g, true, true)?;
    let c = m.first().map_or(0, Vec::len);
    if m.len() % per != 0 {
        return Err(AnalysisError::Shape);
    }
    let r = m.len() / per;
    if r < 2 || c < 2 {
        return Err(AnalysisError::TooFew);
    }
    let empty = Item::Empty;
    let at = |i: usize, j: usize| g.get(i).and_then(|x| x.get(j)).unwrap_or(&empty);
    let col_name = |j: usize| label_of(at(0, j + 1));
    let sample_name = |i: usize| {
        let s = label_of(at(1 + i * per, 0));
        if s.is_empty() {
            format!("{} {}", l.t("sample"), i + 1)
        } else {
            s
        }
    };
    // cell(i, j) = observations of sample i in column j.
    let cell = |i: usize, j: usize| -> Vec<f64> { (0..per).map(|k| m[i * per + k][j]).collect() };
    let all: Vec<f64> = m.iter().flatten().copied().collect();
    let grand = mean(&all);
    let sample_vals = |i: usize| -> Vec<f64> { (0..c).flat_map(|j| cell(i, j)).collect() };
    let col_vals = |j: usize| -> Vec<f64> { (0..r).flat_map(|i| cell(i, j)).collect() };
    let ss_sample: f64 = (0..r)
        .map(|i| (c * per) as f64 * (mean(&sample_vals(i)) - grand).powi(2))
        .sum();
    let ss_cols: f64 = (0..c)
        .map(|j| (r * per) as f64 * (mean(&col_vals(j)) - grand).powi(2))
        .sum();
    let ss_cells: f64 = (0..r)
        .flat_map(|i| (0..c).map(move |j| (i, j)))
        .map(|(i, j)| per as f64 * (mean(&cell(i, j)) - grand).powi(2))
        .sum();
    let ss_within: f64 = (0..r)
        .flat_map(|i| (0..c).map(move |j| (i, j)))
        .map(|(i, j)| sum_sq_dev(&cell(i, j)))
        .sum();
    let ss_inter = ss_cells - ss_sample - ss_cols;
    let (df_s, df_c) = ((r - 1) as f64, (c - 1) as f64);
    let df_i = df_s * df_c;
    let df_w = (r * c * (per - 1)) as f64;
    let ms_w = ss_within / df_w;

    let mut t = Table::default();
    t.push(vec![l.cell("anovaTwoRep")]);
    t.blank();
    let mut head = vec![l.cell("summary")];
    head.extend((0..c).map(|j| Out::Text(col_name(j))));
    head.push(l.cell("total"));
    t.push(head);
    let block = |t: &mut Table, groups: Vec<Vec<f64>>| {
        let total: Vec<f64> = groups.iter().flatten().copied().collect();
        let mut gs = groups;
        gs.push(total);
        let stats: [(&'static str, StatFn); 4] = [
            ("count", |v| v.len() as f64),
            ("sum", |v| v.iter().sum()),
            ("average", mean),
            ("variance", var_s),
        ];
        for (key, f) in stats {
            let mut row = vec![l.cell(key)];
            row.extend(gs.iter().map(|v| num(f(v))));
            t.push(row);
        }
    };
    for i in 0..r {
        t.push(vec![Out::Text(sample_name(i))]);
        block(&mut t, (0..c).map(|j| cell(i, j)).collect());
        t.blank();
    }
    t.push(vec![l.cell("total")]);
    {
        // The total block has no "Total" column.
        let mut sub = Table::default();
        block(&mut sub, (0..c).map(col_vals).collect());
        for mut row in sub.rows {
            row.pop();
            t.push(row);
        }
    }
    t.blank();
    t.blank();
    t.push(vec![l.cell("anova")]);
    t.push(anova_header(l));
    let test = Some((ms_w, df_w, alpha));
    t.push(source_row(l.cell("sample"), ss_sample, df_s, test));
    t.push(source_row(l.cell("columns"), ss_cols, df_c, test));
    t.push(source_row(l.cell("interaction"), ss_inter, df_i, test));
    t.push(source_row(l.cell("withinRep"), ss_within, df_w, None));
    t.blank();
    t.push(vec![
        l.cell("total"),
        num(sum_sq_dev(&all)),
        num((all.len() - 1) as f64),
    ]);
    Ok(t)
}

// ------------------------------------------------------------------ F, t and z tests

fn two_head(title: &'static str, a: &Series, b: &Series, l: Labels) -> Table {
    let mut t = Table::default();
    t.push(vec![l.cell(title)]);
    t.blank();
    t.push(vec![
        Out::Empty,
        Out::Text(a.name.clone()),
        Out::Text(b.name.clone()),
    ]);
    t
}

fn f_test(a: &Series, b: &Series, alpha: f64, l: Labels) -> Res<Table> {
    check_alpha(alpha)?;
    if a.v.len() < 2 || b.v.len() < 2 {
        return Err(AnalysisError::TooFew);
    }
    let (va, vb) = (var_s(&a.v), var_s(&b.v));
    let (da, db) = ((a.v.len() - 1) as f64, (b.v.len() - 1) as f64);
    let f = va / vb;
    // The tail on the side of the observed ratio: lower when F < 1, upper otherwise.
    let (p, crit) = if f < 1.0 {
        (stat::f_cdf(f, da, db), stat::f_inv(alpha, da, db))
    } else {
        (stat::f_rt(f, da, db), stat::f_inv_rt(alpha, da, db))
    };
    let mut t = two_head("fTest", a, b, l);
    t.push(vec![l.cell("mean"), num(mean(&a.v)), num(mean(&b.v))]);
    t.push(vec![l.cell("variance"), num(va), num(vb)]);
    t.push(vec![
        l.cell("observations"),
        num(a.v.len() as f64),
        num(b.v.len() as f64),
    ]);
    t.push(vec![l.cell("df"), num(da), num(db)]);
    t.push(vec![l.cell("f"), num(f)]);
    t.push(vec![l.cell("pFOne"), num(p)]);
    t.push(vec![l.cell("fCritOne"), num(crit)]);
    Ok(t)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TKind {
    Paired,
    Equal,
    Unequal,
}

fn t_test(a: &Series, b: &Series, kind: TKind, alpha: f64, hyp: f64, l: Labels) -> Res<Table> {
    check_alpha(alpha)?;
    let (na, nb) = (a.v.len() as f64, b.v.len() as f64);
    if na < 2.0 || nb < 2.0 {
        return Err(AnalysisError::TooFew);
    }
    let (ma, mb) = (mean(&a.v), mean(&b.v));
    let (va, vb) = (var_s(&a.v), var_s(&b.v));
    let title = match kind {
        TKind::Paired => "tPaired",
        TKind::Equal => "tEqual",
        TKind::Unequal => "tUnequal",
    };
    let mut t = two_head(title, a, b, l);
    t.push(vec![l.cell("mean"), num(ma), num(mb)]);
    t.push(vec![l.cell("variance"), num(va), num(vb)]);
    t.push(vec![l.cell("observations"), num(na), num(nb)]);
    let (stat_t, df) = match kind {
        TKind::Paired => {
            if a.v.len() != b.v.len() {
                return Err(AnalysisError::Shape);
            }
            let d: Vec<f64> = a.v.iter().zip(&b.v).map(|(x, y)| x - y).collect();
            let sab: f64 = a.v.iter().zip(&b.v).map(|(x, y)| (x - ma) * (y - mb)).sum();
            let r = sab / (sum_sq_dev(&a.v) * sum_sq_dev(&b.v)).sqrt();
            t.push(vec![l.cell("pearson"), num(r)]);
            ((mean(&d) - hyp) / (var_s(&d) / na).sqrt(), na - 1.0)
        }
        TKind::Equal => {
            let pooled = ((na - 1.0) * va + (nb - 1.0) * vb) / (na + nb - 2.0);
            t.push(vec![l.cell("pooledVar"), num(pooled)]);
            (
                (ma - mb - hyp) / (pooled * (1.0 / na + 1.0 / nb)).sqrt(),
                na + nb - 2.0,
            )
        }
        TKind::Unequal => {
            let (qa, qb) = (va / na, vb / nb);
            // Welch–Satterthwaite degrees of freedom, rounded to a whole number.
            let df = ((qa + qb).powi(2) / (qa * qa / (na - 1.0) + qb * qb / (nb - 1.0))).round();
            ((ma - mb - hyp) / (qa + qb).sqrt(), df)
        }
    };
    t.push(vec![l.cell("hypDiff"), num(hyp)]);
    t.push(vec![l.cell("df"), num(df)]);
    t.push(vec![l.cell("tStat"), num(stat_t)]);
    let p = t_tail(stat_t, df);
    t.push(vec![l.cell("pTOne"), num(p)]);
    t.push(vec![l.cell("tCritOne"), num(stat::t_inv(1.0 - alpha, df))]);
    t.push(vec![l.cell("pTTwo"), num(2.0 * p)]);
    t.push(vec![
        l.cell("tCritTwo"),
        num(stat::t_inv(1.0 - alpha / 2.0, df)),
    ]);
    Ok(t)
}

fn z_test(
    a: &Series,
    b: &Series,
    alpha: f64,
    hyp: f64,
    (var_a, var_b): (f64, f64),
    l: Labels,
) -> Res<Table> {
    check_alpha(alpha)?;
    if !(var_a > 0.0 && var_b > 0.0) {
        return Err(AnalysisError::Param);
    }
    let (na, nb) = (a.v.len() as f64, b.v.len() as f64);
    let (ma, mb) = (mean(&a.v), mean(&b.v));
    let z = (ma - mb - hyp) / (var_a / na + var_b / nb).sqrt();
    let p = 1.0 - stat::norm_s_cdf(z.abs());
    let mut t = two_head("zTest", a, b, l);
    t.push(vec![l.cell("mean"), num(ma), num(mb)]);
    t.push(vec![l.cell("knownVar"), num(var_a), num(var_b)]);
    t.push(vec![l.cell("observations"), num(na), num(nb)]);
    t.push(vec![l.cell("hypDiff"), num(hyp)]);
    t.push(vec![l.cell("z"), num(z)]);
    t.push(vec![l.cell("pZOne"), num(p)]);
    t.push(vec![l.cell("zCritOne"), num(stat::norm_s_inv(1.0 - alpha))]);
    t.push(vec![l.cell("pZTwo"), num(2.0 * p)]);
    t.push(vec![
        l.cell("zCritTwo"),
        num(stat::norm_s_inv(1.0 - alpha / 2.0)),
    ]);
    Ok(t)
}

// ------------------------------------------------------------------ smoothing

/// `sqrt(Σ (actual − forecast)² / w)` over the `w` rows ending at `i` (the tools' standard
/// error), `#N/A` until `w` forecasts exist.
fn std_errors(y: &[f64], f: &[Option<f64>], w: usize) -> Vec<Out> {
    (0..y.len())
        .map(|i| {
            if i + 1 < w {
                return Out::Error(OutError::NA);
            }
            let window = i + 1 - w..=i;
            let mut s = 0.0;
            for k in window {
                match f[k] {
                    Some(fk) => s += (y[k] - fk).powi(2),
                    None => return Out::Error(OutError::NA),
                }
            }
            num((s / w as f64).sqrt())
        })
        .collect()
}

fn forecast_table(
    y: &[f64],
    f: &[Option<f64>],
    se: Option<usize>,
    labels: bool,
    l: Labels,
) -> Table {
    let errs = se.map(|w| std_errors(y, f, w));
    let mut t = Table::default();
    if labels {
        let mut head = vec![l.cell("forecast")];
        if errs.is_some() {
            head.push(l.cell("stdErrors"));
        }
        t.push(head);
    }
    for (i, fi) in f.iter().enumerate() {
        let mut row = vec![fi.map_or(Out::Error(OutError::NA), num)];
        if let Some(e) = &errs {
            row.push(e[i].clone());
        }
        t.push(row);
    }
    t
}

fn exp_smoothing(y: &[f64], damping: f64, se: bool, labels: bool, l: Labels) -> Res<Table> {
    if !(0.0..=1.0).contains(&damping) {
        return Err(AnalysisError::Param);
    }
    let alpha = 1.0 - damping;
    // F(1) is undefined, F(2) = y(1), F(t+1) = α·y(t) + (1 − α)·F(t).
    let mut f: Vec<Option<f64>> = vec![None; y.len()];
    for t in 1..y.len() {
        f[t] = Some(match f[t - 1] {
            None => y[t - 1],
            Some(prev) => alpha * y[t - 1] + damping * prev,
        });
    }
    Ok(forecast_table(y, &f, se.then_some(3), labels, l))
}

fn moving_average(y: &[f64], w: usize, se: bool, labels: bool, l: Labels) -> Res<Table> {
    if w < 2 || w > y.len() {
        return Err(AnalysisError::Param);
    }
    let f: Vec<Option<f64>> = (0..y.len())
        .map(|i| (i + 1 >= w).then(|| mean(&y[i + 1 - w..=i])))
        .collect();
    Ok(forecast_table(y, &f, se.then_some(w), labels, l))
}

// ------------------------------------------------------------------ histogram, rank

fn histogram(
    data: &[f64],
    bins: Option<Vec<f64>>,
    pareto: bool,
    cumulative: bool,
    l: Labels,
) -> Table {
    let mut bins = bins.unwrap_or_else(|| {
        // ⌈√n⌉ equal-width bins from the minimum to the maximum.
        let min = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let k = (data.len() as f64).sqrt().ceil().max(1.0) as usize;
        let w = (max - min) / k as f64;
        (1..=k)
            .map(|i| if i == k { max } else { min + w * i as f64 })
            .collect()
    });
    bins.sort_by(f64::total_cmp);
    bins.dedup();
    // freq[i] counts bins[i-1] < x ≤ bins[i]; the last slot is "More".
    let mut freq = vec![0usize; bins.len() + 1];
    for x in data {
        let i = bins.partition_point(|b| b < x);
        freq[i] += 1;
    }
    let n = data.len() as f64;
    let names: Vec<Out> = bins
        .iter()
        .map(|b| num(*b))
        .chain(std::iter::once(l.cell("more")))
        .collect();
    let build = |order: &[usize]| -> Table {
        let mut t = Table::default();
        let mut head = vec![l.cell("bin"), l.cell("frequency")];
        if cumulative {
            head.push(l.cell("cumulative"));
        }
        t.push(head);
        let mut acc = 0usize;
        for &i in order {
            acc += freq[i];
            let mut row = vec![names[i].clone(), num(freq[i] as f64)];
            if cumulative {
                row.push(Out::Percent(acc as f64 / n));
            }
            t.push(row);
        }
        t
    };
    let order: Vec<usize> = (0..freq.len()).collect();
    let mut t = build(&order);
    if pareto {
        let mut sorted = order.clone();
        // Stable: equal frequencies keep the bin order.
        sorted.sort_by(|a, b| freq[*b].cmp(&freq[*a]));
        let w = t.width();
        t.beside(build(&sorted), w);
    }
    t
}

fn rank_percentile(series: &[Series], l: Labels) -> Table {
    let mut table = Table::default();
    for (s_i, s) in series.iter().enumerate() {
        let n = s.v.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| s.v[*b].total_cmp(&s.v[*a]));
        let mut t = Table::default();
        t.push(vec![
            l.cell("point"),
            Out::Text(s.name.clone()),
            l.cell("rank"),
            l.cell("percent"),
        ]);
        for &i in &order {
            let x = s.v[i];
            let above = s.v.iter().filter(|y| **y > x).count();
            let below = s.v.iter().filter(|y| **y < x).count();
            // Percent rank inclusive: values below / (n − 1) (PERCENTRANK.INC).
            let pct = if n > 1 {
                below as f64 / (n - 1) as f64
            } else {
                1.0
            };
            t.push(vec![
                num((i + 1) as f64),
                num(x),
                num((above + 1) as f64),
                Out::Percent(pct),
            ]);
        }
        table.beside(t, s_i * 5);
    }
    table
}

// ------------------------------------------------------------------ random numbers, sampling

const DEFAULT_SEED: u64 = 0x2545_F491_4F6C_DD1D;

/// SplitMix64 (Steele, Lea & Flood 2014): a small, well-mixed 64-bit generator.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)` with 53 random bits.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal (Box–Muller).
    fn normal(&mut self) -> f64 {
        let u1 = 1.0 - self.unit(); // (0, 1]
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.unit() * n as f64) as usize).min(n - 1)
    }
}

fn poisson(rng: &mut Rng, lambda: f64) -> f64 {
    if lambda > 700.0 {
        // e^−λ underflows: normal approximation.
        return (lambda + lambda.sqrt() * rng.normal()).round().max(0.0);
    }
    // Inversion: walk the CDF with p(k) = p(k−1)·λ/k.
    let u = rng.unit();
    let mut p = (-lambda).exp();
    let mut s = p;
    let mut k = 0.0;
    while u > s && k < 10_000.0 {
        k += 1.0;
        p *= lambda / k;
        s += p;
    }
    k
}

fn random(columns: usize, count: usize, seed: u64, dist: &RandomDist, input: &Grid) -> Res<Table> {
    if columns == 0 || count == 0 {
        return Err(AnalysisError::Param);
    }
    if columns.saturating_mul(count) > MAX_OUTPUT_CELLS {
        return Err(AnalysisError::TooLarge);
    }
    let discrete: Vec<(f64, f64)> = if matches!(dist, RandomDist::Discrete) {
        let m = full_matrix(input, false, false)?;
        if m.first().map_or(0, Vec::len) != 2 {
            return Err(AnalysisError::Shape);
        }
        let pairs: Vec<(f64, f64)> = m.iter().map(|r| (r[0], r[1])).collect();
        let total: f64 = pairs.iter().map(|p| p.1).sum();
        if pairs.iter().any(|p| p.1 < 0.0) || (total - 1.0).abs() > 1e-9 {
            return Err(AnalysisError::Param);
        }
        pairs
    } else {
        Vec::new()
    };
    match *dist {
        RandomDist::Uniform { lo, hi } if !(lo.is_finite() && hi.is_finite() && lo <= hi) => {
            return Err(AnalysisError::Param)
        }
        RandomDist::Normal { sd, .. } if sd.is_nan() || sd < 0.0 => {
            return Err(AnalysisError::Param)
        }
        RandomDist::Bernoulli { p } | RandomDist::Binomial { p, .. }
            if !(0.0..=1.0).contains(&p) =>
        {
            return Err(AnalysisError::Param)
        }
        RandomDist::Binomial { trials, .. } if trials > 10_000 => return Err(AnalysisError::Param),
        RandomDist::Poisson { lambda } if !(lambda > 0.0 && lambda.is_finite()) => {
            return Err(AnalysisError::Param)
        }
        _ => {}
    }
    let mut rng = Rng(seed);
    let mut t = Table::default();
    for _ in 0..count {
        let mut row = Vec::with_capacity(columns);
        for _ in 0..columns {
            let x = match *dist {
                RandomDist::Uniform { lo, hi } => lo + rng.unit() * (hi - lo),
                RandomDist::Normal { mean, sd } => mean + sd * rng.normal(),
                RandomDist::Bernoulli { p } => f64::from(u8::from(rng.unit() < p)),
                RandomDist::Binomial { p, trials } => {
                    (0..trials).filter(|_| rng.unit() < p).count() as f64
                }
                RandomDist::Poisson { lambda } => poisson(&mut rng, lambda),
                RandomDist::Discrete => {
                    let u = rng.unit();
                    let mut acc = 0.0;
                    let mut pick = discrete.last().map_or(0.0, |p| p.0);
                    for (v, p) in &discrete {
                        acc += p;
                        if u < acc {
                            pick = *v;
                            break;
                        }
                    }
                    pick
                }
            };
            row.push(Out::Num(x));
        }
        t.push(row);
    }
    Ok(t)
}

fn sampling(v: &[f64], method: SamplingMethod, seed: u64) -> Res<Table> {
    let out: Vec<f64> = match method {
        SamplingMethod::Periodic { period } => {
            if period == 0 || period > v.len() {
                return Err(AnalysisError::Param);
            }
            v.iter().skip(period - 1).step_by(period).copied().collect()
        }
        SamplingMethod::Random { count } => {
            if count == 0 {
                return Err(AnalysisError::Param);
            }
            if count > MAX_OUTPUT_CELLS {
                return Err(AnalysisError::TooLarge);
            }
            let mut rng = Rng(seed);
            (0..count).map(|_| v[rng.below(v.len())]).collect()
        }
    };
    Ok(Table {
        rows: out.into_iter().map(|x| vec![Out::Num(x)]).collect(),
    })
}

// ------------------------------------------------------------------ regression

struct RegOptions {
    labels: bool,
    constant: bool,
    confidence: f64,
    residuals: bool,
    std_residuals: bool,
}

fn regression(y_in: &Grid, x_in: &Grid, o: &RegOptions, l: Labels) -> Res<Table> {
    if !(o.confidence > 0.0 && o.confidence < 1.0) {
        return Err(AnalysisError::Param);
    }
    let ym = full_matrix(y_in, o.labels, false)?;
    if ym.first().map_or(0, Vec::len) != 1 {
        return Err(AnalysisError::Shape);
    }
    let y: Vec<f64> = ym.iter().map(|r| r[0]).collect();
    let x = full_matrix(x_in, o.labels, false)?;
    let n = y.len();
    let k = x.first().map_or(0, Vec::len);
    if x.len() != n || k == 0 {
        return Err(AnalysisError::Shape);
    }
    if n <= k + usize::from(o.constant) {
        return Err(AnalysisError::TooFew);
    }
    let fit = stat::ols(&y, &x, o.constant).ok_or(AnalysisError::Shape)?;
    let x_names: Vec<String> = (0..k)
        .map(|j| match (o.labels, x_in.first().and_then(|r| r.get(j))) {
            (true, Some(c)) => label_of(c),
            _ => format!("{} {}", l.t("xVariable"), j + 1),
        })
        .collect();
    let df_res = fit.df;
    let df_reg = n as f64 - usize::from(o.constant) as f64 - df_res;
    let df_tot = df_reg + df_res;
    let nf = n as f64;
    let adj = 1.0 - (1.0 - fit.r2) * (df_tot / df_res);
    let mut t = Table::default();
    t.push(vec![l.cell("summaryOutput")]);
    t.blank();
    t.push(vec![l.cell("regStats")]);
    t.push(vec![l.cell("multipleR"), num(fit.r2.max(0.0).sqrt())]);
    t.push(vec![l.cell("rSquare"), num(fit.r2)]);
    t.push(vec![l.cell("adjRSquare"), num(adj)]);
    t.push(vec![l.cell("stdErr"), num(fit.se_y)]);
    t.push(vec![l.cell("observations"), num(nf)]);
    t.blank();
    t.push(vec![l.cell("anova")]);
    t.push(vec![
        Out::Empty,
        l.cell("df"),
        l.cell("ss"),
        l.cell("ms"),
        l.cell("f"),
        l.cell("sigF"),
    ]);
    let ms_res = fit.ss_resid / df_res;
    let f = (fit.ss_reg / df_reg) / ms_res;
    t.push(vec![
        l.cell("regression"),
        num(df_reg),
        num(fit.ss_reg),
        num(fit.ss_reg / df_reg),
        num(f),
        num(stat::f_rt(f, df_reg, df_res)),
    ]);
    t.push(vec![
        l.cell("residual"),
        num(df_res),
        num(fit.ss_resid),
        num(ms_res),
    ]);
    t.push(vec![
        l.cell("total"),
        num(df_tot),
        num(fit.ss_reg + fit.ss_resid),
    ]);
    t.blank();
    let extra = (o.confidence - 0.95).abs() > 1e-12;
    let mut head = vec![
        Out::Empty,
        l.cell("coefficients"),
        l.cell("stdErr"),
        l.cell("tStat"),
        l.cell("pValue"),
        l.pct("lower", 0.95),
        l.pct("upper", 0.95),
    ];
    if extra {
        head.push(l.pct("lower", o.confidence));
        head.push(l.pct("upper", o.confidence));
    }
    t.push(head);
    let t95 = stat::t_inv(0.975, df_res);
    let tc = stat::t_inv(1.0 - (1.0 - o.confidence) / 2.0, df_res);
    let coef_row = |name: Out, b: f64, se: Option<f64>| -> Vec<Out> {
        let Some(se) = se else {
            let na = Out::Error(OutError::NA);
            let mut row = vec![
                name,
                num(b),
                na.clone(),
                na.clone(),
                na.clone(),
                na.clone(),
                na.clone(),
            ];
            if extra {
                row.push(na.clone());
                row.push(na);
            }
            return row;
        };
        let ts = b / se;
        let mut row = vec![
            name,
            num(b),
            num(se),
            num(ts),
            num(2.0 * t_tail(ts, df_res)),
            num(b - t95 * se),
            num(b + t95 * se),
        ];
        if extra {
            row.push(num(b - tc * se));
            row.push(num(b + tc * se));
        }
        row
    };
    t.push(coef_row(
        l.cell("intercept"),
        fit.intercept,
        fit.se_intercept,
    ));
    for ((name, b), se) in x_names
        .iter()
        .zip(&fit.coefficients)
        .zip(&fit.se_coefficients)
    {
        t.push(coef_row(Out::Text(name.clone()), *b, Some(*se)));
    }
    if o.residuals || o.std_residuals {
        let y_name = match (o.labels, y_in.first().and_then(|r| r.first())) {
            (true, Some(c)) => label_of(c),
            _ => "Y".to_owned(),
        };
        let pred: Vec<f64> = x
            .iter()
            .map(|r| {
                fit.intercept
                    + r.iter()
                        .zip(&fit.coefficients)
                        .map(|(a, b)| a * b)
                        .sum::<f64>()
            })
            .collect();
        // Standardized residual: residual / standard deviation of the residuals (n − 1).
        let s_res = (fit.ss_resid / (nf - 1.0)).sqrt();
        t.blank();
        t.blank();
        t.push(vec![l.cell("residualOutput")]);
        t.blank();
        let mut head = vec![
            l.cell("observation"),
            Out::Text(format!("{} {y_name}", l.t("predicted"))),
            l.cell("residuals"),
        ];
        if o.std_residuals {
            head.push(l.cell("stdResiduals"));
        }
        t.push(head);
        for i in 0..n {
            let e = y[i] - pred[i];
            let mut row = vec![num((i + 1) as f64), num(pred[i]), num(e)];
            if o.std_residuals {
                row.push(num(e / s_res));
            }
            t.push(row);
        }
    }
    Ok(t)
}

// ------------------------------------------------------------------ Fourier analysis

/// In-place iterative radix-2 Cooley–Tukey FFT; `inverse` uses `e^{+2πi/N}` and divides by N.
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * 2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    if inverse {
        let nf = n as f64;
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v /= nf;
        }
    }
}

fn fourier(g: &Grid, labels: bool, inverse: bool, l: Labels) -> Res<Table> {
    let mut cells = g.iter().flatten();
    if labels {
        cells.next();
    }
    let mut re = Vec::new();
    let mut im = Vec::new();
    for c in cells {
        let (a, b) = match c {
            Item::Num(n) => (*n, 0.0),
            Item::Text(t) => stat::parse_complex(t).ok_or(AnalysisError::NonNumeric)?,
            Item::Empty => return Err(AnalysisError::NonNumeric),
        };
        re.push(a);
        im.push(b);
    }
    let n = re.len();
    if n == 0 || !n.is_power_of_two() || n > MAX_FOURIER {
        return Err(AnalysisError::PowerOfTwo);
    }
    fft(&mut re, &mut im, inverse);
    // Rounding noise far below the largest magnitude is shown as 0.
    let scale = re
        .iter()
        .chain(&im)
        .fold(0.0_f64, |m, v| m.max(v.abs()))
        .max(f64::MIN_POSITIVE);
    let clean = |v: f64| if v.abs() < scale * 1e-13 { 0.0 } else { v };
    let mut t = Table::default();
    if labels {
        t.push(vec![l.cell(if inverse {
            "inverseFourier"
        } else {
            "fourier"
        })]);
    }
    for (a, b) in re.into_iter().zip(im) {
        t.push(vec![
            stat::format_complex(clean(a), clean(b)).map_or(Out::Error(OutError::Num), Out::Text)
        ]);
    }
    Ok(t)
}

#[cfg(test)]
mod tests;
