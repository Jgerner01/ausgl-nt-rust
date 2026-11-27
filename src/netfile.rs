//! AUSGL-NT-Netzdatei (*.NET): Punkte und Messungen in einer lesbaren ASCII-Datei –
//! eine Mischung aus KOR- und DAT-Datei. Felder durch Leerzeichen getrennt, '#' leitet Kommentare ein.
//!
//! Jede Zeile beginnt mit einem Kennbuchstaben:
//!   P  Punkt  Rechtswert  Hochwert  Höhe  Art  [Code]
//!        Art: F = Lage und Höhe fest, L = nur Lage fest, H = nur Höhe fest, N = Neupunkt (Näherung)
//!   S  Standpunkt  Instrumentenhöhe  [Bemerkung …]
//!   Z  Ziel  Hz[gon]  Strecke[m]  V[gon]/dH[m]  Zielhöhe  [Code]  [Status]  [σHz σS σV]
//!        Strecke mit '*' = Horizontalstrecke, dann steht in der V-Spalte der Höhenunterschied
//!        Status: drei Zeichen R S H – Richtung / Strecke / Höhe verwendet, '-' an der Stelle = gestrichen
//!        σHz [mgon], σS [mm], σV [mgon beim Zenitwinkel, mm beim Höhenunterschied]
//!   N  Von  Nach  dH[m]  Strecke[m]  [Status H/-]  [σdH mm]
//! '-' steht für „kein Wert“ bzw. „Voreinstellung“. Z-Zeilen gehören zur letzten S-Zeile.
//! Beim Einlesen werden fehlerhafte Zeilen übergangen und als Hinweis gemeldet; der Rest bleibt gültig.
//! Gestrichene Beobachtungen werden intern über Schlüssel aus der Dateizeile geführt (`adjust::obs_key`).

use crate::adjust::{obs_key, ObsKind};
use crate::io::{KorPoint, NivLine, Setup, Shot};

/// Umrechnung der Genauigkeitsspalten mgon → gon bzw. mm → m
const MILLI: f64 = 1000.0;
/// Nachkommastellen in der Datei
const NK_KOORD: usize = 4;
const NK_WINKEL: usize = 5;
const NK_STRECKE: usize = 4;
const NK_HOEHE: usize = 4;
const NK_SIGMA: usize = 3;

/// Festlegung eines Punktes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    /// Lage und Höhe fest
    F,
    /// nur Lage fest
    L,
    /// nur Höhe fest
    H,
    /// Neupunkt
    N,
}

