//! Map projections of chartex `ST_GeoProjectionType` ([MS-ODRAWXML]), implemented from the public
//! formulas of J. P. Snyder, *Map Projections — A Working Manual* (USGS Professional Paper 1395,
//! 1987) and A. H. Robinson's 1974 table. Coordinates are in units of the Earth's radius; `y` grows
//! northwards.

use std::f64::consts::{FRAC_PI_4, PI};

/// A projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// Mercator (conformal cylinder): `x = λ`, `y = ln tan(π/4 + φ/2)`; latitudes clamped to ±85°.
    Mercator,
    /// Miller cylindrical: `x = λ`, `y = 1.25 ln tan(π/4 + 0.4 φ)`.
    Miller,
    /// Robinson pseudo-cylinder: interpolated table, `x = 0.8487 X λ`, `y = 1.3523 Y`.
    Robinson,
    /// Albers equal-area conic with standard parallels `φ1`, `φ2` and origin `(λ0, φ0)` (degrees).
    Albers {
        /// First standard parallel.
        lat1: f64,
        /// Second standard parallel.
        lat2: f64,
        /// Central meridian.
        lon0: f64,
        /// Latitude of origin.
        lat0: f64,
    },
}

/// Robinson's table (every 5° from the equator): X (parallel length) and Y (distance from the
/// equator), as published by Robinson (1974) and reproduced by Snyder (1993).
const ROBINSON: [(f64, f64); 19] = [
    (1.0000, 0.0000),
    (0.9986, 0.0620),
    (0.9954, 0.1240),
    (0.9900, 0.1860),
    (0.9822, 0.2480),
    (0.9730, 0.3100),
    (0.9600, 0.3720),
    (0.9427, 0.4340),
    (0.9216, 0.4958),
    (0.8962, 0.5571),
    (0.8679, 0.6176),
    (0.8350, 0.6769),
    (0.7986, 0.7346),
    (0.7597, 0.7903),
    (0.7186, 0.8435),
    (0.6732, 0.8936),
    (0.6213, 0.9394),
    (0.5722, 0.9761),
    (0.5322, 1.0000),
];

impl Projection {
    /// The projection of a chartex `projectionType` (`None` / unknown: `default`).
    #[must_use]
    pub fn of_chartex(name: Option<&str>, default: Projection, extent: [f64; 4]) -> Projection {
        match name {
            Some("mercator") => Projection::Mercator,
            Some("miller") => Projection::Miller,
            Some("robinson") => Projection::Robinson,
            Some("albers") => Projection::albers_for(extent),
            _ => default,
        }
    }

    /// An Albers projection for a `[min lon, min lat, max lon, max lat]` extent: standard parallels
    /// at one sixth and five sixths of its latitudes (Snyder's rule of thumb).
    #[must_use]
    pub fn albers_for([x0, y0, x1, y1]: [f64; 4]) -> Projection {
        let dy = y1 - y0;
        let (mut lat1, mut lat2) = (y0 + dy / 6.0, y1 - dy / 6.0);
        // Parallels symmetric about the equator make the cone a cylinder (n = 0).
        if (lat1 + lat2).abs() < 1.0 {
            lat1 = 20.0_f64.copysign(lat1 + lat2 + 1e-9);
            lat2 = lat1 + 30.0_f64.copysign(lat1);
        }
        Projection::Albers {
            lat1,
            lat2,
            lon0: f64::midpoint(x0, x1),
            lat0: f64::midpoint(y0, y1),
        }
    }

