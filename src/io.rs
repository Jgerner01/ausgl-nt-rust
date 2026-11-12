//! Dateiformate: Punktdateien (KOR/KON, CADdy-ASCII), Meßdaten (DAT/DAM, CADdy), Projektdatei.
//!
//! Punktzeile:   Nummer  Rechtswert  Hochwert  [Höhe]  [Code]
//! Standpunkt:   -Nummer  Instrumentenhöhe  [Bemerkung]
//! Zielpunkt:    Nummer  Hz[gon]  Strecke[*]  V[gon] bzw. dH  Zielhöhe  Code
//!               (Strecke mit '*' = Horizontalstrecke, dann folgt der Höhenunterschied)
//! Nivellement:  -Nummer 0   /   Zielnummer  dH  Strecke
//! Wie UZERLEG.PAS und ZERLEGzp in ULAGE.PAS: Felder durch Leerzeichen/Tab getrennt.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct KorPoint {
    pub name: String,
    pub y: f64,
    pub x: f64,
    pub h: Option<f64>,
    pub code: String,
}

/// Zielung von einem Standpunkt
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Shot {
    pub target: String,
    /// Horizontalrichtung in gon
    pub hz: Option<f64>,
    /// Strecke (Schrägstrecke, oder Horizontalstrecke wenn `dist_horizontal`)
    pub dist: Option<f64>,
    pub dist_horizontal: bool,
    /// Zenitwinkel in gon
    pub zenith: Option<f64>,
    /// Höhenunterschied (bei Horizontalstrecke mit '*')
    pub dh: Option<f64>,
    pub th: f64,
    pub code: String,
    pub line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Setup {
    pub station: String,
    pub ih: f64,
    pub remark: String,
    pub shots: Vec<Shot>,
    pub line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NivLine {
    pub from: String,
    pub to: String,
    pub dh: f64,
    /// Nivellementsstrecke in m
    pub s: f64,
    pub line: usize,
}

fn tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ' ' || c == '\t' || c == ';').filter(|s| !s.is_empty()).collect()
}

fn num(s: &str) -> Option<f64> {
    s.trim_end_matches('*').replace(',', ".").parse::<f64>().ok()
}

/// Punktdatei lesen (KOR/KON). Zeilen mit Code '-' oder '/' an 3. Stelle werden übergangen (wie Original).
pub fn parse_kor(text: &str) -> Vec<KorPoint> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = tokens(line);
        if t.len() < 3 || line.trim_start().starts_with('#') {
            continue;
        }
        let (Some(y), Some(x)) = (num(t[1]), num(t[2])) else { continue };
        let h = t.get(3).and_then(|s| num(s)).filter(|h| *h != 0.0);
        let mut code = t.get(4).map(|s| s.to_string()).unwrap_or_else(|| "00".into());
        if code == "0" {
            code = "00".into();
        }
        if code.len() == 1 {
            code = format!("0{}", code);
        }
        if code.len() > 2 && matches!(code.as_bytes()[2], b'-' | b'/') {
            continue;
        }
        if let Some(p) = out.iter_mut().find(|p: &&mut KorPoint| p.name == t[0]) {
            *p = KorPoint { name: t[0].into(), y, x, h, code };
        } else {
            out.push(KorPoint { name: t[0].into(), y, x, h, code });
        }
    }
    out
}

/// Punktdatei schreiben (wie Schreibnp: Nummer:15, Y:12:nk, X:12:nk, H:10:nk, Code:15)
pub fn write_kor(points: &[KorPoint], nk: usize) -> String {
    let mut s = String::new();
    for p in points {
        s += &format!("{:>15}{:>14.*}{:>14.*}{:>11.*}{:>8}\r\n", p.name, nk, p.y, nk, p.x, nk, p.h.unwrap_or(0.0), p.code);
    }
    s
}

