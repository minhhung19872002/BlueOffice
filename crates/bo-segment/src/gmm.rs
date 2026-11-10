//! Colour models: k-means clustering (deterministic farthest-point seeding, Lloyd
//! iterations) and Gaussian mixtures with full 3×3 covariances in RGB, as used by the
//! GrabCut family of segmenters (C. Rother, V. Kolmogorov, A. Blake, "GrabCut —
//! Interactive Foreground Extraction using Iterated Graph Cuts", SIGGRAPH 2004).

/// An RGB colour in 0..=255 floating point.
pub type Rgb = [f64; 3];

/// Variance added to every covariance diagonal: flat synthetic colours would
/// otherwise give singular covariances (≈ the noise of 8-bit quantisation, squared).
const REGULARISE: f64 = 4.0;

/// Largest number of points k-means clusters (a strided sample of the rest).
pub const KMEANS_SAMPLE: usize = 20_000;

fn dist2(a: &Rgb, b: &Rgb) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Cluster `points` into at most `k` groups. Seeds: the point nearest the mean, then
/// repeatedly the point farthest from every seed (deterministic); then at most
/// `iterations` Lloyd steps. Returns the label of each point and the centres.
pub fn kmeans(points: &[Rgb], k: usize, iterations: usize) -> (Vec<usize>, Vec<Rgb>) {
    if points.is_empty() || k == 0 {
        return (vec![0; points.len()], Vec::new());
    }
    let n = points.len() as f64;
    let mut mean = [0.0; 3];
    for p in points {
        for c in 0..3 {
            mean[c] += p[c] / n;
        }
    }
    let first = points
        .iter()
        .enumerate()
        .min_by(|a, b| dist2(a.1, &mean).total_cmp(&dist2(b.1, &mean)))
        .map_or(0, |(i, _)| i);
    let mut centres = vec![points[first]];
    let mut nearest: Vec<f64> = points.iter().map(|p| dist2(p, &points[first])).collect();
    while centres.len() < k {
        let (i, d) =
            nearest.iter().enumerate().fold(
                (0, -1.0),
                |acc, (i, d)| if *d > acc.1 { (i, *d) } else { acc },
            );
        if d <= 0.0 {
            break; // fewer distinct colours than clusters
        }
        let c = points[i];
        centres.push(c);
        for (j, p) in points.iter().enumerate() {
            nearest[j] = nearest[j].min(dist2(p, &c));
        }
    }
    let mut labels = vec![0usize; points.len()];
    for _ in 0..iterations.max(1) {
        let mut changed = false;
        for (j, p) in points.iter().enumerate() {
            let best = (0..centres.len())
                .min_by(|a, b| dist2(p, &centres[*a]).total_cmp(&dist2(p, &centres[*b])))
                .unwrap_or(0);
            if labels[j] != best {
                labels[j] = best;
                changed = true;
            }
        }
        let mut sum = vec![[0.0; 3]; centres.len()];
        let mut count = vec![0usize; centres.len()];
        for (p, l) in points.iter().zip(&labels) {
            for c in 0..3 {
                sum[*l][c] += p[c];
            }
            count[*l] += 1;
        }
        for (i, c) in centres.iter_mut().enumerate() {
            if count[i] > 0 {
                let m = count[i] as f64;
                *c = [sum[i][0] / m, sum[i][1] / m, sum[i][2] / m];
            }
        }
        if !changed {
            break;
        }
    }
    (labels, centres)
}

/// One Gaussian of a mixture.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// Mixing weight.
    pub weight: f64,
    /// Mean colour.
    pub mean: Rgb,
    /// Inverse covariance (row-major 3×3).
    inv: [f64; 9],
    /// `ln(weight) − ½ ln det Σ − 3/2 ln 2π`: the log-density at the mean.
    log_norm: f64,
}

impl Component {
    fn log_density(&self, z: &Rgb) -> f64 {
        let d = [
            z[0] - self.mean[0],
            z[1] - self.mean[1],
            z[2] - self.mean[2],
        ];
        let i = &self.inv;
        let q = d[0] * (i[0] * d[0] + i[1] * d[1] + i[2] * d[2])
            + d[1] * (i[3] * d[0] + i[4] * d[1] + i[5] * d[2])
            + d[2] * (i[6] * d[0] + i[7] * d[1] + i[8] * d[2]);
        self.log_norm - 0.5 * q
    }
}

