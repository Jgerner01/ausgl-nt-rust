//! Import von CAPLAN-Datensätzen (*.N, Cremer Programmentwicklung).
//!
//! Aufbau der Datei:
//!   `!`-Zeilen        Kommentar
//!   Punktblock        Name  W  Rechtswert  Hochwert  Höhe  | Objektart | Attribute
//!   `T`               nächste Zeile = Titel
//!   `F` / `f`         Festpunkte: Name  Komponente (1 = Rechtswert, 2 = Hochwert, 3 = Höhe)
//!   `E` / `e`         Horizontalstrecken: Von  Nach  Strecke[m]  σ[m]
//!   `R` / `r`         Richtungssätze: Zeile mit Standpunkt, darunter Ziel  Richtung[gon]  σ[gon]
//!   `H` / `h`         Höhenunterschiede: Von  Nach  dH[m]  σ[m]
//!   `B` / `b`         GNSS-Basislinien (ECEF) – werden nicht ausgewertet
//! Ein Abschnitt beginnt mit einer Zeile, die nur aus dem Kennbuchstaben in Spalte 1 besteht. Groß- und
//! Kleinschreibung des Kennbuchstabens werden gleich behandelt.
//! Die Standardabweichungen je Beobachtung werden übernommen (Shot::sigma_hz/sigma_dist, NivLine::sigma).

use crate::adjust::Variant;
use crate::io::{KorPoint, NetKind, NivLine, Setup, Shot};
use std::collections::BTreeMap;

/// Komponenten-Nummern im Festpunktabschnitt
const KOMP_Y: u8 = 1;
const KOMP_X: u8 = 2;
const KOMP_H: u8 = 3;

/// Ergebnis des Imports
#[derive(Clone, Debug, Default)]
pub struct CaplanData {
    pub title: String,
    /// Festpunkte in der Lage (Höhe gesetzt, wenn auch die Höhe fest ist)
    pub fixed: Vec<KorPoint>,
    /// Näherungskoordinaten aller übrigen Punkte
    pub approx: Vec<KorPoint>,
    /// Punkte, bei denen nur die Höhe fest ist (Koordinaten in `approx`)
    pub hfix: Vec<String>,
    pub setups: Vec<Setup>,
    pub niv: Vec<NivLine>,
    /// Anzahl gelesener Richtungen, Strecken, Höhenunterschiede
    pub n_dir: usize,
    pub n_dist: usize,
    pub n_dh: usize,
    /// Hinweise zum Import (übergangene Abschnitte usw.)
    pub notes: Vec<String>,
}

impl CaplanData {
    /// passende Netzart: Lage + Höhe, wenn Höhenunterschiede vorhanden sind
    pub fn kind(&self) -> NetKind {
        let plan = self.n_dir + self.n_dist > 0;
        match (plan, self.n_dh > 0) {
            (true, true) => NetKind::LageHoehe,
            (false, true) => NetKind::Niv,
            _ => NetKind::Lage,
        }
    }

    /// ohne Festpunkte wird als freies Netz gerechnet
    pub fn variant(&self) -> Variant {
        if self.fixed.is_empty() && self.hfix.is_empty() {
            Variant::FreiesNetz
        } else {
            Variant::Normal
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} Festpunkte, {} Neupunkte, {} Richtungen, {} Strecken, {} Höhenunterschiede",
            self.fixed.len() + self.hfix.len(),
            self.approx.len() - self.hfix.len(),
            self.n_dir,
            self.n_dist,
            self.n_dh
        )
    }
}

fn num(s: &str) -> Option<f64> {
    s.replace(',', ".").parse::<f64>().ok()
}

/// Abschnittskennung: Zeile besteht nur aus einem Buchstaben in Spalte 1
/// (Punktnamen stehen rechtsbündig eingerückt – ein Standpunkt „N“ oder „E“ ist keine Kennung)
fn section_letter(line: &str) -> Option<char> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let t = line.trim_end();
    let mut c = t.chars();
    match (c.next(), c.next()) {
        (Some(ch), None) if ch.is_ascii_alphabetic() => Some(ch.to_ascii_uppercase()),
        _ => None,
    }
}