/// Tachymeter-Meßdaten lesen
pub fn parse_tachy(text: &str) -> Vec<Setup> {
    let mut out: Vec<Setup> = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let t = tokens(line);
        if t.is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(st) = t[0].strip_prefix('-') {
            if st.is_empty() {
                continue;
            }
            out.push(Setup {
                station: st.to_string(),
                ih: t.get(1).and_then(|s| num(s)).unwrap_or(0.0),
                remark: t.get(2..).map(|r| r.join(" ")).unwrap_or_default(),
                shots: vec![],
                line: ln + 1,
            });
            continue;
        }
        let Some(setup) = out.last_mut() else { continue };
        let hz = t.get(1).and_then(|s| num(s));
        let mut shot = Shot { target: t[0].to_string(), hz, dist: None, dist_horizontal: false, zenith: None, dh: None, th: 0.0, code: "00".into(), line: ln + 1 };
        if let Some(sd) = t.get(2) {
            shot.dist_horizontal = sd.ends_with('*');
            shot.dist = num(sd).filter(|d| *d > 0.00001);
        }
        if t.len() == 3 {
            // nur Richtung und Strecke: Horizontalstrecke (ken=0 im Original)
            shot.dist_horizontal = true;
        }
        if let Some(v) = t.get(3).and_then(|s| num(s)) {
            if shot.dist_horizontal {
                shot.dh = Some(v);
            } else if v != 0.0 {
                shot.zenith = Some(v);
            } else {
                // kein Zenitwinkel: Strecke als Horizontalstrecke (Original: HStr nur bei dh<>0)
                shot.dist_horizontal = true;
            }
        } else if !shot.dist_horizontal && shot.dist.is_some() {
            shot.dist_horizontal = true;
        }
        if let Some(th) = t.get(4).and_then(|s| num(s)) {
            shot.th = th;
        }
        if let Some(c) = t.get(5) {
            shot.code = c.to_string();
        }
        if shot.hz.is_none() && shot.dist.is_none() {
            continue;
        }
        setup.shots.push(shot);
    }
    out.retain(|s| !s.shots.is_empty());
    out
}

/// Nivellement lesen: -Standpunkt 0 / Ziel dH Strecke
pub fn parse_niv(text: &str) -> Vec<NivLine> {
    let mut out = Vec::new();
    let mut from: Option<String> = None;
    for (ln, line) in text.lines().enumerate() {
        let t = tokens(line);
        if t.is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(st) = t[0].strip_prefix('-') {
            from = Some(st.to_string());
            continue;
        }
        let (Some(f), Some(dh)) = (from.clone(), t.get(1).and_then(|s| num(s))) else { continue };
        let s = t.get(2).and_then(|s| num(s)).filter(|s| *s > 0.0).unwrap_or(1.0);
        out.push(NivLine { from: f, to: t[0].to_string(), dh, s, line: ln + 1 });
    }
    out
}

/// Meßdaten zurückschreiben (CADdy-Format), z. B. nach Satzmittelbildung (*.DAM)
pub fn write_tachy(setups: &[Setup]) -> String {
    let mut s = String::new();
    for st in setups {
        s += &format!("-{:<14}{:>10.3}  {}\r\n", st.station, st.ih, st.remark);
        for sh in &st.shots {
            let hz = sh.hz.map(|v| format!("{:>12.5}", v)).unwrap_or_else(|| format!("{:>12}", "0"));
            let d = match sh.dist {
                Some(d) => format!("{:>12.4}{}", d, if sh.dist_horizontal { "*" } else { " " }),
                None => format!("{:>12} ", "0"),
            };
            let v = if sh.dist_horizontal { sh.dh.map(|v| format!("{:>12.4}", v)) } else { sh.zenith.map(|v| format!("{:>12.5}", v)) }.unwrap_or_else(|| format!("{:>12}", "0"));
            s += &format!("{:<15}{}{}{}{:>9.3}  {}\r\n", sh.target, hz, d, v, sh.th, sh.code);
        }
    }
    s
}

pub fn read_text(path: &Path) -> std::io::Result<String> {
    let b = std::fs::read(path)?;
    Ok(match String::from_utf8(b.clone()) {
        Ok(s) => s,
        Err(_) => b.iter().map(|&c| cp1252(c)).collect(),
    })
}

fn cp1252(c: u8) -> char {
    match c {
        0x80 => '€',
        0x84 => '„',
        0x93 => '“',
        0x94 => '”',
        0x96 => '–',
        _ => c as char,
    }
}

/// Art der Berechnung (Projekttyp)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NetKind {
    /// Lagenetz (Richtungen und Strecken)
    #[default]
    Lage,
    /// Höhennetz aus Nivellement
    Niv,
    /// trigonometrisches Höhennetz
    Trig,
    /// 2D-Lageausgleichung kombiniert mit Höhenausgleichung
    LageHoehe,
    /// volle räumliche (3D) Ausgleichung
    Raum3D,
    /// räumliche Vorwärtsschnitte
    Vorwaerts,
}

impl NetKind {
    pub const ALL: [NetKind; 6] = [NetKind::Lage, NetKind::Niv, NetKind::Trig, NetKind::LageHoehe, NetKind::Raum3D, NetKind::Vorwaerts];
    pub fn label(&self) -> &'static str {
        match self {
            NetKind::Lage => "Lagenetz (2D)",
            NetKind::Niv => "Höhennetz – Nivellement",
            NetKind::Trig => "Höhennetz – trig. Höhen",
            NetKind::LageHoehe => "2D-Lage + Höhenausgleichung",
            NetKind::Raum3D => "Volle 3D-Ausgleichung",
            NetKind::Vorwaerts => "Räumliche Vorwärtsschnitte",
        }
    }
    pub fn uses_tachy(&self) -> bool {
        !matches!(self, NetKind::Niv)
    }
}

