//! Näherungskoordinaten der Neupunkte (wie UNIT3.PAS Vorlkoord: Orientierung t-r, Polar,
//! Bogenschnitt, Rückwärtseinschnitt RWE, Vorwärtseinschnitt VWE – hier neu geschrieben und um die
//! freie Stationierung ergänzt) und Näherungshöhen (UNIT15 vorl. Höhen).

use crate::geo::{hstr, norm400, riwi, wrap200, RHO};
use crate::io::{NivLine, Setup};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct Approx {
    pub plan: Option<(f64, f64)>,
    pub h: Option<f64>,
}

pub type PointMap = HashMap<String, Approx>;

/// Horizontalstrecke eines Schusses (aus Schrägstrecke und Zenitwinkel)
pub fn shot_hdist(sh: &crate::io::Shot) -> Option<f64> {
    let d = sh.dist?;
    if sh.dist_horizontal {
        return Some(d);
    }
    match sh.zenith {
        Some(z) => Some(hstr(d, z).0),
        None => Some(d),
    }
}

/// Höhenunterschied Standpunkt → Zielpunkt aus einem Schuss (Bodenpunkt zu Bodenpunkt)
pub fn shot_dh(sh: &crate::io::Shot, ih: f64, hdist_from_coords: Option<f64>) -> Option<f64> {
    if sh.dist_horizontal {
        // Original: gegebener Höhenunterschied wird direkt verwendet
        return sh.dh;
    }
    let z = sh.zenith?;
    if let Some(d) = sh.dist {
        let (_, dh) = hstr(d, z);
        return Some(dh + ih - sh.th);
    }
    let s = hdist_from_coords?;
    let zz = if z > 200.0 { 400.0 - z } else { z };
    if (zz - 100.0).abs() < 1e-9 {
        return Some(ih - sh.th);
    }
    Some(s / (zz / RHO).tan() + crate::geo::curv_refr(s) + ih - sh.th)
}

fn circ_mean(vals: &[f64]) -> f64 {
    let (mut s, mut c) = (0.0, 0.0);
    for v in vals {
        let a = v / RHO;
        s += a.sin();
        c += a.cos();
    }
    norm400(s.atan2(c) * RHO)
}

/// Orientierung (t-r) eines Standpunkts aus bekannten Zielen
pub fn orientation(st: &Setup, pts: &PointMap) -> Option<f64> {
    let (ys, xs) = pts.get(&st.station)?.plan?;
    let mut v = Vec::new();
    for sh in &st.shots {
        let (Some(hz), Some(Approx { plan: Some((y, x)), .. })) = (sh.hz, pts.get(&sh.target)) else { continue };
        let (t, s) = riwi(ys, xs, *y, *x);
        if s > 1e-6 {
            v.push(norm400(t - hz));
        }
    }
    if v.is_empty() {
        return None;
    }
    // robuster Mittelwert: Werte, die mehr als 0.1 gon vom Median abweichen, verwerfen
    let mut sorted: Vec<f64> = v.iter().map(|a| wrap200(*a - v[0])).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = norm400(v[0] + sorted[sorted.len() / 2]);
    let good: Vec<f64> = v.iter().cloned().filter(|a| wrap200(a - med).abs() < 0.1).collect();
    Some(if good.is_empty() { med } else { circ_mean(&good) })
}

fn intersect_rays(p1: (f64, f64), t1: f64, p2: (f64, f64), t2: f64) -> Option<(f64, f64)> {
    // y = y1 + u sin t1, x = x1 + u cos t1
    let (s1, c1) = (t1 / RHO).sin_cos();
    let (s2, c2) = (t2 / RHO).sin_cos();
    let det = s1 * (-c2) - (-s2) * c1;
    if det.abs() < 1e-6 {
        return None;
    }
    let (dy, dx) = (p2.0 - p1.0, p2.1 - p1.1);
    let u = (dy * (-c2) - (-s2) * dx) / det;
    Some((p1.0 + u * s1, p1.1 + u * c1))
}