/// CAPLAN-Datensatz lesen
pub fn parse(text: &str) -> CaplanData {
    let mut d = CaplanData::default();
    // Punktname → (Y, X, H)
    let mut coords: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut fix: BTreeMap<String, [bool; 3]> = BTreeMap::new();
    let mut dist_setups: BTreeMap<String, Setup> = BTreeMap::new();
    let mut skipped: BTreeMap<char, usize> = BTreeMap::new();
    let mut section = 'P';
    let mut want_title = false;
    let mut n_baseline = 0usize;

    for (ln, raw) in text.lines().enumerate() {
        let line_no = ln + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim_start().starts_with('!') {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if want_title {
            d.title = line.trim().to_string();
            want_title = false;
            continue;
        }
        if let Some(c) = section_letter(line) {
            section = c;
            want_title = c == 'T';
            continue;
        }
        // Attribute hinter '|' abtrennen
        let data = line.split('|').next().unwrap_or("");
        let t: Vec<&str> = data.split_whitespace().collect();
        match section {
            'P' => {
                // Name W Y X H  (W = Wichtungs-/Punktartkennung)
                let (y, x, h) = match t.len() {
                    n if n >= 5 => (num(t[2]), num(t[3]), num(t[4])),
                    4 => (num(t[1]), num(t[2]), num(t[3])),
                    _ => (None, None, None),
                };
                if let (Some(y), Some(x)) = (y, x) {
                    if !coords.contains_key(t[0]) {
                        order.push(t[0].to_string());
                    }
                    coords.insert(t[0].to_string(), (y, x, h.unwrap_or(0.0)));
                }
            }
            'F' => {
                if let (Some(name), Some(k)) = (t.first(), t.get(1).and_then(|s| s.parse::<u8>().ok())) {
                    let e = fix.entry(name.to_string()).or_default();
                    match k {
                        KOMP_Y => e[0] = true,
                        KOMP_X => e[1] = true,
                        KOMP_H => e[2] = true,
                        _ => {}
                    }
                }
            }
            'E' => {
                if let (Some(from), Some(to), Some(s)) = (t.first(), t.get(1), t.get(2).and_then(|s| num(s))) {
                    let sig = t.get(3).and_then(|s| num(s)).filter(|v| *v > 0.0);
                    let st = dist_setups.entry(from.to_string()).or_insert_with(|| Setup {
                        station: from.to_string(),
                        ih: 0.0,
                        remark: "Strecken (CAPLAN E)".into(),
                        shots: vec![],
                        line: line_no,
                    });
                    st.shots.push(Shot {
                        target: to.to_string(),
                        hz: None,
                        dist: Some(s),
                        dist_horizontal: true,
                        zenith: None,
                        dh: None,
                        th: 0.0,
                        code: "00".into(),
                        line: line_no,
                        sigma_hz: None,
                        sigma_dist: sig,
                        sigma_v: None,
                    });
                    d.n_dist += 1;
                }
            }
            'R' => {
                if t.len() == 1 {
                    d.setups.push(Setup { station: t[0].to_string(), ih: 0.0, remark: "Richtungssatz (CAPLAN R)".into(), shots: vec![], line: line_no });
                } else if let (Some(st), Some(hz)) = (d.setups.last_mut(), t.get(1).and_then(|s| num(s))) {
                    let sig = t.get(2).and_then(|s| num(s)).filter(|v| *v > 0.0);
                    st.shots.push(Shot {
                        target: t[0].to_string(),
                        hz: Some(hz),
                        dist: None,
                        dist_horizontal: false,
                        zenith: None,
                        dh: None,
                        th: 0.0,
                        code: "00".into(),
                        line: line_no,
                        sigma_hz: sig,
                        sigma_dist: None,
                        sigma_v: None,
                    });
                    d.n_dir += 1;
                }
            }
            'H' => {
                if let (Some(from), Some(to), Some(dh)) = (t.first(), t.get(1), t.get(2).and_then(|s| num(s))) {
                    let sig = t.get(3).and_then(|s| num(s)).filter(|v| *v > 0.0);
                    d.niv.push(NivLine { from: from.to_string(), to: to.to_string(), dh, s: 0.0, line: line_no, sigma: sig });
                    d.n_dh += 1;
                }
            }
            'B' => {
                // Basislinie: Zeile "Von Nach dX σ" (dY, dZ folgen in eigenen Zeilen)
                if t.len() == 4 && num(t[2]).is_some() {
                    n_baseline += 1;
                }
            }
            c => *skipped.entry(c).or_default() += 1,
        }
    }
    d.setups.retain(|s| !s.shots.is_empty());
    d.setups.extend(dist_setups.into_values());

    // Strecke der Höhenunterschiede aus den Koordinaten (Anzeige)
    for n in &mut d.niv {
        if let (Some(a), Some(b)) = (coords.get(&n.from), coords.get(&n.to)) {
            n.s = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
        } else {
            n.s = 1.0;
        }
    }

    let with_h = d.n_dh > 0;
    for name in &order {
        let (y, x, h) = coords[name];
        let f = fix.get(name).copied().unwrap_or_default();
        let plan_fixed = f[0] && f[1];
        if f[0] != f[1] {
            d.notes.push(format!("Punkt {}: nur eine Lagekomponente fest – wird als Neupunkt gerechnet.", name));
        }
        if plan_fixed {
            d.fixed.push(KorPoint { name: name.clone(), y, x, h: if f[2] { Some(h) } else { None }, code: "00".into() });
        } else {
            if f[2] {
                d.hfix.push(name.clone());
            }
            d.approx.push(KorPoint { name: name.clone(), y, x, h: if with_h || f[2] { Some(h) } else { None }, code: if f[2] { "HP".into() } else { "NP".into() } });
        }
    }
    for name in fix.keys() {
        if !coords.contains_key(name) {
            d.notes.push(format!("Festpunkt {} hat keine Koordinaten im Punktblock.", name));
        }
    }
    if n_baseline > 0 {
        d.notes.push(format!("{} GNSS-Basislinien (Abschnitt B) wurden nicht übernommen – AUSGL-NT wertet keine Basislinien aus.", n_baseline));
    }
    for (c, n) in skipped {
        d.notes.push(format!("Abschnitt {} ({} Zeilen) ist unbekannt und wurde übergangen.", c, n));
    }
    if d.fixed.is_empty() && d.hfix.is_empty() {
        d.notes.push("Keine Festpunkte (Abschnitt F) – Berechnung als freies Netz.".into());
    } else if with_h && !d.fixed.iter().any(|p| p.h.is_some()) && d.hfix.is_empty() {
        d.notes.push("Keine Höhenfestpunkte – die Höhenausgleichung braucht mindestens einen Festpunkt mit Höhe (oder freies Netz).".into());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const BSP1: &str = "!---\r\n!     Punkt-Name W    Rechtswert      Hochwert         HM-vhe |Objektart|Attribute...\r\n\
               N 7      100.0000      250.0000      0.00000 |\r\n\
              S1 7      100.0000      100.0000      0.00000 |\r\n\
              S2 7      100.0000      400.0000      0.00000 |\r\n\
\r\nT\r\nBsp 1 - Freie Stationierung\r\n\r\nF\r\n\
              S1         1\r\n              S1         2\r\n              S2         1\r\n              S2         2\r\n\
\r\nE\r\n               N              S1    150.0000   0.00200\r\n               N              S2    150.0000   0.00200\r\n\
\r\nR\r\n               N\r\n                              S1   200.00000  0.000600\r\n                              S2     0.00000  0.000600\r\n";

    #[test]
    fn freie_stationierung() {
        let d = parse(BSP1);
        assert_eq!(d.title, "Bsp 1 - Freie Stationierung");
        assert_eq!(d.fixed.len(), 2);
        assert_eq!(d.approx.len(), 1);
        assert_eq!(d.approx[0].name, "N");
        assert_eq!((d.n_dir, d.n_dist, d.n_dh), (2, 2, 0));
        assert_eq!(d.kind(), NetKind::Lage);
        assert_eq!(d.variant(), Variant::Normal);
        // Richtungssatz zuerst, dann Streckensatz
        assert_eq!(d.setups.len(), 2);
        assert_eq!(d.setups[0].shots[0].sigma_hz, Some(0.0006));
        assert_eq!(d.setups[1].shots[1].sigma_dist, Some(0.002));
        assert!(d.setups[1].shots[1].hz.is_none());
    }

    #[test]
    fn kleinbuchstaben_hoehen_basislinien() {
        let t = "  A 7 0 0 10.0 |\n  E 7 100 0 11.0 |\n\nf\n A 1\n A 2\n A 3\n\nh\n A E 1.002 0.002\n\nB\n  A 7 1 2 3\n  A  E  -14.4529 0.007\n   94.8550   0.00302\n  -4.1556   0.00616\n";
        let d = parse(t);
        assert_eq!(d.fixed[0].h, Some(10.0));
        assert_eq!(d.approx[0].h, Some(11.0));
        assert_eq!(d.niv[0].sigma, Some(0.002));
        assert!((d.niv[0].s - 100.0).abs() < 1e-9);
        assert_eq!(d.kind(), NetKind::Niv);
        assert!(d.notes.iter().any(|n| n.contains("1 GNSS-Basislinien")));
    }

    /// Alle Seminar-Datensätze rechnen (cargo test --release -- --ignored --nocapture testdaten)
    #[test]
    #[ignore]
    fn testdaten() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Testdaten/Seminar Qualität von geodätischen Netzen - Dateien");
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |e| e == "N")).collect();
        files.sort();
        for f in files {
            let d = parse(&crate::io::read_text(&f).unwrap());
            let mut s = crate::adjust::Settings::default();
            s.variant = d.variant();
            let inp = crate::adjust::Input { kind: d.kind(), kor: &d.fixed, lagekor: &[], kon: &[], approx: &d.approx, hfix: &d.hfix, setups: &d.setups, niv: &d.niv, disabled: &[] };
            println!("=== {} – {}", f.file_name().unwrap().to_string_lossy(), d.summary());
            for n in &d.notes {
                println!("    Hinweis: {}", n);
            }
            match crate::adjust::run(&inp, &s) {
                Ok(o) => {
                    for p in &o.parts {
                        println!("  {}: n={} u={} f={} s0={:.3} Iter={} konv={}", p.title, p.n_obs, p.n_unknown, p.redundancy, p.sigma0, p.iterations, p.converged);
                        for q in p.points.iter().filter(|q| q.plan_adj || q.h_adj) {
                            println!("    {:>6} Y={:.4} X={:.4} H={:.4} sy={:.1} sx={:.1} sh={:.1} mm", q.name, q.y, q.x, q.h, q.sy * 1e3, q.sx * 1e3, q.sh * 1e3);
                        }
                    }
                }
                Err(e) => println!("  FEHLER: {}", e),
            }
        }
    }
}