/// A Gaussian mixture model of colours.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gmm {
    /// Components (empty when learnt from no samples).
    pub components: Vec<Component>,
}

fn invert(m: &[f64; 9]) -> Option<([f64; 9], f64)> {
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if det.is_nan() || det <= 1e-12 {
        return None;
    }
    let inv = [
        (m[4] * m[8] - m[5] * m[7]) / det,
        (m[2] * m[7] - m[1] * m[8]) / det,
        (m[1] * m[5] - m[2] * m[4]) / det,
        (m[5] * m[6] - m[3] * m[8]) / det,
        (m[0] * m[8] - m[2] * m[6]) / det,
        (m[2] * m[3] - m[0] * m[5]) / det,
        (m[3] * m[7] - m[4] * m[6]) / det,
        (m[1] * m[6] - m[0] * m[7]) / det,
        (m[0] * m[4] - m[1] * m[3]) / det,
    ];
    Some((inv, det))
}

impl Gmm {
    /// Fit one Gaussian per label (`labels[i] < k`) of `points`: weight, mean and
    /// (regularised) covariance from the points assigned to it.
    pub fn learn(points: &[Rgb], labels: &[usize], k: usize) -> Gmm {
        let mut count = vec![0usize; k];
        let mut sum = vec![[0.0f64; 3]; k];
        let mut prod = vec![[0.0f64; 9]; k];
        for (p, &l) in points.iter().zip(labels) {
            if l >= k {
                continue;
            }
            count[l] += 1;
            for a in 0..3 {
                sum[l][a] += p[a];
                for b in 0..3 {
                    prod[l][a * 3 + b] += p[a] * p[b];
                }
            }
        }
        let total: usize = count.iter().sum();
        let mut components = Vec::new();
        for c in 0..k {
            if count[c] == 0 {
                continue;
            }
            let n = count[c] as f64;
            let mean = [sum[c][0] / n, sum[c][1] / n, sum[c][2] / n];
            let mut cov = [0.0; 9];
            for a in 0..3 {
                for b in 0..3 {
                    cov[a * 3 + b] = prod[c][a * 3 + b] / n - mean[a] * mean[b];
                }
                cov[a * 4] += REGULARISE;
            }
            let Some((inv, det)) = invert(&cov) else {
                continue;
            };
            let weight = n / total as f64;
            components.push(Component {
                weight,
                mean,
                inv,
                log_norm: weight.ln() - 0.5 * det.ln() - 1.5 * (2.0 * std::f64::consts::PI).ln(),
            });
        }
        Gmm { components }
    }

    /// Mixture from k-means clusters of `points` (clustered on an evenly strided
    /// sample of at most [`KMEANS_SAMPLE`] points, then every point joins its nearest
    /// centre).
    pub fn from_kmeans(points: &[Rgb], k: usize) -> Gmm {
        let step = points.len().div_ceil(KMEANS_SAMPLE).max(1);
        let sample: Vec<Rgb> = points.iter().step_by(step).copied().collect();
        let (_, centres) = kmeans(&sample, k, 10);
        if centres.is_empty() {
            return Gmm::default();
        }
        let labels: Vec<usize> = points
            .iter()
            .map(|p| {
                (0..centres.len())
                    .min_by(|a, b| dist2(p, &centres[*a]).total_cmp(&dist2(p, &centres[*b])))
                    .unwrap_or(0)
            })
            .collect();
        Gmm::learn(points, &labels, centres.len())
    }

