//! Werkzeuge: Satzmittelbildung (UNIT18.PAS) und räumliche Transformation (UNIT21.PAS),
//! neu geschrieben: Satzmittel mit Lage I/II und Satzausgleichung, Transformation als
//! räumliche Helmert-Transformation (7 bzw. 6 Parameter) über beliebig viele identische Punkte.

use crate::geo::{norm400, wrap200, RHO};
use crate::io::{KorPoint, Setup, Shot};

// ------------------------------------------------------------------ Satzmittel

#[derive(Clone, Debug)]
pub struct SetMeanRow {
    pub target: String,
    pub hz: f64,
    pub s_hz_mgon: f64,
    pub zenith: Option<f64>,
    pub index_err_mgon: Option<f64>,
    pub dist: Option<f64>,
    pub n_sets: usize,
    /// Verbesserungen je Satz (mgon)
    pub v: Vec<Option<f64>>,
}

#[derive(Clone, Debug)]
pub struct SetMeanStation {
    pub station: String,
    pub ih: f64,
    pub n_sets: usize,
    pub rows: Vec<SetMeanRow>,
    /// Standardabweichung einer Richtung (aus der Satzausgleichung), mgon
    pub s_r_mgon: f64,
    pub warnings: Vec<String>,
}

/// Lage II erkennen: Zenitwinkel > 200 gon
fn is_face2(sh: &Shot) -> bool {
    sh.zenith.map_or(false, |z| z > 200.0)
}

