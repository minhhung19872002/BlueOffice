//! Statistical kernels shared with other crates (BlueSheet analysis tools): the distribution
//! functions behind `NORM.S.DIST`, `T.DIST`, `F.DIST`, `CHISQ.DIST` and their inverses, the
//! least-squares fit behind `LINEST` (ECMA-376 Part 1, §18.17.7) and `COMPLEX` text.
//!
//! Everything is a thin wrapper over the formula functions so a tool and the worksheet
//! function always agree.

use crate::functions::{distributions as d, engineering, regression, stats};

/// Standard normal CDF Φ(x) (`NORM.S.DIST(x, TRUE)`).
#[must_use]
pub fn norm_s_cdf(x: f64) -> f64 {
    stats::norm_cdf(x)
}

/// Inverse standard normal CDF (`NORM.S.INV`); `NaN` outside `0 < p < 1`.
#[must_use]
pub fn norm_s_inv(p: f64) -> f64 {
    stats::norm_s_inv_f(p).unwrap_or(f64::NAN)
}

/// Student t CDF with `v` degrees of freedom (`T.DIST(x, v, TRUE)`).
#[must_use]
pub fn t_cdf(x: f64, v: f64) -> f64 {
    d::t_cdf(x, v)
}

/// The t with P(T ≤ t) = p (`T.INV`); `NaN` outside `0 < p < 1` or for `v` < 1.
#[must_use]
pub fn t_inv(p: f64, v: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) || v.is_nan() || v < 1.0 {
        return f64::NAN;
    }
    d::t_quantile(p, v)
}

/// F CDF (`F.DIST(x, a, b, TRUE)`).
#[must_use]
pub fn f_cdf(x: f64, a: f64, b: f64) -> f64 {
    d::f_cdf(x, a, b)
}

/// F right tail P(F > x) (`F.DIST.RT`).
#[must_use]
pub fn f_rt(x: f64, a: f64, b: f64) -> f64 {
    d::f_rt(x, a, b)
}

/// The x with P(F ≤ x) = p (`F.INV`); `NaN` outside `0 ≤ p < 1`.
#[must_use]
pub fn f_inv(p: f64, a: f64, b: f64) -> f64 {
    if !((0.0..1.0).contains(&p) && a >= 1.0 && b >= 1.0) {
        return f64::NAN;
    }
    d::invert(|x| d::f_cdf(x, a, b), p, 0.0, 2.0)
}

/// The x with P(F > x) = p (`F.INV.RT`).
#[must_use]
pub fn f_inv_rt(p: f64, a: f64, b: f64) -> f64 {
    if !(p > 0.0 && p <= 1.0) {
        return f64::NAN;
    }
    f_inv(1.0 - p, a, b)
}

/// Chi-squared CDF (`CHISQ.DIST(x, k, TRUE)`).
#[must_use]
pub fn chisq_cdf(x: f64, k: f64) -> f64 {
    d::chisq_cdf(x, k)
}

/// A least-squares fit as `LINEST(y, x, const, TRUE)` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsFit {
    /// Coefficients of the x variables, in their order.
    pub coefficients: Vec<f64>,
    /// The constant (0 without one).
    pub intercept: f64,
    /// Standard errors of `coefficients`.
    pub se_coefficients: Vec<f64>,
    /// Standard error of the constant (`None` without one).
    pub se_intercept: Option<f64>,
    /// Coefficient of determination r².
    pub r2: f64,
    /// Standard error of the y estimate.
    pub se_y: f64,
    /// The F statistic.
    pub f: f64,
    /// Residual degrees of freedom.
    pub df: f64,
    /// Regression sum of squares.
    pub ss_reg: f64,
    /// Residual sum of squares.
    pub ss_resid: f64,
}

/// Fits `y ≈ x·m (+ b)` where `x` holds one row of regressors per observation (the `LINEST`
/// kernel). `None` when there are no observations or rows differ in length.
#[must_use]
pub fn ols(y: &[f64], x: &[Vec<f64>], constant: bool) -> Option<OlsFit> {
    let k = x.first().map_or(0, Vec::len);
    if y.is_empty() || k == 0 || x.len() != y.len() || x.iter().any(|r| r.len() != k) {
        return None;
    }
    let data = regression::Data {
        y: y.to_vec(),
        x: x.to_vec(),
        k,
        by_row: false,
    };
    let fit = regression::least_squares(&data, constant).ok()?;
    Some(OlsFit {
        coefficients: fit.m,
        intercept: fit.b,
        se_coefficients: fit.se_m,
        se_intercept: fit.se_b,
        r2: fit.r2,
        se_y: fit.se_y,
        f: fit.f,
        df: fit.df,
        ss_reg: fit.ss_reg,
        ss_resid: fit.ss_resid,
    })
}

/// `(re, im)` of complex-number text such as `3+4i` (`COMPLEX`); a plain number has `im = 0`.
#[must_use]
pub fn parse_complex(text: &str) -> Option<(f64, f64)> {
    engineering::complex_from_text(text)
}

/// The text `COMPLEX(re, im)` returns (`None` when a part is not finite).
#[must_use]
pub fn format_complex(re: f64, im: f64) -> Option<String> {
    engineering::complex_to_text(re, im)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels() {
        // hand: Φ(0) = 0.5; the t distribution is symmetric about 0.
        assert!((norm_s_cdf(0.0) - 0.5).abs() < 1e-15);
        assert!((t_cdf(0.0, 7.0) - 0.5).abs() < 1e-12);
        // Standard tables: z(0.975) = 1.959964; t(0.975, 10) = 2.228139; F(0.95; 2, 12) = 3.885294.
        assert!((norm_s_inv(0.975) - 1.959_964).abs() < 1e-6);
        assert!((t_inv(0.975, 10.0) - 2.228_139).abs() < 1e-6);
        assert!((f_inv_rt(0.05, 2.0, 12.0) - 3.885_294).abs() < 1e-6);
        assert!((f_rt(3.885_294, 2.0, 12.0) - 0.05).abs() < 1e-6);
        assert!(t_inv(1.5, 3.0).is_nan());
    }

    #[test]
    fn ols_line() {
        // hand: y = 2x + 1 exactly.
        let x: Vec<Vec<f64>> = (1..=4).map(|i| vec![f64::from(i)]).collect();
        let y = [3.0, 5.0, 7.0, 9.0];
        let fit = ols(&y, &x, true).expect("fit");
        assert!((fit.coefficients[0] - 2.0).abs() < 1e-12);
        assert!((fit.intercept - 1.0).abs() < 1e-12);
        assert!((fit.r2 - 1.0).abs() < 1e-12);
        assert!(ols(&y, &x[..2], true).is_none());
    }

    #[test]
    fn complex_text() {
        assert_eq!(parse_complex("3+4i"), Some((3.0, 4.0)));
        assert_eq!(parse_complex("-2"), Some((-2.0, 0.0)));
        assert_eq!(format_complex(3.0, -4.0).as_deref(), Some("3-4i"));
        assert_eq!(format_complex(0.0, 1.0).as_deref(), Some("i"));
        assert!(parse_complex("abc").is_none());
    }
}