    /// The component most likely to have produced `z`.
    pub fn component_of(&self, z: &Rgb) -> usize {
        self.components
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.log_density(z).total_cmp(&b.1.log_density(z)))
            .map_or(0, |(i, _)| i)
    }

    /// `−ln p(z)` under the mixture (large when no component is learnt).
    pub fn neg_log_likelihood(&self, z: &Rgb) -> f64 {
        if self.components.is_empty() {
            return 1e3;
        }
        // log-sum-exp over the components.
        let mut logs = [f64::NEG_INFINITY; 16];
        let k = self.components.len().min(logs.len());
        for (l, c) in logs.iter_mut().zip(&self.components) {
            *l = c.log_density(z);
        }
        let m = logs[..k].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let s: f64 = logs[..k].iter().map(|l| (l - m).exp()).sum();
        -(m + s.ln())
    }

    /// Refit: assign every point to its most likely component, then learn again.
    #[must_use]
    pub fn refit(&self, points: &[Rgb]) -> Gmm {
        if self.components.is_empty() {
            return Gmm::from_kmeans(points, 5);
        }
        let labels: Vec<usize> = points.iter().map(|p| self.component_of(p)).collect();
        Gmm::learn(points, &labels, self.components.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmeans_separates_three_clusters() {
        let mut pts = Vec::new();
        for i in 0..30 {
            let j = f64::from(i % 5);
            pts.push([10.0 + j, 10.0, 10.0]);
            pts.push([200.0, 20.0 + j, 20.0]);
            pts.push([30.0, 30.0, 220.0 - j]);
        }
        let (labels, centres) = kmeans(&pts, 3, 20);
        assert_eq!(centres.len(), 3);
        // Points of the same source cluster share a label, different clusters differ.
        for i in 0..30 {
            assert_eq!(labels[i * 3], labels[0]);
            assert_eq!(labels[i * 3 + 1], labels[1]);
            assert_eq!(labels[i * 3 + 2], labels[2]);
        }
        assert!(labels[0] != labels[1] && labels[1] != labels[2] && labels[0] != labels[2]);
        // Centres are the cluster means: (12, 10, 10), (200, 22, 20), (30, 30, 218).
        let mut cs = centres.clone();
        cs.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert!((cs[0][0] - 12.0).abs() < 1e-9 && (cs[2][1] - 22.0).abs() < 1e-9);
        assert!((cs[1][2] - 218.0).abs() < 1e-9);
    }

    #[test]
    fn kmeans_with_fewer_colours_than_clusters() {
        let pts = vec![[5.0, 5.0, 5.0]; 10];
        let (labels, centres) = kmeans(&pts, 5, 10);
        assert_eq!(centres.len(), 1);
        assert!(labels.iter().all(|l| *l == 0));
    }

    #[test]
    fn gaussian_density_matches_closed_form() {
        // One isotropic cluster: variance per channel = 2 (values m±√2·…) + REGULARISE.
        let pts: Vec<Rgb> = vec![
            [99.0, 50.0, 10.0],
            [101.0, 50.0, 10.0],
            [100.0, 49.0, 10.0],
            [100.0, 51.0, 10.0],
            [100.0, 50.0, 9.0],
            [100.0, 50.0, 11.0],
        ];
        let g = Gmm::learn(&pts, &[0; 6], 1);
        assert_eq!(g.components.len(), 1);
        let c = &g.components[0];
        assert_eq!(c.mean, [100.0, 50.0, 10.0]);
        // Each channel: variance (1+1)/6 = 1/3 plus 4; covariances 0.
        let var = 1.0 / 3.0 + REGULARISE;
        let z = [102.0, 50.0, 10.0];
        let want = 1.5 * (2.0 * std::f64::consts::PI).ln() + 1.5 * var.ln() + 0.5 * 4.0 / var;
        assert!((g.neg_log_likelihood(&z) - want).abs() < 1e-9);
    }

    #[test]
    fn mixture_prefers_the_nearer_model() {
        let red: Vec<Rgb> = (0..50)
            .map(|i| [200.0 + f64::from(i % 7), 20.0, 30.0])
            .collect();
        let blue: Vec<Rgb> = (0..50)
            .map(|i| [20.0, 30.0, 200.0 + f64::from(i % 5)])
            .collect();
        let fg = Gmm::from_kmeans(&red, 5);
        let bg = Gmm::from_kmeans(&blue, 5);
        let z = [190.0, 25.0, 40.0];
        assert!(fg.neg_log_likelihood(&z) < bg.neg_log_likelihood(&z));
        let refit = fg.refit(&red);
        // Components no point prefers any more are dropped, never added.
        assert!(!refit.components.is_empty() && refit.components.len() <= fg.components.len());
        assert!(refit.neg_log_likelihood(&z) < bg.neg_log_likelihood(&z));
        assert_ne!(Gmm::default().refit(&red).components.len(), 0);
    }
}