/// Satzmittel bilden. Aufeinanderfolgende Standpunktzeilen desselben Punkts = Sätze; innerhalb
/// eines Satzes werden Lage I und Lage II gemittelt.
pub fn set_means(setups: &[Setup]) -> (Vec<SetMeanStation>, Vec<Setup>) {
    // Gruppen gleicher, aufeinanderfolgender Standpunkte
    let mut groups: Vec<Vec<&Setup>> = Vec::new();
    for s in setups {
        match groups.last_mut() {
            Some(g) if g[0].station == s.station => g.push(s),
            _ => groups.push(vec![s]),
        }
    }
    let mut stations = Vec::new();
    let mut out = Vec::new();
    for g in groups {
        // Je Satz: Ziel → (hz, zenit, strecke, index), Lagen gemittelt
        let mut sets: Vec<Vec<(String, f64, Option<f64>, Option<f64>, Option<f64>, f64, String)>> = Vec::new();
        let mut warnings = Vec::new();
        for s in &g {
            let mut set: Vec<(String, f64, Option<f64>, Option<f64>, Option<f64>, f64, String)> = Vec::new();
            let mut pending: Vec<&Shot> = Vec::new();
            for sh in &s.shots {
                if sh.hz.is_none() {
                    continue;
                }
                pending.push(sh);
            }
            let mut used = vec![false; pending.len()];
            for i in 0..pending.len() {
                if used[i] {
                    continue;
                }
                used[i] = true;
                let a = pending[i];
                // passende zweite Lage suchen
                let j = (i + 1..pending.len()).find(|&j| !used[j] && pending[j].target == a.target && (is_face2(pending[j]) != is_face2(a) || (wrap200(pending[j].hz.unwrap() - a.hz.unwrap()).abs() > 150.0)));
                let (hz, z, ix, d) = if let Some(j) = j {
                    used[j] = true;
                    let b = pending[j];
                    let (f1, f2) = if is_face2(a) { (b, a) } else { (a, b) };
                    let h1 = f1.hz.unwrap();
                    let h2 = norm400(f2.hz.unwrap() - 200.0);
                    let hz = norm400(h1 + wrap200(h2 - h1) / 2.0);
                    let (z, ix) = match (f1.zenith, f2.zenith) {
                        (Some(z1), Some(z2)) => (Some((z1 + 400.0 - z2) / 2.0), Some((z1 + z2 - 400.0) / 2.0 * 1000.0)),
                        (Some(z1), None) => (Some(z1), None),
                        _ => (None, None),
                    };
                    let d = match (f1.dist, f2.dist) {
                        (Some(x), Some(y)) => Some((x + y) / 2.0),
                        (x, y) => x.or(y),
                    };
                    (hz, z, ix, d)
                } else if is_face2(a) {
                    (norm400(a.hz.unwrap() - 200.0), a.zenith.map(|z| 400.0 - z), None, a.dist)
                } else {
                    (a.hz.unwrap(), a.zenith, None, a.dist)
                };
                if set.iter().any(|e| e.0 == a.target) {
                    warnings.push(format!("Ziel {} mehrfach im Satz – nur erste Messung verwendet", a.target));
                    continue;
                }
                set.push((a.target.clone(), hz, z, ix, d, a.th, a.code.clone()));
            }
            if !set.is_empty() {
                sets.push(set);
            }
        }
        if sets.is_empty() {
            continue;
        }
        // Zielreihenfolge: wie im ersten Satz, ergänzt
        let mut targets: Vec<String> = Vec::new();
        for s in &sets {
            for e in s {
                if !targets.contains(&e.0) {
                    targets.push(e.0.clone());
                }
            }
        }
        // Satzverschiebungen bezogen auf Satz 1 (gemeinsame Ziele)
        let base = &sets[0];
        let mut shift = vec![0.0; sets.len()];
        for (k, s) in sets.iter().enumerate().skip(1) {
            let d: Vec<f64> = s.iter().filter_map(|e| base.iter().find(|b| b.0 == e.0).map(|b| wrap200(e.1 - b.1))).collect();
            if d.is_empty() {
                warnings.push(format!("Satz {} hat kein gemeinsames Ziel mit Satz 1", k + 1));
                continue;
            }
            shift[k] = d.iter().sum::<f64>() / d.len() as f64;
        }
        // Mittel, iterativ verfeinert (Satzausgleichung: Satzverschiebung = Mittel der Abweichungen)
        let mut mean = vec![0.0; targets.len()];
        for _ in 0..5 {
            for (ti, t) in targets.iter().enumerate() {
                let v: Vec<f64> = sets.iter().enumerate().filter_map(|(k, s)| s.iter().find(|e| &e.0 == t).map(|e| e.1 - shift[k])).collect();
                let r = v[0];
                mean[ti] = norm400(r + v.iter().map(|x| wrap200(x - r)).sum::<f64>() / v.len() as f64);
            }
            for (k, s) in sets.iter().enumerate() {
                let d: Vec<f64> = s.iter().map(|e| wrap200(e.1 - mean[targets.iter().position(|t| *t == e.0).unwrap()])).collect();
                shift[k] = d.iter().sum::<f64>() / d.len() as f64 + if k == 0 { 0.0 } else { 0.0 };
            }
        }
        // auf erstes Ziel reduzieren (Nullrichtung)
        let zero = mean[0];
        let mut rows = Vec::new();
        let mut vv = 0.0;
        let mut nv = 0usize;
        for (ti, t) in targets.iter().enumerate() {
            let mut v = Vec::new();
            let mut zs = Vec::new();
            let mut ixs = Vec::new();
            let mut ds = Vec::new();
            for (k, s) in sets.iter().enumerate() {
                if let Some(e) = s.iter().find(|e| &e.0 == t) {
                    let r = wrap200(mean[ti] - (e.1 - shift[k])) * 1000.0;
                    vv += r * r;
                    nv += 1;
                    v.push(Some(r));
                    if let Some(z) = e.2 {
                        zs.push(z);
                    }
                    if let Some(i) = e.3 {
                        ixs.push(i);
                    }
                    if let Some(d) = e.4 {
                        ds.push(d);
                    }
                } else {
                    v.push(None);
                }
            }
            let n_sets = v.iter().filter(|x| x.is_some()).count();
            let avg = |x: &Vec<f64>| if x.is_empty() { None } else { Some(x.iter().sum::<f64>() / x.len() as f64) };
            rows.push(SetMeanRow { target: t.clone(), hz: norm400(mean[ti] - zero), s_hz_mgon: 0.0, zenith: avg(&zs), index_err_mgon: avg(&ixs), dist: avg(&ds), n_sets, v });
        }
        let ns = sets.len();
        let f = (nv as isize - targets.len() as isize - (ns as isize - 1)).max(0) as f64;
        let s_r = if f > 0.0 { (vv / f).sqrt() } else { 0.0 };
        for r in rows.iter_mut() {
            r.s_hz_mgon = s_r / (r.n_sets.max(1) as f64).sqrt();
            for (k, v) in r.v.iter().enumerate() {
                if let Some(v) = v {
                    if s_r > 0.0 && v.abs() > 3.0 * s_r && ns > 2 {
                        warnings.push(format!("Ziel {} Satz {}: Abweichung {:.1} mgon > 3·s", r.target, k + 1, v));
                    }
                }
            }
        }
        // gemittelte Messung für *.DAM
        let first = g[0];
        let th_of = |t: &str| -> (f64, String) {
            for s in &sets {
                if let Some(e) = s.iter().find(|e| e.0 == t) {
                    return (e.5, e.6.clone());
                }
            }
            (0.0, "00".into())
        };
        let shots = rows
            .iter()
            .map(|r| {
                let (th, code) = th_of(&r.target);
                Shot { target: r.target.clone(), hz: Some(r.hz), dist: r.dist, dist_horizontal: r.zenith.is_none() && r.dist.is_some(), zenith: r.zenith, dh: None, th, code, line: 0 }
            })
            .collect();
        out.push(Setup { station: first.station.clone(), ih: first.ih, remark: format!("Satzmittel aus {} Sätzen", ns), shots, line: first.line });
        stations.push(SetMeanStation { station: first.station.clone(), ih: first.ih, n_sets: ns, rows, s_r_mgon: s_r, warnings });
    }
    (stations, out)
}