    /// Projects `(lon, lat)` in degrees.
    #[must_use]
    pub fn project(&self, lon: f64, lat: f64) -> (f64, f64) {
        let lam = lon.to_radians();
        match *self {
            Projection::Mercator => {
                let phi = lat.clamp(-85.0, 85.0).to_radians();
                (lam, (FRAC_PI_4 + phi / 2.0).tan().ln())
            }
            Projection::Miller => {
                let phi = lat.clamp(-90.0, 90.0).to_radians();
                (lam, 1.25 * (FRAC_PI_4 + 0.4 * phi).tan().ln())
            }
            Projection::Robinson => {
                let a = lat.abs().min(90.0);
                let i = ((a / 5.0).floor() as usize).min(17);
                let f = (a - i as f64 * 5.0) / 5.0;
                let (x0, y0) = ROBINSON[i];
                let (x1, y1) = ROBINSON[i + 1];
                let (x, y) = (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
                (0.8487 * x * lam, (1.3523 * y).copysign(lat))
            }
            Projection::Albers {
                lat1,
                lat2,
                lon0,
                lat0,
            } => {
                let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
                let n = f64::midpoint(p1.sin(), p2.sin());
                if n.abs() < 1e-9 {
                    // Degenerate cone: the cylindrical equal-area limit.
                    return (lam - lon0.to_radians(), lat.to_radians().sin());
                }
                let c = p1.cos().powi(2) + 2.0 * n * p1.sin();
                let rho = |phi: f64| (c - 2.0 * n * phi.sin()).max(0.0).sqrt() / n;
                let rho0 = rho(lat0.to_radians());
                let mut d = lam - lon0.to_radians();
                // Wrap the longitude difference into (−π, π].
                while d > PI {
                    d -= 2.0 * PI;
                }
                while d <= -PI {
                    d += 2.0 * PI;
                }
                let theta = n * d;
                let r = rho(lat.clamp(-90.0, 90.0).to_radians());
                (r * theta.sin(), rho0 - r * theta.cos())
            }
        }
    }
}

/// Latitude bound of the cylindrical projections' world view.
pub const WORLD_LAT: (f64, f64) = (-58.0, 84.0);

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn mercator_known_points() {
        let p = Projection::Mercator;
        assert!(close(p.project(0.0, 0.0), (0.0, 0.0)));
        // y(45°) = ln tan 67.5° = asinh(1) = 0.881373587 (Snyder, Table 7).
        let (x, y) = p.project(90.0, 45.0);
        assert!((x - FRAC_PI_2).abs() < 1e-12);
        assert!((y - 0.881_373_587).abs() < 1e-8, "{y}");
        // y(60°) = 1.316957897; southern latitudes mirror.
        assert!((p.project(0.0, -60.0).1 + 1.316_957_897).abs() < 1e-8);
    }

    #[test]
    fn miller_known_points() {
        let p = Projection::Miller;
        // y(45°) = 1.25 ln tan 63° = 0.842844 ; y(90°) = 1.25 ln tan 81° = 2.303412.
        assert!((p.project(0.0, 45.0).1 - 0.842_844).abs() < 1e-5);
        assert!((p.project(0.0, 90.0).1 - 2.303_412).abs() < 1e-5);
        assert!((p.project(-180.0, 0.0).0 + PI).abs() < 1e-12);
    }

    #[test]
    fn robinson_known_points() {
        let p = Projection::Robinson;
        // At the table's nodes: x = 0.8487 X λ, y = 1.3523 Y.
        let (x, y) = p.project(180.0, 45.0);
        assert!((x - 0.8487 * 0.8962 * PI).abs() < 1e-9);
        assert!((y - 1.3523 * 0.5571).abs() < 1e-9);
        let (_, y) = p.project(0.0, -90.0);
        assert!((y + 1.3523).abs() < 1e-9);
        // Halfway between 10° and 15°: linear in the table.
        let (_, y) = p.project(0.0, 12.5);
        assert!((y - 1.3523 * 0.155).abs() < 1e-9);
    }

    #[test]
    fn albers_known_points() {
        // Snyder's numerical example (sphere, R = 1): φ1 = 29.5°, φ2 = 45.5°, φ0 = 23°, λ0 = −96°,
        // φ = 35°, λ = −75° → x = 0.2952720, y = 0.2416774.
        let p = Projection::Albers {
            lat1: 29.5,
            lat2: 45.5,
            lon0: -96.0,
            lat0: 23.0,
        };
        let (x, y) = p.project(-75.0, 35.0);
        assert!((x - 0.295_272_0).abs() < 1e-6, "{x}");
        assert!((y - 0.241_677_4).abs() < 1e-6, "{y}");
    }
}