impl Art {
    pub const ALL: [Art; 4] = [Art::F, Art::L, Art::H, Art::N];
    pub fn letter(&self) -> &'static str {
        match self {
            Art::F => "F",
            Art::L => "L",
            Art::H => "H",
            Art::N => "N",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Art::F => "Festpunkt Lage und Höhe",
            Art::L => "Festpunkt nur Lage",
            Art::H => "Festpunkt nur Höhe",
            Art::N => "Neupunkt",
        }
    }
    fn parse(s: &str) -> Option<Art> {
        match s.to_ascii_uppercase().as_str() {
            "F" => Some(Art::F),
            "L" => Some(Art::L),
            "H" => Some(Art::H),
            "N" => Some(Art::N),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetPoint {
    pub name: String,
    pub y: Option<f64>,
    pub x: Option<f64>,
    pub h: Option<f64>,
    pub art: Art,
    pub code: String,
}

impl NetPoint {
    pub fn plan(&self) -> Option<(f64, f64)> {
        match (self.y, self.x) {
            (Some(y), Some(x)) => Some((y, x)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NetData {
    pub points: Vec<NetPoint>,
    pub setups: Vec<Setup>,
    pub niv: Vec<NivLine>,
    /// gestrichene Beobachtungen (Schlüssel wie `adjust::obs_key`)
    pub disabled: Vec<String>,
    pub notes: Vec<String>,
}

impl NetData {
    pub fn summary(&self) -> String {
        let fixed = self.points.iter().filter(|p| p.art != Art::N).count();
        format!(
            "{} Festpunkte, {} Neupunkte, {} Standpunkte, {} Zielungen, {} Höhenunterschiede",
            fixed,
            self.points.len() - fixed,
            self.setups.len(),
            self.setups.iter().map(|s| s.shots.len()).sum::<usize>(),
            self.niv.len()
        )
    }
}

fn num(s: &str) -> Option<f64> {
    if s == "-" {
        return None;
    }
    s.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Feld ist "-" oder eine Zahl
fn is_value(s: &str) -> bool {
    s == "-" || num(s).is_some()
}

/// Statusfeld einer Zielung: genau drei Zeichen R/S/H bzw. '-'
fn parse_status3(s: &str) -> Option<[bool; 3]> {
    let c: Vec<char> = s.chars().map(|c| c.to_ascii_uppercase()).collect();
    if c.len() != 3 {
        return None;
    }
    let ok = |ch: char, l: char| ch == l || ch == '-';
    if ok(c[0], 'R') && ok(c[1], 'S') && ok(c[2], 'H') {
        Some([c[0] == 'R', c[1] == 'S', c[2] == 'H'])
    } else {
        None
    }
}

/// Kommentar ab '#' (am Zeilenanfang oder nach Leerraum) abschneiden
fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'#' && (i == 0 || b[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

/// Netzdatei lesen
pub fn parse(text: &str) -> NetData {
    let mut d = NetData::default();
    let mut orphan = 0usize;
    for (ln, raw) in text.lines().enumerate() {
        let line_no = ln + 1;
        let t: Vec<&str> = strip_comment(raw).split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        let bad = |d: &mut NetData, what: &str| d.notes.push(format!("Zeile {}: {} – übergangen.", line_no, what));
        match t[0].to_ascii_uppercase().as_str() {
            "P" => {
                if t.len() < 4 {
                    bad(&mut d, "Punktzeile unvollständig (P Punkt Y X [H] [Art] [Code])");
                    continue;
                }
                if !is_value(t[2]) || !is_value(t[3]) || !t.get(4).map_or(true, |s| is_value(s)) {
                    bad(&mut d, "Koordinate nicht lesbar");
                    continue;
                }
                let art = match t.get(5) {
                    None => Art::N,
                    Some(a) => match Art::parse(a) {
                        Some(a) => a,
                        None => {
                            bad(&mut d, "Punktart muss F, L, H oder N sein");
                            continue;
                        }
                    },
                };
                let p = NetPoint {
                    name: t[1].to_string(),
                    y: num(t[2]),
                    x: num(t[3]),
                    h: t.get(4).and_then(|s| num(s)),
                    art,
                    code: t.get(6).map(|s| s.to_string()).filter(|s| s != "-").unwrap_or_default(),
                };
                if let Some(q) = d.points.iter_mut().find(|q| q.name == p.name) {
                    d.notes.push(format!("Zeile {}: Punkt {} doppelt – die letzte Zeile gilt.", line_no, p.name));
                    *q = p;
                } else {
                    d.points.push(p);
                }
            }
            "S" => {
                if t.len() < 2 {
                    bad(&mut d, "Standpunktzeile ohne Punktnummer");
                    continue;
                }
                d.setups.push(Setup {
                    station: t[1].to_string(),
                    ih: t.get(2).and_then(|s| num(s)).unwrap_or(0.0),
                    remark: t.get(3..).map(|r| r.join(" ")).unwrap_or_default(),
                    shots: vec![],
                    line: line_no,
                });
            }
            "Z" => {
                if d.setups.is_empty() {
                    orphan += 1;
                    continue;
                }
                if t.len() < 3 {
                    bad(&mut d, "Zielung ohne Messwert");
                    continue;
                }
                // die ersten vier Messfelder (Hz, Strecke, V/dH, Zielhöhe) müssen Zahlen oder '-' sein
                let dist_tok = t.get(3).copied().unwrap_or("-");
                let dist_horizontal = dist_tok.ends_with('*');
                let dist_core = dist_tok.trim_end_matches('*');
                let dist_core = if dist_core.is_empty() { "-" } else { dist_core };
                if !is_value(t[2]) || !is_value(dist_core) || !t.get(4).map_or(true, |s| is_value(s)) || !t.get(5).map_or(true, |s| is_value(s)) {
                    bad(&mut d, "Messwert nicht lesbar");
                    continue;
                }
                let hz = num(t[2]);
                let dist = num(dist_core).filter(|v| *v > 0.0);
                let v = t.get(4).and_then(|s| num(s));
                let th = t.get(5).and_then(|s| num(s)).unwrap_or(0.0);
                let mut code = String::new();
                let mut status: Option<[bool; 3]> = None;
                let mut sig: Vec<Option<f64>> = Vec::new();
                for tok in t.iter().skip(6) {
                    if status.is_none() {
                        if let Some(st) = parse_status3(tok) {
                            status = Some(st);
                            continue;
                        }
                        if code.is_empty() {
                            code = tok.to_string();
                            continue;
                        }
                    } else if is_value(tok) && sig.len() < 3 {
                        sig.push(num(tok).filter(|v| *v > 0.0).map(|v| v / MILLI));
                        continue;
                    }
                    d.notes.push(format!("Zeile {}: Feld „{}“ unbekannt – ignoriert.", line_no, tok));
                }
                if code == "-" {
                    code.clear();
                }
                let (zenith, dh) = if dist_horizontal { (None, v) } else { (v.filter(|z| *z != 0.0), None) };
                if hz.is_none() && dist.is_none() && zenith.is_none() && dh.is_none() {
                    bad(&mut d, "Zielung ohne Messwert");
                    continue;
                }
                let st = status.unwrap_or([true; 3]);
                for (on, k) in st.iter().zip([ObsKind::Dir, ObsKind::HDist, ObsKind::Zenith]) {
                    if !on {
                        d.disabled.push(obs_key('D', line_no, k));
                    }
                }
                let setup = d.setups.last_mut().expect("Standpunkt geprüft");
                setup.shots.push(Shot {
                    target: t[1].to_string(),
                    hz,
                    dist,
                    // Strecke ohne Zenitwinkel gilt als horizontal (wie DAT)
                    dist_horizontal: dist_horizontal || (dist.is_some() && zenith.is_none()),
                    zenith,
                    dh,
                    th,
                    code: if code.is_empty() { "00".into() } else { code },
                    line: line_no,
                    sigma_hz: sig.first().copied().flatten(),
                    sigma_dist: sig.get(1).copied().flatten(),
                    sigma_v: sig.get(2).copied().flatten(),
                });
            }
            "N" => {
                if t.len() < 4 || num(t[3]).is_none() || !t.get(4).map_or(true, |s| is_value(s)) {
                    bad(&mut d, "Höhenunterschied unvollständig (N Von Nach dH [Strecke])");
                    continue;
                }
                // nach der Strecke: [Status H/-] [σdH]
                let rest = &t[t.len().min(5)..];
                let mut on = true;
                let mut i = 0;
                if let Some(first) = rest.first() {
                    if first.eq_ignore_ascii_case("h") {
                        i = 1;
                    } else if *first == "-" {
                        on = false;
                        i = 1;
                    }
                }
                let mut sigma = None;
                let mut sigma_read = false;
                for tok in &rest[i..] {
                    if !sigma_read && is_value(tok) {
                        sigma = num(tok).filter(|v| *v > 0.0).map(|v| v / MILLI);
                        sigma_read = true;
                    } else {
                        d.notes.push(format!("Zeile {}: Feld „{}“ unbekannt – ignoriert.", line_no, tok));
                    }
                }
                if !on {
                    d.disabled.push(obs_key('N', line_no, ObsKind::DhNiv));
                }
                d.niv.push(NivLine {
                    from: t[1].to_string(),
                    to: t[2].to_string(),
                    dh: num(t[3]).unwrap_or(0.0),
                    s: t.get(4).and_then(|s| num(s)).filter(|s| *s > 0.0).unwrap_or(1.0),
                    line: line_no,
                    sigma,
                });
            }
            other => d.notes.push(format!("Zeile {}: Kennung „{}“ unbekannt (erlaubt: P, S, Z, N) – übergangen.", line_no, other)),
        }
    }
    if orphan > 0 {
        d.notes.push(format!("{} Zielungen stehen vor der ersten S-Zeile – übergangen.", orphan));
    }
    d.setups.retain(|s| !s.shots.is_empty());
    d
}

fn f_opt(v: Option<f64>, nk: usize, w: usize) -> String {
    match v {
        Some(v) => format!("{:>w$.nk$}", v, w = w, nk = nk),
        None => format!("{:>w$}", "-", w = w),
    }
}

fn sig_opt(v: Option<f64>) -> String {
    f_opt(v.map(|v| v * MILLI), NK_SIGMA, 7)
}

/// Netzdatei schreiben
pub fn write(title: &str, points: &[NetPoint], setups: &[Setup], niv: &[NivLine], disabled: &[String]) -> String {
    let off = |k: String| disabled.iter().any(|d| *d == k);
    let mut s = String::new();
    s += &format!("# AUSGL-NT Netzdatei – {}\r\n", title);
    s += "# Zeilen: P = Punkt, S = Standpunkt, Z = Zielung (gehört zur letzten S-Zeile), N = Höhenunterschied\r\n";
    s += "# '-' = kein Wert bzw. Voreinstellung des Projekts, '#' = Kommentar. Felder durch Leerzeichen getrennt.\r\n";
    s += "#\r\n";
    s += "# Art: F = Lage und Höhe fest, L = nur Lage fest, H = nur Höhe fest, N = Neupunkt (Koordinaten = Näherung)\r\n";
    s += &format!("# {:<14} {:>15} {:>15} {:>11}  Art  Code\r\n", "P Punkt", "Rechtswert", "Hochwert", "Höhe");
    for p in points {
        s += &format!(
            "P {:<12} {} {} {}  {}    {}\r\n",
            p.name,
            f_opt(p.y, NK_KOORD, 15),
            f_opt(p.x, NK_KOORD, 15),
            f_opt(p.h, NK_KOORD, 11),
            p.art.letter(),
            if p.code.is_empty() { "-" } else { &p.code }
        );
    }
    if !setups.is_empty() {
        s += "#\r\n";
        s += "# Strecke mit '*' = Horizontalstrecke, dann steht unter V der Höhenunterschied dH [m]\r\n";
        s += "# Status: R = Richtung, S = Strecke, H = Höhe/Zenit verwendet – '-' an der Stelle = gestrichen\r\n";
        s += "# σHz [mgon], σS [mm], σV [mgon bei Zenitwinkel, mm bei dH]; '-' = Voreinstellung\r\n";
        s += "# S Standpunkt   Instr.höhe  Bemerkung\r\n";
        s += &format!(
            "# {:<12} {:>11} {:>12} {:>11} {:>7}  {:<6} {}  {:>7} {:>7} {:>7}\r\n",
            "Z Ziel", "Hz", "Strecke", "V/dH", "Zielh.", "Code", "St.", "σHz", "σS", "σV"
        );
        for st in setups {
            s += &format!("S {:<12} {:>7.3}  {}\r\n", st.station, st.ih, st.remark);
            for sh in &st.shots {
                let v = if sh.dist_horizontal { f_opt(sh.dh, NK_HOEHE, 11) } else { f_opt(sh.zenith, NK_WINKEL, 11) };
                let dist = format!("{}{}", f_opt(sh.dist, NK_STRECKE, 11), if sh.dist_horizontal { "*" } else { " " });
                let flag = |k: ObsKind, c: char| if off(obs_key('D', sh.line, k)) { '-' } else { c };
                let status: String = [flag(ObsKind::Dir, 'R'), flag(ObsKind::HDist, 'S'), flag(ObsKind::Zenith, 'H')].iter().collect();
                s += &format!(
                    "Z {:<12} {} {} {} {:>7.3}  {:<6} {}  {} {} {}\r\n",
                    sh.target,
                    f_opt(sh.hz, NK_WINKEL, 11),
                    dist,
                    v,
                    sh.th,
                    if sh.code.is_empty() { "-" } else { &sh.code },
                    status,
                    sig_opt(sh.sigma_hz),
                    sig_opt(sh.sigma_dist),
                    sig_opt(sh.sigma_v)
                );
            }
        }
    }
    if !niv.is_empty() {
        s += "#\r\n";
        s += "# Status: H = verwendet, '-' = gestrichen; σdH [mm], '-' = Voreinstellung (mm je km)\r\n";
        s += &format!("# {:<12} {:<12} {:>11} {:>10}  St.  {:>7}\r\n", "N Von", "Nach", "dH", "Strecke", "σdH");
        for n in niv {
            let st = if off(obs_key('N', n.line, ObsKind::DhNiv)) { "-" } else { "H" };
            s += &format!("N {:<12} {:<12} {:>11.5} {:>10.2}  {}    {}\r\n", n.from, n.to, n.dh, n.s, st, sig_opt(n.sigma));
        }
    }
    s
}

/// Punktliste in die Eingaben der Ausgleichung aufteilen: (Festpunkte Lage, Näherungen, Höhenfestpunkte, Hinweise)
pub fn split_points(points: &[NetPoint]) -> (Vec<KorPoint>, Vec<KorPoint>, Vec<String>, Vec<String>) {
    let mut kor = Vec::new();
    let mut approx = Vec::new();
    let mut hfix = Vec::new();
    let mut notes = Vec::new();
    for p in points {
        let code = if p.code.is_empty() { "00".to_string() } else { p.code.clone() };
        let (y, x) = p.plan().unwrap_or((f64::NAN, f64::NAN));
        let mut art = p.art;
        if matches!(art, Art::F | Art::L) && p.plan().is_none() {
            notes.push(format!("Punkt {}: Festpunkt ohne Lagekoordinaten – Lage wird als Neupunkt gerechnet.", p.name));
            art = if art == Art::F { Art::H } else { Art::N };
        }
        if matches!(art, Art::F | Art::H) && p.h.is_none() {
            notes.push(format!("Punkt {}: feste Höhe ohne Höhenwert – Höhe wird als Neupunkt gerechnet.", p.name));
            art = if art == Art::F { Art::L } else { Art::N };
        }
        match art {
            Art::F => kor.push(KorPoint { name: p.name.clone(), y, x, h: p.h, code }),
            Art::L => {
                kor.push(KorPoint { name: p.name.clone(), y, x, h: None, code: code.clone() });
                if p.h.is_some() {
                    approx.push(KorPoint { name: p.name.clone(), y, x, h: p.h, code });
                }
            }
            Art::H => {
                hfix.push(p.name.clone());
                approx.push(KorPoint { name: p.name.clone(), y, x, h: p.h, code });
            }
            Art::N => approx.push(KorPoint { name: p.name.clone(), y, x, h: p.h, code }),
        }
    }
    (kor, approx, hfix, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETZ: &str = "# Kommentar\n\
P G1   1000.0000  2000.0000  100.0000  F  GP\n\
P N1   1100.0000  2000.0000  -         N\n\
p h1   -          -          101.5     H\n\
S G1 1.500 erster Satz\n\
Z N1  100.00000  100.0000*  0.5000  1.300  NP  R-H  0.50 - -\n\
Z h1   50.00000  -   -   0   # nur Richtung\n\
X kaputt\n\
N G1 h1 1.50000 120.0  -  2.0\n\
N h1 N1 -0.5 80\n";

    #[test]
    fn lesen() {
        let d = parse(NETZ);
        assert_eq!(d.points.len(), 3);
        assert_eq!(d.points[2].art, Art::H);
        assert_eq!(d.points[1].h, None);
        assert_eq!(d.setups.len(), 1);
        assert_eq!(d.setups[0].remark, "erster Satz");
        let z = &d.setups[0].shots[0];
        assert!(z.dist_horizontal);
        assert_eq!(z.dh, Some(0.5));
        assert_eq!(z.code, "NP");
        assert!((z.sigma_hz.unwrap() - 0.0005).abs() < 1e-12);
        assert_eq!(z.sigma_dist, None);
        assert!(d.disabled.contains(&obs_key('D', 6, ObsKind::HDist)));
        assert_eq!(d.setups[0].shots[1].dist, None);
        assert_eq!(d.niv.len(), 2);
        assert!(d.disabled.contains(&obs_key('N', 9, ObsKind::DhNiv)));
        assert!((d.niv[0].sigma.unwrap() - 0.002).abs() < 1e-12);
        assert!(!d.disabled.contains(&obs_key('N', 10, ObsKind::DhNiv)));
        assert!(d.notes.iter().any(|n| n.starts_with("Zeile 8")));
    }

    #[test]
    fn schreiben_und_wieder_lesen() {
        let d = parse(NETZ);
        let text = write("Test", &d.points, &d.setups, &d.niv, &d.disabled);
        let e = parse(&text);
        assert!(e.notes.is_empty(), "{:?}", e.notes);
        assert_eq!(e.points, d.points);
        assert_eq!(e.setups.len(), d.setups.len());
        for (a, b) in e.setups[0].shots.iter().zip(&d.setups[0].shots) {
            assert_eq!((a.hz, a.dist, a.dh, a.zenith, a.sigma_hz, a.sigma_dist), (b.hz, b.dist, b.dh, b.zenith, b.sigma_hz, b.sigma_dist));
        }
        // gestrichene Strecke und gestrichenes Nivellement bleiben gestrichen
        assert_eq!(e.disabled.len(), 2);
        assert_eq!(e.niv[0].sigma, d.niv[0].sigma);
    }

    #[test]
    fn punkte_aufteilen() {
        let d = parse(NETZ);
        let (kor, approx, hfix, notes) = split_points(&d.points);
        assert_eq!(kor.len(), 1);
        assert_eq!(hfix, vec!["h1".to_string()]);
        assert_eq!(approx.len(), 2);
        assert!(notes.is_empty());
    }

    /// Rundreise mit den Seminardaten: CAPLAN → Netzdatei → Ausgleichung unverändert
    /// (cargo test --release -- --ignored --nocapture rundreise; AUSGL_NET_OUT=Ordner schreibt die Dateien)
    #[test]
    #[ignore]
    fn rundreise_seminar() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Testdaten/Seminar Qualität von geodätischen Netzen - Dateien");
        for f in ["Bsp2 - Polygonzug beidseitig angeschlossen.N", "Bsp3 - Brückennetz mit 6 SP.N"] {
            let c = crate::caplan::parse(&crate::io::read_text(&dir.join(f)).unwrap());
            let mut s = crate::adjust::Settings::default();
            s.variant = c.variant();
            let run = |kor: &[KorPoint], approx: &[KorPoint], hfix: &[String], setups: &[Setup], niv: &[NivLine], dis: &[String]| {
                let inp = crate::adjust::Input { kind: c.kind(), kor, lagekor: &[], kon: &[], approx, hfix, setups, niv, disabled: dis };
                let o = crate::adjust::run(&inp, &s).unwrap();
                o.parts.iter().map(|p| (p.sigma0, p.redundancy)).collect::<Vec<_>>()
            };
            let r1 = run(&c.fixed, &c.approx, &c.hfix, &c.setups, &c.niv, &[]);
            // Punkte wie im Editor zusammenstellen
            let mut pts: Vec<NetPoint> = c.fixed.iter().map(|p| NetPoint { name: p.name.clone(), y: Some(p.y), x: Some(p.x), h: p.h, art: if p.h.is_some() { Art::F } else { Art::L }, code: String::new() }).collect();
            pts.extend(c.approx.iter().map(|p| NetPoint { name: p.name.clone(), y: Some(p.y), x: Some(p.x), h: p.h, art: Art::N, code: String::new() }));
            let text = write(f, &pts, &c.setups, &c.niv, &[]);
            if let Ok(out) = std::env::var("AUSGL_NET_OUT") {
                std::fs::write(std::path::Path::new(&out).join(f.replace(".N", ".net")), &text).unwrap();
            }
            let d = parse(&text);
            assert!(d.notes.is_empty(), "{:?}", d.notes);
            let (kor, approx, hfix, _) = split_points(&d.points);
            let r2 = run(&kor, &approx, &hfix, &d.setups, &d.niv, &d.disabled);
            println!("{}: CAPLAN {:?} / Netzdatei {:?}", f, r1, r2);
            for (a, b) in r1.iter().zip(&r2) {
                assert!((a.0 - b.0).abs() < 1e-9 && a.1 == b.1);
            }
        }
    }
}