/// Projektbeschrieb (wie Projrec in UVAR.PAS) plus Einstellungen der neuen Version
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectFile {
    pub name: String,
    pub nummer: String,
    pub beschreibung: String,
    pub ort: String,
    pub ort1: String,
    pub bearbeiter: String,
    pub datum: String,
    pub kind: NetKind,
    /// Altpunkte (*.KOR)
    pub kor: String,
    /// Tachymeter-Meßdaten (*.DAT / *.DAM)
    pub mes: String,
    /// Nivellement (optional, für Höhen- und 3D-Netze)
    pub niv: String,
    /// Lagekoordinaten aller Punkte für trig. Höhennetze (lagekor)
    pub lagekor: String,
    pub settings: crate::adjust::Settings,
    /// gestrichene Beobachtungen (Schlüssel)
    pub disabled: Vec<String>,
}

impl ProjectFile {
    pub fn resolve(&self, base: &Path, f: &str) -> Option<PathBuf> {
        if f.trim().is_empty() {
            return None;
        }
        let p = PathBuf::from(f);
        Some(if p.is_absolute() { p } else { base.join(p) })
    }
}

/// Alte Delphi-Projektdatei (*.PRJ, 'file of projrec') so weit lesen, wie das Layout bekannt ist:
/// ShortStrings projname[15], nummer[15], beschreibung[255], ort[30], ort1[30], bearbeiter[30],
/// korpfad[255], korname[15], mespfad[255], mesname[15], ldatum[15], art:integer, lagekor[255].
pub fn import_legacy_prj(bytes: &[u8]) -> Option<ProjectFile> {
    let mut off = 0usize;
    let mut ss = |len: usize| -> Option<String> {
        let b = bytes.get(off..off + len + 1)?;
        let n = (b[0] as usize).min(len);
        off += len + 1;
        Some(b[1..1 + n].iter().map(|&c| cp1252(c)).collect::<String>().trim().to_string())
    };
    let name = ss(15)?;
    let nummer = ss(15)?;
    let beschreibung = ss(255)?;
    let ort = ss(30)?;
    let ort1 = ss(30)?;
    let bearbeiter = ss(30)?;
    let korpfad = ss(255)?;
    let korname = ss(15)?;
    let mespfad = ss(255)?;
    let mesname = ss(15)?;
    let datum = ss(15)?;
    let base = off;
    let aligned = base.div_ceil(4) * 4;
    let art = bytes.get(aligned..aligned + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
    let lk = bytes.get(aligned + 4..).and_then(|b| {
        let n = *b.first()? as usize;
        Some(b.get(1..1 + n)?.iter().map(|&c| cp1252(c)).collect::<String>())
    });
    let kind = match art {
        1 => NetKind::Niv,
        2 => NetKind::Trig,
        3 => NetKind::Vorwaerts,
        _ => NetKind::Lage,
    };
    let join = |p: &str, n: &str| if n.is_empty() { String::new() } else { format!("{}{}", p, n) };
    Some(ProjectFile {
        name,
        nummer,
        beschreibung,
        ort,
        ort1,
        bearbeiter,
        datum,
        kind,
        kor: join(&korpfad, &korname),
        mes: join(&mespfad, &mesname),
        niv: if kind == NetKind::Niv { join(&mespfad, &mesname) } else { String::new() },
        lagekor: lk.unwrap_or_default().trim().to_string(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caddy_lines() {
        let t = "-125      1.520    PP\n1251     125.1234    230.333     98.4567   1.600    GP5\n1252     126.4567   190.444*    10.123     1.600    GP5\n1253 300.0\n";
        let s = parse_tachy(t);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].station, "125");
        assert!((s[0].ih - 1.52).abs() < 1e-9);
        let a = &s[0].shots[0];
        assert_eq!(a.zenith, Some(98.4567));
        assert!(!a.dist_horizontal);
        let b = &s[0].shots[1];
        assert!(b.dist_horizontal);
        assert_eq!(b.dh, Some(10.123));
        assert_eq!(s[0].shots[2].dist, None);
    }

    #[test]
    fn niv_lines() {
        let n = parse_niv("-2400   0\n2456     -13.124     2350\n");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].from, "2400");
        assert!((n[0].dh + 13.124).abs() < 1e-12);
    }

    #[test]
    fn kor_lines() {
        let k = parse_kor("1234  4565310.18  5312520.13  513.16  015\n99 1 2 0 00-x\n");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].h, Some(513.16));
    }
}