// ------------------------------------------------------------------ Transformation

#[derive(Clone, Debug)]
pub struct TransResult {
    pub with_scale: bool,
    pub t: [f64; 3],
    pub r: [[f64; 3]; 3],
    pub scale: f64,
    /// Drehwinkel um Y-, X-, Z-Achse in gon (ω, φ, κ)
    pub angles: [f64; 3],
    pub residuals: Vec<(String, f64, f64, f64)>,
    pub s0: f64,
    pub identical: usize,
    pub transformed: Vec<KorPoint>,
    pub center_src: [f64; 3],
    pub center_dst: [f64; 3],
}

fn jacobi_eigen4(mut a: [[f64; 4]; 4]) -> ([f64; 4], [[f64; 4]; 4]) {
    let mut v = [[0.0; 4]; 4];
    for (i, r) in v.iter_mut().enumerate() {
        r[i] = 1.0;
    }
    for _ in 0..100 {
        let mut off = 0.0;
        for p in 0..4 {
            for q in p + 1..4 {
                off += a[p][q] * a[p][q];
            }
        }
        if off < 1e-30 {
            break;
        }
        for p in 0..4 {
            for q in p + 1..4 {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..4 {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..4 {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..4 {
                    let (vkp, vkq) = (v[k][p], v[k][q]);
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ([a[0][0], a[1][1], a[2][2], a[3][3]], v)
}

/// Räumliche Helmert-Transformation (Horn, Quaternionen) von `src` nach `dst` über gleichnamige Punkte.
pub fn helmert3d(src: &[KorPoint], dst: &[KorPoint], with_scale: bool) -> Result<TransResult, String> {
    let pairs: Vec<([f64; 3], [f64; 3], String)> = src
        .iter()
        .filter_map(|a| {
            let b = dst.iter().find(|b| b.name == a.name)?;
            Some(([a.y, a.x, a.h.unwrap_or(0.0)], [b.y, b.x, b.h.unwrap_or(0.0)], a.name.clone()))
        })
        .collect();
    if pairs.len() < 3 {
        return Err(format!("mindestens 3 identische Punkte nötig (gefunden: {})", pairs.len()));
    }
    let n = pairs.len() as f64;
    let mut ca = [0.0; 3];
    let mut cb = [0.0; 3];
    for (a, b, _) in &pairs {
        for i in 0..3 {
            ca[i] += a[i] / n;
            cb[i] += b[i] / n;
        }
    }
    let mut m = [[0.0; 3]; 3];
    let (mut sa, mut sb) = (0.0, 0.0);
    for (a, b, _) in &pairs {
        let da = [a[0] - ca[0], a[1] - ca[1], a[2] - ca[2]];
        let db = [b[0] - cb[0], b[1] - cb[1], b[2] - cb[2]];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += da[i] * db[j];
            }
            sa += da[i] * da[i];
            sb += db[i] * db[i];
        }
    }
    let (sxx, sxy, sxz, syx, syy, syz, szx, szy, szz) = (m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2]);
    let nm = [
        [sxx + syy + szz, syz - szy, szx - sxz, sxy - syx],
        [syz - szy, sxx - syy - szz, sxy + syx, szx + sxz],
        [szx - sxz, sxy + syx, -sxx + syy - szz, syz + szy],
        [sxy - syx, szx + sxz, syz + szy, -sxx - syy + szz],
    ];
    let (ev, vecs) = jacobi_eigen4(nm);
    let k = (0..4).max_by(|&i, &j| ev[i].partial_cmp(&ev[j]).unwrap()).unwrap();
    let q = [vecs[0][k], vecs[1][k], vecs[2][k], vecs[3][k]];
    let (q0, qx, qy, qz) = (q[0], q[1], q[2], q[3]);
    let r = [
        [q0 * q0 + qx * qx - qy * qy - qz * qz, 2.0 * (qx * qy - q0 * qz), 2.0 * (qx * qz + q0 * qy)],
        [2.0 * (qy * qx + q0 * qz), q0 * q0 - qx * qx + qy * qy - qz * qz, 2.0 * (qy * qz - q0 * qx)],
        [2.0 * (qz * qx - q0 * qy), 2.0 * (qz * qy + q0 * qx), q0 * q0 - qx * qx - qy * qy + qz * qz],
    ];
    let rot = |p: [f64; 3]| [r[0][0] * p[0] + r[0][1] * p[1] + r[0][2] * p[2], r[1][0] * p[0] + r[1][1] * p[1] + r[1][2] * p[2], r[2][0] * p[0] + r[2][1] * p[1] + r[2][2] * p[2]];
    let scale = if with_scale {
        let mut num = 0.0;
        for (a, b, _) in &pairs {
            let ra = rot([a[0] - ca[0], a[1] - ca[1], a[2] - ca[2]]);
            num += ra[0] * (b[0] - cb[0]) + ra[1] * (b[1] - cb[1]) + ra[2] * (b[2] - cb[2]);
        }
        if sa > 0.0 {
            num / sa
        } else {
            1.0
        }
    } else {
        1.0
    };
    let _ = sb;
    let rc = rot(ca);
    let t = [cb[0] - scale * rc[0], cb[1] - scale * rc[1], cb[2] - scale * rc[2]];
    let apply = |p: [f64; 3]| {
        let rp = rot(p);
        [t[0] + scale * rp[0], t[1] + scale * rp[1], t[2] + scale * rp[2]]
    };
    let mut residuals = Vec::new();
    let mut vv = 0.0;
    for (a, b, nme) in &pairs {
        let c = apply(*a);
        let v = [b[0] - c[0], b[1] - c[1], b[2] - c[2]];
        vv += v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
        residuals.push((nme.clone(), v[0], v[1], v[2]));
    }
    let u = if with_scale { 7.0 } else { 6.0 };
    let f = 3.0 * n - u;
    let s0 = if f > 0.0 { (vv / f).sqrt() } else { 0.0 };
    let transformed = src
        .iter()
        .map(|a| {
            let c = apply([a.y, a.x, a.h.unwrap_or(0.0)]);
            KorPoint { name: a.name.clone(), y: c[0], x: c[1], h: Some(c[2]), code: a.code.clone() }
        })
        .collect();
    // Drehwinkel (Y-X-Z Konvention, für die Anzeige)
    let phi = (-r[2][0]).asin();
    let omega = r[2][1].atan2(r[2][2]);
    let kappa = r[1][0].atan2(r[0][0]);
    Ok(TransResult {
        with_scale,
        t,
        r,
        scale,
        angles: [wrap200(omega * RHO), wrap200(phi * RHO), wrap200(kappa * RHO)],
        residuals,
        s0,
        identical: pairs.len(),
        transformed,
        center_src: ca,
        center_dst: cb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helmert_roundtrip() {
        let src: Vec<KorPoint> = [(0.0, 0.0, 0.0), (100.0, 0.0, 5.0), (0.0, 80.0, 10.0), (60.0, 70.0, -4.0), (30.0, 20.0, 2.0)]
            .iter()
            .enumerate()
            .map(|(i, p)| KorPoint { name: format!("P{}", i), y: p.0, x: p.1, h: Some(p.2), code: "00".into() })
            .collect();
        let (k, s) = (0.3f64, 1.0001);
        let dst: Vec<KorPoint> = src
            .iter()
            .map(|p| KorPoint { name: p.name.clone(), y: 1000.0 + s * (k.cos() * p.y - k.sin() * p.x), x: 2000.0 + s * (k.sin() * p.y + k.cos() * p.x), h: Some(50.0 + s * p.h.unwrap()), code: "00".into() })
            .collect();
        let r = helmert3d(&src, &dst, true).unwrap();
        assert!(r.s0 < 1e-6, "{}", r.s0);
        assert!((r.scale - s).abs() < 1e-9);
    }

    #[test]
    fn set_mean_two_faces() {
        let mk = |t: &str, hz: f64, z: f64| Shot { target: t.into(), hz: Some(hz), dist: Some(100.0), dist_horizontal: false, zenith: Some(z), dh: None, th: 1.5, code: "00".into(), line: 0 };
        let st = |shots: Vec<Shot>| Setup { station: "S".into(), ih: 1.5, remark: String::new(), shots, line: 0 };
        let s1 = st(vec![mk("A", 10.0, 99.0), mk("B", 110.0, 101.0), mk("A", 210.002, 301.002), mk("B", 310.002, 299.002)]);
        let s2 = st(vec![mk("A", 60.0, 99.0), mk("B", 160.001, 101.0)]);
        let (res, out) = set_means(&[s1, s2]);
        assert_eq!(res.len(), 1);
        assert_eq!(out[0].shots.len(), 2);
        assert!((out[0].shots[0].hz.unwrap()).abs() < 1e-9);
        assert!((out[0].shots[1].hz.unwrap() - 100.0).abs() < 0.002);
        assert!((out[0].shots[0].zenith.unwrap() - 98.999).abs() < 0.0011);
    }
}
