//! Grundkonstanten und Winkelfunktionen (Neugrad / gon), wie im Original WAUSGL
//! (OSTART.PAS: rwi = 100, ro = 2*rwi/pi, rm = 6380000).

/// rechter Winkel in gon
pub const RWI: f64 = 100.0;
/// gon je Radiant (ro = 2*rwi/pi)
pub const RHO: f64 = 2.0 * RWI / std::f64::consts::PI;
/// mittlerer Erdradius in m
pub const RM: f64 = 6_380_000.0;
/// Refraktionskoeffizient, der im Original in der Konstante 68e-9 steckt: (1-k)/(2R) = 68e-9
pub const K_REFR: f64 = 0.1324;

/// Richtungswinkel von (y0,x0) nach (y1,x1) in gon [0,400) und Strecke (riwi im Original;
/// y = Rechtswert, x = Hochwert, t von Nord im Uhrzeigersinn).
pub fn riwi(y0: f64, x0: f64, y1: f64, x1: f64) -> (f64, f64) {
    let dy = y1 - y0;
    let dx = x1 - x0;
    let s = (dy * dy + dx * dx).sqrt();
    if s == 0.0 {
        return (0.0, 0.0);
    }
    (norm400(dy.atan2(dx) * RHO), s)
}

/// Winkel auf [0, 400) gon
pub fn norm400(a: f64) -> f64 {
    let mut r = a % 400.0;
    if r < 0.0 {
        r += 400.0;
    }
    if r >= 400.0 {
        r -= 400.0;
    }
    r
}

/// Winkeldifferenz auf (-200, 200] gon
pub fn wrap200(a: f64) -> f64 {
    let mut r = norm400(a);
    if r > 200.0 {
        r -= 400.0;
    }
    r
}

/// Horizontalstrecke und Höhenunterschied aus Schrägstrecke und Zenitwinkel (HStr im Original):
/// s = |S sin z|, dh = s / tan z + 68e-9 s² (Erdkrümmung und Refraktion).
pub fn hstr(slope: f64, zenith_gon: f64) -> (f64, f64) {
    let z = zenith_gon / RHO;
    let s = (slope * z.sin()).abs();
    if (zenith_gon - RWI).abs() < 1e-12 || (zenith_gon - 3.0 * RWI).abs() < 1e-12 {
        return (s, 0.0);
    }
    let zz = if zenith_gon > 2.0 * RWI { 4.0 * RWI - zenith_gon } else { zenith_gon } / RHO;
    (s, s / zz.tan() + curv_refr(s))
}

/// Korrektur für Erdkrümmung und Refraktion (Original: 68e-9 * s²)
pub fn curv_refr(s: f64) -> f64 {
    68e-9 * s * s
}

/// Zahl mit fester Nachkommazahl, Dezimalpunkt
pub fn f(v: f64, nk: usize) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    format!("{:.*}", nk, v)
}

/// Rechte Ausrichtung in Breite w
pub fn r(v: f64, w: usize, nk: usize) -> String {
    format!("{:>w$}", f(v, nk), w = w)
}

pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:02}.{:02}.{:04}", d, m, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bearings() {
        assert!((riwi(0.0, 0.0, 1.0, 0.0).0 - 100.0).abs() < 1e-12);
        assert!((riwi(0.0, 0.0, 0.0, 1.0).0).abs() < 1e-12);
        assert!((riwi(0.0, 0.0, 0.0, -1.0).0 - 200.0).abs() < 1e-12);
        assert!((riwi(0.0, 0.0, -1.0, 0.0).0 - 300.0).abs() < 1e-12);
        assert!((wrap200(399.0) + 1.0).abs() < 1e-12);
    }
}