/// Stellt alle möglichen Näherungskoordinaten auf; Ergebnis: Liste der Bestimmungsarten (Protokoll)
pub fn approximate_plan(setups: &[Setup], pts: &mut PointMap) -> Vec<String> {
    let mut log = Vec::new();
    for _round in 0..50 {
        let mut changed = false;
        let ori: Vec<Option<f64>> = setups.iter().map(|s| orientation(s, pts)).collect();

        // 1) polar von orientierten Standpunkten
        for (i, st) in setups.iter().enumerate() {
            let (Some(o), Some(Approx { plan: Some(ps), .. })) = (ori[i], pts.get(&st.station).copied()) else { continue };
            for sh in &st.shots {
                if pts.get(&sh.target).and_then(|a| a.plan).is_some() {
                    continue;
                }
                let (Some(hz), Some(s)) = (sh.hz, shot_hdist(sh)) else { continue };
                let t = (hz + o) / RHO;
                let p = (ps.0 + s * t.sin(), ps.1 + s * t.cos());
                pts.entry(sh.target.clone()).or_default().plan = Some(p);
                log.push(format!("{}: polar von {}", sh.target, st.station));
                changed = true;
            }
        }
        if changed {
            continue;
        }

        // 2) freie Stationierung: unbekannter Standpunkt, >= 2 bekannte Ziele mit Richtung und Strecke
        for st in setups {
            if pts.get(&st.station).and_then(|a| a.plan).is_some() {
                continue;
            }
            let mut loc = Vec::new();
            let mut glob = Vec::new();
            for sh in &st.shots {
                let (Some(hz), Some(s), Some(g)) = (sh.hz, shot_hdist(sh), pts.get(&sh.target).and_then(|a| a.plan)) else { continue };
                let a = hz / RHO;
                loc.push((s * a.sin(), s * a.cos()));
                glob.push(g);
            }
            if loc.len() < 2 {
                continue;
            }
            // 2D-Helmert (Drehung + Verschiebung, Maßstab frei für Näherung)
            let n = loc.len() as f64;
            let (ly, lx) = (loc.iter().map(|p| p.0).sum::<f64>() / n, loc.iter().map(|p| p.1).sum::<f64>() / n);
            let (gy, gx) = (glob.iter().map(|p| p.0).sum::<f64>() / n, glob.iter().map(|p| p.1).sum::<f64>() / n);
            let (mut a, mut b, mut q) = (0.0, 0.0, 0.0);
            for (l, g) in loc.iter().zip(&glob) {
                let (u, v) = (l.0 - ly, l.1 - lx);
                let (uu, vv) = (g.0 - gy, g.1 - gx);
                a += u * uu + v * vv;
                b += v * uu - u * vv;
                q += u * u + v * v;
            }
            if q < 1e-9 {
                continue;
            }
            let (ca, sb) = (a / q, b / q);
            // Standpunkt = Ursprung des lokalen Systems
            let y = gy + ca * (0.0 - ly) + sb * (0.0 - lx);
            let x = gx - sb * (0.0 - ly) + ca * (0.0 - lx);
            pts.entry(st.station.clone()).or_default().plan = Some((y, x));
            log.push(format!("{}: freie Stationierung ({} Anschlüsse)", st.station, loc.len()));
            changed = true;
        }
        if changed {
            continue;
        }

        // 3) Vorwärtseinschnitt aus zwei orientierten Strahlen
        let mut rays: HashMap<String, Vec<((f64, f64), f64, String)>> = HashMap::new();
        for (i, st) in setups.iter().enumerate() {
            let (Some(o), Some(Approx { plan: Some(ps), .. })) = (ori[i], pts.get(&st.station).copied()) else { continue };
            for sh in &st.shots {
                if pts.get(&sh.target).and_then(|a| a.plan).is_some() {
                    continue;
                }
                if let Some(hz) = sh.hz {
                    rays.entry(sh.target.clone()).or_default().push((ps, norm400(hz + o), st.station.clone()));
                }
            }
        }
        let mut names: Vec<_> = rays.keys().cloned().collect();
        names.sort();
        for name in names {
            let r = &rays[&name];
            let mut best: Option<((f64, f64), f64, String)> = None;
            for i in 0..r.len() {
                for j in i + 1..r.len() {
                    if r[i].2 == r[j].2 {
                        continue;
                    }
                    let gamma = wrap200(r[j].1 - r[i].1).abs();
                    let q = (gamma.min(200.0 - gamma) / 100.0).min(1.0);
                    if q < 0.05 {
                        continue;
                    }
                    if let Some(p) = intersect_rays(r[i].0, r[i].1, r[j].0, r[j].1) {
                        // Schnittpunkt muss vor beiden Standpunkten liegen
                        let (ti, _) = riwi(r[i].0 .0, r[i].0 .1, p.0, p.1);
                        let (tj, _) = riwi(r[j].0 .0, r[j].0 .1, p.0, p.1);
                        if wrap200(ti - r[i].1).abs() > 1.0 || wrap200(tj - r[j].1).abs() > 1.0 {
                            continue;
                        }
                        if best.as_ref().map_or(true, |b| q > b.1) {
                            best = Some((p, q, format!("{}/{}", r[i].2, r[j].2)));
                        }
                    }
                }
            }
            if let Some((p, _, via)) = best {
                pts.entry(name.clone()).or_default().plan = Some(p);
                log.push(format!("{}: Vorwärtseinschnitt {}", name, via));
                changed = true;
            }
        }
        if changed {
            continue;
        }

        // 4) Bogenschnitt aus zwei Strecken von bekannten Standpunkten (Seite über dritte Strecke oder Richtungssinn)
        let mut arcs: HashMap<String, Vec<((f64, f64), f64)>> = HashMap::new();
        for st in setups {
            let Some(ps) = pts.get(&st.station).and_then(|a| a.plan) else { continue };
            for sh in &st.shots {
                if pts.get(&sh.target).and_then(|a| a.plan).is_some() {
                    continue;
                }
                if let Some(s) = shot_hdist(sh) {
                    arcs.entry(sh.target.clone()).or_default().push((ps, s));
                }
            }
        }
        let mut names: Vec<_> = arcs.keys().cloned().collect();
        names.sort();
        for name in names {
            let a = &arcs[&name];
            if a.len() < 3 {
                continue;
            }
            let ((p1, r1), (p2, r2)) = (a[0], a[1]);
            let (t, d) = riwi(p1.0, p1.1, p2.0, p2.1);
            if d < 1e-6 || d > r1 + r2 || d < (r1 - r2).abs() {
                continue;
            }
            let cosg = ((r1 * r1 + d * d - r2 * r2) / (2.0 * r1 * d)).clamp(-1.0, 1.0);
            let g = cosg.acos() * RHO;
            let cand: Vec<(f64, f64)> = [t + g, t - g].iter().map(|tt| (p1.0 + r1 * (tt / RHO).sin(), p1.1 + r1 * (tt / RHO).cos())).collect();
            let (p3, r3) = a[2];
            let err = |c: &(f64, f64)| (riwi(p3.0, p3.1, c.0, c.1).1 - r3).abs();
            let c = if err(&cand[0]) < err(&cand[1]) { cand[0] } else { cand[1] };
            if err(&c) < 0.5 {
                pts.entry(name.clone()).or_default().plan = Some(c);
                log.push(format!("{}: Bogenschnitt", name));
                changed = true;
            }
        }
        if changed {
            continue;
        }

        // 5) Rückwärtseinschnitt: unbekannter Standpunkt, >= 3 bekannte Ziele nur mit Richtungen
        for st in setups {
            if pts.get(&st.station).and_then(|a| a.plan).is_some() {
                continue;
            }
            let obs: Vec<((f64, f64), f64)> = st.shots.iter().filter_map(|sh| Some((pts.get(&sh.target)?.plan?, sh.hz?))).collect();
            if obs.len() < 3 {
                continue;
            }
            if let Some(p) = resection(&obs) {
                pts.entry(st.station.clone()).or_default().plan = Some(p);
                log.push(format!("{}: Rückwärtseinschnitt ({} Ziele)", st.station, obs.len()));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    log
}

/// Rückwärtseinschnitt durch Ausgleichung (y, x, o) von mehreren Startwerten aus
fn resection(obs: &[((f64, f64), f64)]) -> Option<(f64, f64)> {
    let n = obs.len() as f64;
    let (cy, cx) = (obs.iter().map(|o| o.0 .0).sum::<f64>() / n, obs.iter().map(|o| o.0 .1).sum::<f64>() / n);
    let ext = obs.iter().map(|o| ((o.0 .0 - cy).powi(2) + (o.0 .1 - cx).powi(2)).sqrt()).fold(1.0, f64::max);
    let mut best: Option<((f64, f64), f64)> = None;
    for k in 0..9 {
        let a = k as f64 * 400.0 / 8.0 / RHO;
        let rr = if k == 0 { 0.0 } else { ext * 0.7 };
        let (mut y, mut x) = (cy + rr * a.sin(), cx + rr * a.cos());
        let mut ok = true;
        for _ in 0..30 {
            let o = circ_mean(&obs.iter().map(|(p, hz)| norm400(riwi(y, x, p.0, p.1).0 - hz)).collect::<Vec<_>>());
            // Normalgleichungen 3x3
            let mut nm = [[0.0f64; 3]; 3];
            let mut nv = [0.0f64; 3];
            for (p, hz) in obs {
                let (t, s) = riwi(y, x, p.0, p.1);
                if s < 1e-6 {
                    ok = false;
                    break;
                }
                let (dy, dx) = (p.0 - y, p.1 - x);
                let a = [-RHO * dx / (s * s), RHO * dy / (s * s), -1.0];
                let l = wrap200(hz - (t - o));
                for i in 0..3 {
                    for j in 0..3 {
                        nm[i][j] += a[i] * a[j];
                    }
                    nv[i] += a[i] * l;
                }
            }
            if !ok {
                break;
            }
            let Some(d) = solve3(nm, nv) else {
                ok = false;
                break;
            };
            y += d[0];
            x += d[1];
            if d[0].abs() + d[1].abs() < 1e-6 {
                break;
            }
        }
        if !ok || !y.is_finite() {
            continue;
        }
        let o = circ_mean(&obs.iter().map(|(p, hz)| norm400(riwi(y, x, p.0, p.1).0 - hz)).collect::<Vec<_>>());
        let res: f64 = obs.iter().map(|(p, hz)| wrap200(riwi(y, x, p.0, p.1).0 - o - hz).powi(2)).sum();
        if best.map_or(true, |b| res < b.1) {
            best = Some(((y, x), res));
        }
    }
    best.filter(|b| b.1.sqrt() < 0.5).map(|b| b.0)
}

fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for c in 0..3 {
        let p = (c..3).max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())?;
        if a[p][c].abs() < 1e-14 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in 0..3 {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in 0..3 {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    Some([b[0] / a[0][0], b[1] / a[1][1], b[2] / a[2][2]])
}

/// Näherungshöhen über trig. Höhenunterschiede und Nivellement fortpflanzen
pub fn approximate_heights(setups: &[Setup], niv: &[NivLine], pts: &mut PointMap) -> Vec<String> {
    let mut log = Vec::new();
    // Kanten: (von, nach, dh)
    let mut edges: Vec<(String, String, f64)> = Vec::new();
    for st in setups {
        let ps = pts.get(&st.station).and_then(|a| a.plan);
        for sh in &st.shots {
            let s_coord = match (ps, pts.get(&sh.target).and_then(|a| a.plan)) {
                (Some(a), Some(b)) => Some(riwi(a.0, a.1, b.0, b.1).1),
                _ => None,
            };
            if let Some(dh) = shot_dh(sh, st.ih, s_coord) {
                edges.push((st.station.clone(), sh.target.clone(), dh));
            }
        }
    }
    for n in niv {
        edges.push((n.from.clone(), n.to.clone(), n.dh));
    }
    for _ in 0..200 {
        let mut changed = false;
        for (a, b, dh) in &edges {
            let ha = pts.get(a).and_then(|p| p.h);
            let hb = pts.get(b).and_then(|p| p.h);
            match (ha, hb) {
                (Some(h), None) => {
                    pts.entry(b.clone()).or_default().h = Some(h + dh);
                    log.push(format!("{}: Höhe von {}", b, a));
                    changed = true;
                }
                (None, Some(h)) => {
                    pts.entry(a.clone()).or_default().h = Some(h - dh);
                    log.push(format!("{}: Höhe von {}", a, b));
                    changed = true;
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::Shot;

    fn shot(t: &str, hz: f64, d: Option<f64>) -> Shot {
        Shot { target: t.into(), hz: Some(hz), dist: d, dist_horizontal: true, zenith: None, dh: None, th: 0.0, code: "00".into(), line: 0 }
    }

    #[test]
    fn resection_finds_station() {
        let truth = (100.0, 200.0);
        let targets = [("A", (0.0, 0.0)), ("B", (300.0, 50.0)), ("C", (150.0, 400.0)), ("D", (-50.0, 350.0))];
        let o = 37.0;
        let mut pts = PointMap::new();
        let mut shots = Vec::new();
        for (n, p) in targets {
            pts.insert(n.into(), Approx { plan: Some(p), h: None });
            let (t, _) = riwi(truth.0, truth.1, p.0, p.1);
            shots.push(shot(n, norm400(t - o), None));
        }
        let st = Setup { station: "S".into(), ih: 0.0, remark: String::new(), shots, line: 0 };
        approximate_plan(&[st], &mut pts);
        let p = pts["S"].plan.unwrap();
        assert!((p.0 - truth.0).abs() < 1e-4 && (p.1 - truth.1).abs() < 1e-4, "{:?}", p);
    }
}
