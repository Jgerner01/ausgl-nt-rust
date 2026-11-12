//! Ausgleichung nach vermittelnden Beobachtungen (Gauß-Markov), neu geschrieben nach dem Rechenweg
//! von ULAGE.PAS (Lagenetz), UNIT7.PAS (Höhennetz) und UVORWS.PAS (räumliche Vorwärtsschnitte):
//!
//! * Beobachtungen: Richtungen (gon, je Standpunkt eine Orientierungsunbekannte), Horizontal- und
//!   Schrägstrecken, Zenitwinkel, trig. Höhenunterschiede, Nivellement, Altpunkte als Beobachtung
//! * Varianten wie im Original: normal, Maßstabsfaktor, Altpunkte als Beobachtungen (+ Maßstab),
//!   freies Netz (Lagerung über alle Punkte, Gesamtspurminimierung)
//! * wählbare Lösungsverfahren (linalg), iterativ (Gauß-Newton) bis zur Konvergenz
//! * Statistik: s0, Redundanzanteile (# / ## wie im Original), normierte Verbesserungen (Baarda),
//!   geschätzte grobe Fehler, Fehlerellipsen
//! * optional robuste Schätzung (IRLS) zum Aufdecken von Ausreißern

use crate::approx::{shot_dh, shot_hdist, Approx, PointMap};
use crate::geo::{hstr, norm400, riwi, wrap200, RHO, RM};
use crate::io::{KorPoint, NivLine, Setup};
use crate::linalg::{self, Mat, Solver};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Variant {
    #[default]
    Normal,
    Massstab,
    AltpunkteBeob,
    AltpunkteBeobMassstab,
    FreiesNetz,
}

impl Variant {
    pub const ALL: [Variant; 5] = [Variant::Normal, Variant::Massstab, Variant::AltpunkteBeob, Variant::AltpunkteBeobMassstab, Variant::FreiesNetz];
    pub fn label(&self) -> &'static str {
        match self {
            Variant::Normal => "1. Normale Ausgleichung",
            Variant::Massstab => "2. Maßstabsfaktor für die Strecken",
            Variant::AltpunkteBeob => "3. Altpunkte als Beobachtungen",
            Variant::AltpunkteBeobMassstab => "4. Altpunkte als Beob. + Maßstab",
            Variant::FreiesNetz => "5. Freie Netzausgleichung",
        }
    }
    pub fn scale(&self) -> bool {
        matches!(self, Variant::Massstab | Variant::AltpunkteBeobMassstab)
    }
    pub fn ap_obs(&self) -> bool {
        matches!(self, Variant::AltpunkteBeob | Variant::AltpunkteBeobMassstab)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Robust {
    #[default]
    Keine,
    Huber,
    Daenisch,
    IggIII,
    Tukey,
    L1,
}

impl Robust {
    pub const ALL: [Robust; 6] = [Robust::Keine, Robust::Huber, Robust::Daenisch, Robust::IggIII, Robust::Tukey, Robust::L1];
    pub fn label(&self) -> &'static str {
        match self {
            Robust::Keine => "keine (klassisch, L2)",
            Robust::Huber => "Huber",
            Robust::Daenisch => "Dänische Methode (Krarup)",
            Robust::IggIII => "IGG-III (Yang)",
            Robust::Tukey => "Tukey (Biweight)",
            Robust::L1 => "L1-Norm (IRLS)",
        }
    }
    /// übliche Abstimmkonstante
    pub fn default_k(&self) -> f64 {
        match self {
            Robust::Keine => 0.0,
            Robust::Huber => 1.5,
            Robust::Daenisch => 2.0,
            Robust::IggIII => 1.5,
            Robust::Tukey => 4.685,
            Robust::L1 => 0.0,
        }
    }
    /// Gewichtsfaktor zur standardisierten Verbesserung u
    pub fn weight(&self, u: f64, k: f64) -> f64 {
        let a = u.abs();
        match self {
            Robust::Keine => 1.0,
            Robust::Huber => {
                if a <= k {
                    1.0
                } else {
                    k / a
                }
            }
            Robust::Daenisch => {
                if a <= k {
                    1.0
                } else {
                    (-(a / k).powi(2) + 1.0).exp()
                }
            }
            Robust::IggIII => {
                let k1 = 3.0 * k;
                if a <= k {
                    1.0
                } else if a <= k1 {
                    k / a * ((k1 - a) / (k1 - k)).powi(2)
                } else {
                    0.0
                }
            }
            Robust::Tukey => {
                if a < k {
                    (1.0 - (a / k).powi(2)).powi(2)
                } else {
                    0.0
                }
            }
            Robust::L1 => 1.0 / a.max(0.05),
        }
    }
}

/// Einstellungen der Ausgleichung (a-priori-Genauigkeiten wie FORM5/FORM8/FORM14 des Originals)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub variant: Variant,
    pub solver: Solver,
    pub robust: Robust,
    /// Abstimmkonstante; 0 = Standardwert des Schätzers
    pub robust_k: f64,
    /// Richtungen: Standardabweichung in mgon (Original fr = 0.0005 gon)
    pub sigma_dir_mgon: f64,
    /// Strecken: konstanter Anteil in mm (Original fs = 0.005 m)
    pub sigma_dist_mm: f64,
    /// Strecken: streckenabhängiger Anteil in mm/km (Original fs1 = 3)
    pub sigma_dist_ppm: f64,
    /// Zenitwinkel in mgon
    pub sigma_zen_mgon: f64,
    /// Altpunkte als Beobachtung: Lage in mm (Original fk = 0.01 m)
    pub sigma_fix_mm: f64,
    /// Altpunkte als Beobachtung: Höhe in mm (Original fk = 0.005 m bei Höhen)
    pub sigma_fix_h_mm: f64,
    /// Nivellement: mm für 1 km (Original frh = 0.005 m, Gewicht 1/s)
    pub sigma_niv_mm_km: f64,
    /// trig. Höhenunterschiede: mm je km Zielweite (Gewicht 1/s² wie Original)
    pub sigma_trig_mm_km: f64,
    /// Richtungsgewicht entfernungsabhängig (Original gew_dist, 0 = aus)
    pub dir_weight_dist: f64,
    /// Strecken wegen Höhe und Gauß-Krüger reduzieren
    pub reduce: bool,
    /// mittlere Höhe Hm in m
    pub hm: f64,
    /// Abstand vom Mittelmeridian Ym in m
    pub ym: f64,
    /// Grenzwerte vor der Ausgleichung (Original: 10·fr1 gon, fl1 m, 0.5 m)
    pub prescreen: bool,
    pub limit_dir_gon: f64,
    pub limit_dist_m: f64,
    pub limit_h_m: f64,
    pub max_iter: usize,
    /// kritischer Wert für normierte Verbesserungen (Baarda, α = 0.1 % → 3.29)
    pub k_crit: f64,
    /// Nachkommastellen Koordinaten
    pub nk: usize,
    /// Fehlerellipsen mit s0 a posteriori (sonst a priori = 1)
    pub ellipse_aposteriori: bool,
    /// Näherungskoordinaten aus vorhandener *.KON übernehmen
    pub use_kon: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            variant: Variant::Normal,
            solver: Solver::GaussJordan,
            robust: Robust::Keine,
            robust_k: 0.0,
            sigma_dir_mgon: 0.5,
            sigma_dist_mm: 5.0,
            sigma_dist_ppm: 3.0,
            sigma_zen_mgon: 1.0,
            sigma_fix_mm: 10.0,
            sigma_fix_h_mm: 5.0,
            sigma_niv_mm_km: 5.0,
            sigma_trig_mm_km: 10.0,
            dir_weight_dist: 0.0,
            reduce: false,
            hm: 0.0,
            ym: 0.0,
            prescreen: true,
            limit_dir_gon: 0.2,
            limit_dist_m: 0.3,
            limit_h_m: 0.5,
            max_iter: 15,
            k_crit: 3.29,
            nk: 3,
            ellipse_aposteriori: true,
            use_kon: true,
        }
    }
}

impl Settings {
    pub fn k(&self) -> f64 {
        if self.robust_k > 0.0 {
            self.robust_k
        } else {
            self.robust.default_k()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObsKind {
    Dir,
    HDist,
    SDist,
    Zenith,
    DhTrig,
    DhNiv,
    CoordY,
    CoordX,
    CoordH,
}

impl ObsKind {
    pub fn label(&self) -> &'static str {
        match self {
            ObsKind::Dir => "Richtung",
            ObsKind::HDist => "Horiz.strecke",
            ObsKind::SDist => "Schrägstrecke",
            ObsKind::Zenith => "Zenitwinkel",
            ObsKind::DhTrig => "trig. dH",
            ObsKind::DhNiv => "Niv. dH",
            ObsKind::CoordY => "Altpunkt Y",
            ObsKind::CoordX => "Altpunkt X",
            ObsKind::CoordH => "Altpunkt H",
        }
    }
    pub fn angular(&self) -> bool {
        matches!(self, ObsKind::Dir | ObsKind::Zenith)
    }
    /// Einheit der Verbesserungen in Tabellen
    pub fn unit(&self) -> &'static str {
        if self.angular() {
            "mgon"
        } else {
            "mm"
        }
    }
    pub fn key_suffix(&self) -> &'static str {
        match self {
            ObsKind::Dir => "R",
            ObsKind::HDist | ObsKind::SDist => "S",
            ObsKind::Zenith | ObsKind::DhTrig => "V",
            ObsKind::DhNiv => "N",
            ObsKind::CoordY => "Y",
            ObsKind::CoordX => "X",
            ObsKind::CoordH => "H",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NPoint {
    pub name: String,
    pub y: f64,
    pub x: f64,
    pub h: f64,
    pub has_plan: bool,
    pub has_h: bool,
    /// Altpunkt (aus *.KOR) in der Lage bzw. Höhe
    pub fix_plan: bool,
    pub fix_h: bool,
    /// gegebene Koordinaten (Altpunkte) bzw. Näherung
    pub y0: f64,
    pub x0: f64,
    pub h0: f64,
    pub code: String,
}

#[derive(Clone, Debug)]
pub struct Obs {
    pub kind: ObsKind,
    pub from: usize,
    pub to: usize,
    pub setup: usize,
    pub value: f64,
    pub sigma: f64,
    pub ih: f64,
    pub th: f64,
    pub key: String,
    pub line: usize,
    pub active: bool,
    /// gemessene/abgeleitete Strecke für Anzeige und Gewicht
    pub dist: f64,
    /// Reduktion der Strecke (m), nur zur Anzeige (Korr.s im Original)
    pub corr: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Net {
    pub pts: Vec<NPoint>,
    pub obs: Vec<Obs>,
    pub setup_station: Vec<usize>,
    pub plan: bool,
    pub height: bool,
    pub title: String,
}

impl Net {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.pts.iter().position(|p| p.name == name)
    }
}

#[derive(Clone, Debug)]
pub struct ObsResult {
    pub obs: usize,
    pub v: f64,
    pub redundancy: f64,
    /// normierte Verbesserung (Baarda, a priori)
    pub nv: f64,
    /// geschätzter grober Fehler
    pub gf: f64,
    /// Standardabweichung der ausgeglichenen Beobachtung
    pub sigma_adj: f64,
    pub sigma_apriori: f64,
    /// robuster Gewichtsfaktor (1 = volles Gewicht)
    pub w: f64,
    pub outlier: bool,
    pub adjusted: f64,
}

impl ObsResult {
    /// Kennzeichen der Kontrollierbarkeit wie im Original: ## r ≤ 0.03, # r ≤ 0.1
    pub fn mark(&self) -> &'static str {
        if self.redundancy <= 0.03 {
            "##"
        } else if self.redundancy <= 0.1 {
            "#"
        } else {
            ""
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PointResult {
    pub idx: usize,
    pub name: String,
    pub y: f64,
    pub x: f64,
    pub h: f64,
    pub plan_adj: bool,
    pub h_adj: bool,
    pub fix_plan: bool,
    pub fix_h: bool,
    pub sy: f64,
    pub sx: f64,
    pub sh: f64,
    /// Helmert-Ellipse: Halbachsen (m) und Richtungswinkel der großen Halbachse (gon)
    pub ea: f64,
    pub eb: f64,
    pub et: f64,
    /// Änderung gegenüber Näherung bzw. Altkoordinate
    pub dy: f64,
    pub dx: f64,
    pub dh: f64,
}

#[derive(Clone, Debug, Default)]
pub struct PartResult {
    pub title: String,
    pub net: Net,
    pub obs: Vec<ObsResult>,
    pub points: Vec<PointResult>,
    pub orient: Vec<(String, f64, f64)>,
    pub scale_ppm: Option<(f64, f64)>,
    pub n_obs: usize,
    pub n_unknown: usize,
    pub defect: usize,
    pub redundancy: usize,
    pub sigma0: f64,
    pub pvv: f64,
    pub iterations: usize,
    pub converged: bool,
    pub robust_iterations: usize,
    pub cond: Option<f64>,
    pub chi2_ok: Option<bool>,
    pub struck: Vec<(String, String)>,
    pub notes: Vec<String>,
    pub solver: Solver,
    pub robust: Robust,
    pub variant: Variant,
}

impl PartResult {
    pub fn outliers(&self) -> usize {
        self.obs.iter().filter(|o| o.outlier).count()
    }
}

// ---------------------------------------------------------------- Netz aufbauen

pub struct Input<'a> {
    pub kind: crate::io::NetKind,
    pub kor: &'a [KorPoint],
    pub lagekor: &'a [KorPoint],
    pub kon: &'a [KorPoint],
    pub setups: &'a [Setup],
    pub niv: &'a [NivLine],
    pub disabled: &'a [String],
}

fn dist_sigma(s: &Settings, d: f64) -> f64 {
    (s.sigma_dist_mm + s.sigma_dist_ppm * d / 1000.0) / 1000.0
}

/// Näherungs- und Altpunkte zusammenstellen, Näherungskoordinaten rechnen
pub fn build_points(inp: &Input, s: &Settings, need_plan: bool, need_h: bool, log: &mut Vec<String>) -> PointMap {
    let mut pm = PointMap::new();
    for p in inp.lagekor {
        pm.insert(p.name.clone(), Approx { plan: Some((p.y, p.x)), h: None });
    }
    for p in inp.kor {
        let e = pm.entry(p.name.clone()).or_default();
        e.plan = Some((p.y, p.x));
        if p.h.is_some() {
            e.h = p.h;
        }
    }
    if s.use_kon {
        for p in inp.kon {
            let e = pm.entry(p.name.clone()).or_default();
            if e.plan.is_none() {
                e.plan = Some((p.y, p.x));
            }
            if e.h.is_none() {
                e.h = p.h;
            }
        }
    }
    if need_plan {
        log.extend(crate::approx::approximate_plan(inp.setups, &mut pm));
    }
    if need_h {
        log.extend(crate::approx::approximate_heights(inp.setups, inp.niv, &mut pm));
    }
    pm
}

/// Beobachtungsschlüssel (für Streichen): Datei-Zeile + Art
pub fn obs_key(prefix: char, line: usize, k: ObsKind) -> String {
    format!("{}{}:{}", prefix, line, k.key_suffix())
}

/// Netz für eine Teilausgleichung aufbauen.
/// `plan`/`height`: welche Koordinaten unbekannt sein können; `trig_dh`: Höhenunterschiede aus Zenitwinkeln.
pub fn build_net(inp: &Input, s: &Settings, pm: &PointMap, plan: bool, height: bool, mode3d: bool, title: &str) -> Result<Net, String> {
    let mut net = Net { plan, height, title: title.into(), ..Default::default() };
    let mut idx: HashMap<String, usize> = HashMap::new();
    let fixed_plan: HashMap<&str, &KorPoint> = inp.kor.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut get = |net: &mut Net, name: &str| -> usize {
        if let Some(&i) = idx.get(name) {
            return i;
        }
        let a = pm.get(name).copied().unwrap_or_default();
        let k = fixed_plan.get(name);
        let (y, x) = a.plan.unwrap_or((f64::NAN, f64::NAN));
        let h = a.h.unwrap_or(f64::NAN);
        net.pts.push(NPoint {
            name: name.into(),
            y,
            x,
            h,
            has_plan: a.plan.is_some(),
            has_h: a.h.is_some(),
            fix_plan: k.is_some(),
            fix_h: k.map_or(false, |k| k.h.is_some()),
            y0: y,
            x0: x,
            h0: h,
            code: k.map(|k| k.code.clone()).unwrap_or_else(|| "NP".into()),
        });
        idx.insert(name.into(), net.pts.len() - 1);
        net.pts.len() - 1
    };
    let dis = |k: &str| inp.disabled.iter().any(|d| d == k);
    let red_factor = |d: f64| -> f64 {
        if s.reduce {
            // Original up7: dsh = hm/rm*s, dsgk = ym²/(2 rm²)*s; s_red = s - dsh + dsgk
            -s.hm / RM * d + s.ym * s.ym / (2.0 * RM * RM) * d
        } else {
            0.0
        }
    };
    let kind = inp.kind;
    let vorw = kind == crate::io::NetKind::Vorwaerts;

    if kind.uses_tachy() {
        for st in inp.setups {
            let si = get(&mut net, &st.station);
            let setup = net.setup_station.len();
            net.setup_station.push(si);
            for sh in &st.shots {
                if sh.target == st.station {
                    continue;
                }
                let ti = get(&mut net, &sh.target);
                let hd = shot_hdist(sh);
                let pa = &net.pts[si];
                let pb = &net.pts[ti];
                let s_coord = if pa.has_plan && pb.has_plan { Some(riwi(pa.y, pa.x, pb.y, pb.x).1) } else { None };
                let dist_any = hd.or(s_coord).unwrap_or(100.0);
                let push = |net: &mut Net, k: ObsKind, value: f64, sigma: f64, dist: f64, corr: f64| {
                    let key = obs_key('D', sh.line, k);
                    let active = !dis(&key);
                    net.obs.push(Obs { kind: k, from: si, to: ti, setup, value, sigma, ih: st.ih, th: sh.th, key, line: sh.line, active, dist, corr });
                };
                if plan {
                    if let Some(hz) = sh.hz {
                        let mut sig = s.sigma_dir_mgon / 1000.0;
                        if s.dir_weight_dist > 0.0 {
                            // Original: Gewicht sqrt(s / gew_dist)
                            sig /= (dist_any / s.dir_weight_dist).sqrt().sqrt();
                        }
                        push(&mut net, ObsKind::Dir, norm400(hz), sig, dist_any, 0.0);
                    }
                    if !vorw {
                        if let Some(d) = sh.dist {
                            if mode3d && !sh.dist_horizontal {
                                let c = red_factor(d);
                                push(&mut net, ObsKind::SDist, d + c, dist_sigma(s, d), d, c);
                            } else if let Some(h) = hd {
                                let c = red_factor(h);
                                push(&mut net, ObsKind::HDist, h + c, dist_sigma(s, h), h, c);
                            }
                        }
                    }
                }
                if height {
                    if mode3d {
                        if let (Some(z), false) = (sh.zenith, sh.dist_horizontal) {
                            push(&mut net, ObsKind::Zenith, z, s.sigma_zen_mgon / 1000.0, dist_any, 0.0);
                        } else if let (Some(dh), true) = (sh.dh, sh.dist_horizontal) {
                            let sig = trig_sigma(s, dist_any);
                            push(&mut net, ObsKind::DhTrig, dh, sig, dist_any, 0.0);
                        }
                    } else if let Some(dh) = shot_dh(sh, st.ih, s_coord.map(|sc| sc - red_factor(sc))) {
                        let sig = trig_sigma(s, dist_any);
                        push(&mut net, ObsKind::DhTrig, dh, sig, dist_any, 0.0);
                    }
                }
            }
        }
    }
    if height && !vorw {
        for n in inp.niv {
            let a = get(&mut net, &n.from);
            let b = get(&mut net, &n.to);
            let key = obs_key('N', n.line, ObsKind::DhNiv);
            let active = !dis(&key);
            let sig = s.sigma_niv_mm_km / 1000.0 * (n.s / 1000.0).sqrt().max(0.01);
            net.obs.push(Obs { kind: ObsKind::DhNiv, from: a, to: b, setup: usize::MAX, value: n.dh, sigma: sig, ih: 0.0, th: 0.0, key, line: n.line, active, dist: n.s, corr: 0.0 });
        }
    }
    // Altpunkte als Beobachtung
    if s.variant.ap_obs() {
        let n = net.pts.len();
        for i in 0..n {
            let p = net.pts[i].clone();
            if plan && p.fix_plan {
                for (k, v) in [(ObsKind::CoordY, p.y0), (ObsKind::CoordX, p.x0)] {
                    let key = format!("P{}:{}", p.name, k.key_suffix());
                    let active = !dis(&key);
                    net.obs.push(Obs { kind: k, from: i, to: i, setup: usize::MAX, value: v, sigma: s.sigma_fix_mm / 1000.0, ih: 0.0, th: 0.0, key, line: 0, active, dist: 0.0, corr: 0.0 });
                }
            }
            if height && p.fix_h {
                let key = format!("P{}:H", p.name);
                let active = !dis(&key);
                net.obs.push(Obs { kind: ObsKind::CoordH, from: i, to: i, setup: usize::MAX, value: p.h0, sigma: s.sigma_fix_h_mm / 1000.0, ih: 0.0, th: 0.0, key, line: 0, active, dist: 0.0, corr: 0.0 });
            }
        }
    }
    // Punkte ohne Näherung melden
    let mut missing = Vec::new();
    for (i, p) in net.pts.iter().enumerate() {
        let used_plan = plan && net.obs.iter().any(|o| o.active && (o.from == i || o.to == i) && matches!(o.kind, ObsKind::Dir | ObsKind::HDist | ObsKind::SDist | ObsKind::Zenith));
        let used_h = height && net.obs.iter().any(|o| o.active && (o.from == i || o.to == i) && matches!(o.kind, ObsKind::Zenith | ObsKind::DhTrig | ObsKind::DhNiv | ObsKind::SDist));
        if (used_plan && !p.has_plan) || (used_h && !p.has_h && height) {
            missing.push(p.name.clone());
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "Für folgende Punkte konnten keine Näherungswerte bestimmt werden: {}.\nBitte Beobachtungen ergänzen oder Näherungskoordinaten in einer *.KON-Datei angeben.",
            missing.join(", ")
        ));
    }
    Ok(net)
}

// ---------------------------------------------------------------- Ausgleichung

struct Params {
    iy: Vec<Option<usize>>,
    ix: Vec<Option<usize>>,
    ih: Vec<Option<usize>>,
    io: Vec<Option<usize>>,
    imu: Option<usize>,
    n: usize,
}

#[derive(Clone)]
struct State {
    y: Vec<f64>,
    x: Vec<f64>,
    h: Vec<f64>,
    o: Vec<f64>,
    mu: f64,
}

fn eval(o: &Obs, st: &State) -> f64 {
    let (a, b) = (o.from, o.to);
    let k_r = 1.0 - crate::geo::K_REFR;
    match o.kind {
        ObsKind::Dir => norm400(riwi(st.y[a], st.x[a], st.y[b], st.x[b]).0 - st.o[o.setup]),
        ObsKind::HDist => riwi(st.y[a], st.x[a], st.y[b], st.x[b]).1 * (1.0 + st.mu * 1e-6),
        ObsKind::SDist => {
            let s = riwi(st.y[a], st.x[a], st.y[b], st.x[b]).1;
            let dz = (st.h[b] + o.th) - (st.h[a] + o.ih) - s * s / (2.0 * RM);
            (s * s + dz * dz).sqrt() * (1.0 + st.mu * 1e-6)
        }
        ObsKind::Zenith => {
            let s = riwi(st.y[a], st.x[a], st.y[b], st.x[b]).1;
            let dz = (st.h[b] + o.th) - (st.h[a] + o.ih) - k_r * s * s / (2.0 * RM);
            norm400(s.atan2(dz) * RHO)
        }
        ObsKind::DhTrig | ObsKind::DhNiv => st.h[b] - st.h[a],
        ObsKind::CoordY => st.y[a],
        ObsKind::CoordX => st.x[a],
        ObsKind::CoordH => st.h[a],
    }
}

fn diff(o: &Obs, a: f64, b: f64) -> f64 {
    if o.kind.angular() {
        wrap200(a - b)
    } else {
        a - b
    }
}

/// eine Zeile der Designmatrix (Ableitungen numerisch, zentrale Differenzen)
fn design_row(o: &Obs, st: &State, p: &Params, row: &mut [f64]) {
    for v in row.iter_mut() {
        *v = 0.0;
    }
    let hstep = 1e-4;
    let pts = if o.from == o.to { vec![o.from] } else { vec![o.from, o.to] };
    let mut tmp = st.clone();
    for &pi in &pts {
        for c in 0..3 {
            let idx = match c {
                0 => p.iy[pi],
                1 => p.ix[pi],
                _ => p.ih[pi],
            };
            let Some(j) = idx else { continue };
            let orig = match c {
                0 => st.y[pi],
                1 => st.x[pi],
                _ => st.h[pi],
            };
            let set = |t: &mut State, v: f64| match c {
                0 => t.y[pi] = v,
                1 => t.x[pi] = v,
                _ => t.h[pi] = v,
            };
            set(&mut tmp, orig + hstep);
            let fp = eval(o, &tmp);
            set(&mut tmp, orig - hstep);
            let fm = eval(o, &tmp);
            set(&mut tmp, orig);
            row[j] += diff(o, fp, fm) / (2.0 * hstep);
        }
    }
    if o.kind == ObsKind::Dir {
        if let Some(j) = p.io[o.setup] {
            row[j] = -1.0;
        }
    }
    if matches!(o.kind, ObsKind::HDist | ObsKind::SDist) {
        if let Some(j) = p.imu {
            let mut t = st.clone();
            t.mu = st.mu + 1.0;
            row[j] = eval(o, &t) - eval(o, st);
        }
    }
}

fn involved(o: &Obs, i: usize) -> bool {
    o.from == i || o.to == i
}

/// Ausgleichung eines Netzes
pub fn adjust(mut net: Net, s: &Settings) -> Result<PartResult, String> {
    let np = net.pts.len();
    let free = s.variant == Variant::FreiesNetz;
    let mut struck: Vec<(String, String)> = Vec::new();
    let mut notes = Vec::new();

    // Orientierungen (t-r) aus Näherungen
    let nsetup = net.setup_station.len();
    let mut st = State { y: net.pts.iter().map(|p| p.y).collect(), x: net.pts.iter().map(|p| p.x).collect(), h: net.pts.iter().map(|p| p.h).collect(), o: vec![0.0; nsetup], mu: 0.0 };
    let compute_orient = |net: &Net, st: &mut State| {
        for k in 0..nsetup {
            let mut v = Vec::new();
            for o in net.obs.iter().filter(|o| o.active && o.kind == ObsKind::Dir && o.setup == k) {
                let (t, d) = riwi(st.y[o.from], st.x[o.from], st.y[o.to], st.x[o.to]);
                if d > 0.0 && t.is_finite() {
                    v.push(norm400(t - o.value));
                }
            }
            if !v.is_empty() {
                let mut w: Vec<f64> = v.iter().map(|a| wrap200(a - v[0])).collect();
                w.sort_by(|a, b| a.partial_cmp(b).unwrap());
                st.o[k] = norm400(v[0] + w[w.len() / 2]);
            }
        }
    };
    compute_orient(&net, &mut st);

    // Grenzwertprüfung vor der Ausgleichung (Original: "Richtung gestrichen" / "Strecke gestrichen")
    if s.prescreen {
        for o in net.obs.iter_mut().filter(|o| o.active) {
            let f = eval(o, &st);
            if !f.is_finite() {
                continue;
            }
            let w = diff(o, o.value, f).abs();
            let (lim, txt) = match o.kind {
                ObsKind::Dir | ObsKind::Zenith => (s.limit_dir_gon, format!("{:.4} gon", w)),
                ObsKind::HDist | ObsKind::SDist => (s.limit_dist_m, format!("{:.3} m", w)),
                ObsKind::DhTrig | ObsKind::DhNiv => (s.limit_h_m, format!("{:.3} m", w)),
                _ => (f64::MAX, String::new()),
            };
            if w > lim {
                o.active = false;
                struck.push((o.key.clone(), format!("{} gestrichen (Widerspruch {} > Grenzwert)", o.kind.label(), txt)));
            }
        }
    }

    // Unbekannte festlegen
    let has_dist = net.obs.iter().any(|o| o.active && matches!(o.kind, ObsKind::HDist | ObsKind::SDist));
    let mut p = Params { iy: vec![None; np], ix: vec![None; np], ih: vec![None; np], io: vec![None; nsetup], imu: None, n: 0 };
    for i in 0..np {
        let pt = &net.pts[i];
        let used_plan = net.plan && net.obs.iter().any(|o| o.active && involved(o, i) && matches!(o.kind, ObsKind::Dir | ObsKind::HDist | ObsKind::SDist | ObsKind::Zenith | ObsKind::CoordX | ObsKind::CoordY));
        let used_h = net.height && net.obs.iter().any(|o| o.active && involved(o, i) && matches!(o.kind, ObsKind::Zenith | ObsKind::DhTrig | ObsKind::DhNiv | ObsKind::CoordH | ObsKind::SDist));
        let release = free || s.variant.ap_obs();
        if used_plan && pt.has_plan && (!pt.fix_plan || release) {
            p.iy[i] = Some(p.n);
            p.ix[i] = Some(p.n + 1);
            p.n += 2;
        }
        if used_h && pt.has_h && (!pt.fix_h || release) {
            p.ih[i] = Some(p.n);
            p.n += 1;
        }
    }
    for k in 0..nsetup {
        if net.obs.iter().any(|o| o.active && o.kind == ObsKind::Dir && o.setup == k) {
            p.io[k] = Some(p.n);
            p.n += 1;
        }
    }
    if s.variant.scale() && has_dist {
        p.imu = Some(p.n);
        p.n += 1;
    }
    if s.variant.scale() && !has_dist {
        notes.push("Maßstabsfaktor nicht bestimmbar: keine Strecken".into());
    }

    // Datum des freien Netzes: Nullraum G und Bedingungen B (nur Koordinaten)
    let mut g_cols: Vec<Vec<f64>> = Vec::new();
    if free {
        let plan_pts: Vec<usize> = (0..np).filter(|&i| p.iy[i].is_some()).collect();
        let h_pts: Vec<usize> = (0..np).filter(|&i| p.ih[i].is_some()).collect();
        if !plan_pts.is_empty() {
            let n = plan_pts.len() as f64;
            let ys = plan_pts.iter().map(|&i| st.y[i]).sum::<f64>() / n;
            let xs = plan_pts.iter().map(|&i| st.x[i]).sum::<f64>() / n;
            let mut ty = vec![0.0; p.n];
            let mut tx = vec![0.0; p.n];
            let mut rot = vec![0.0; p.n];
            for &i in &plan_pts {
                ty[p.iy[i].unwrap()] = 1.0;
                tx[p.ix[i].unwrap()] = 1.0;
                rot[p.iy[i].unwrap()] = st.x[i] - xs;
                rot[p.ix[i].unwrap()] = -(st.y[i] - ys);
            }
            for k in 0..nsetup {
                if let Some(j) = p.io[k] {
                    rot[j] = RHO;
                }
            }
            g_cols.push(ty);
            g_cols.push(tx);
            let has_dir = net.obs.iter().any(|o| o.active && o.kind == ObsKind::Dir);
            if has_dir || true {
                g_cols.push(rot);
            }
            if !has_dist {
                let mut sc = vec![0.0; p.n];
                for &i in &plan_pts {
                    sc[p.iy[i].unwrap()] = st.y[i] - ys;
                    sc[p.ix[i].unwrap()] = st.x[i] - xs;
                }
                let hn = h_pts.len().max(1) as f64;
                let hs = h_pts.iter().map(|&i| st.h[i]).sum::<f64>() / hn;
                for &i in &h_pts {
                    sc[p.ih[i].unwrap()] = st.h[i] - hs;
                }
                g_cols.push(sc);
                notes.push("freies Netz ohne Strecken: Maßstab über Datum gelagert".into());
            }
        }
        if !h_pts.is_empty() {
            let mut th = vec![0.0; p.n];
            for &i in &h_pts {
                th[p.ih[i].unwrap()] = 1.0;
            }
            g_cols.push(th);
        }
    }
    let defect = g_cols.len();
    // B: G nur auf Koordinaten, Spalten normiert
    let coord_mask: Vec<bool> = {
        let mut m = vec![false; p.n];
        for i in 0..np {
            for j in [p.iy[i], p.ix[i], p.ih[i]].into_iter().flatten() {
                m[j] = true;
            }
        }
        m
    };
    let b_cols: Vec<Vec<f64>> = g_cols
        .iter()
        .map(|g| {
            let v: Vec<f64> = g.iter().enumerate().map(|(j, &x)| if coord_mask[j] { x } else { 0.0 }).collect();
            let nrm = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
            v.iter().map(|x| x / nrm).collect()
        })
        .collect();

    let active: Vec<usize> = (0..net.obs.len()).filter(|&i| net.obs[i].active).collect();
    let m = active.len();
    if p.n == 0 {
        return Err("Keine Unbekannten: alle Punkte sind Altpunkte oder ohne Beobachtungen.".into());
    }
    if m + defect <= p.n {
        return Err(format!("keine Überbestimmung – Ausgleichung nicht möglich ({} Beobachtungen, {} Unbekannte)", m, p.n));
    }

    let mut wf = vec![1.0f64; net.obs.len()];
    let robust = s.robust != Robust::Keine;
    let k = s.k();
    let mut iterations = 0;
    let mut converged = false;
    let mut robust_iterations = 0;
    let mut sol_q = Mat::zeros(0, 0);
    let mut cond = None;
    let mut a_rows = Mat::zeros(m, p.n);

    let max_outer = if robust { 40 } else { 1 };
    for outer in 0..max_outer {
        // Gauß-Newton
        let mut it_conv = false;
        for _ in 0..s.max_iter.max(1) {
            iterations += 1;
            let extra = defect;
            let mut a = Mat::zeros(m + extra, p.n);
            let mut l = vec![0.0; m + extra];
            let mut row = vec![0.0; p.n];
            for (r, &oi) in active.iter().enumerate() {
                let o = &net.obs[oi];
                design_row(o, &st, &p, &mut row);
                let sw = wf[oi].max(1e-6).sqrt() / o.sigma;
                for j in 0..p.n {
                    a[(r, j)] = row[j] * sw;
                }
                l[r] = diff(o, o.value, eval(o, &st)) * sw;
            }
            // Datumsbedingungen (freies Netz) als zusätzliche Zeilen: AᵀA + c²BBᵀ
            let mut c2 = 1.0;
            if extra > 0 {
                let nd: f64 = (0..p.n).map(|j| (0..m).map(|r| a[(r, j)] * a[(r, j)]).sum::<f64>()).sum::<f64>() / p.n as f64;
                c2 = nd.max(1e-12);
                for (q, b) in b_cols.iter().enumerate() {
                    for j in 0..p.n {
                        a[(m + q, j)] = b[j] * c2.sqrt();
                    }
                }
            }
            let sol = linalg::solve(s.solver, &a, &l).map_err(|e| e.0)?;
            cond = sol.cond;
            let mut q = sol.qxx;
            if extra > 0 {
                // Q_B = M⁻¹ − G (Gᵀ B Bᵀ G)⁻¹ Gᵀ   (B mit c skaliert)
                let d = g_cols.len();
                let mut gtb = Mat::zeros(d, d);
                for (i, g) in g_cols.iter().enumerate() {
                    for (j, b) in b_cols.iter().enumerate() {
                        gtb[(i, j)] = g.iter().zip(b).map(|(x, y)| x * y).sum::<f64>() * c2.sqrt();
                    }
                }
                let mm = gtb.mul(&gtb.t());
                let mi = linalg::invert_gauss_jordan(&mm).map_err(|e| e.0)?;
                for i in 0..p.n {
                    for j in 0..p.n {
                        let mut acc = 0.0;
                        for u in 0..d {
                            if g_cols[u][i] == 0.0 {
                                continue;
                            }
                            for w in 0..d {
                                acc += g_cols[u][i] * mi[(u, w)] * g_cols[w][j];
                            }
                        }
                        q[(i, j)] -= acc;
                    }
                }
            }
            sol_q = q;
            a_rows = a;
            // Zuschläge anbringen
            let mut maxd: f64 = 0.0;
            for i in 0..np {
                if let Some(j) = p.iy[i] {
                    st.y[i] += sol.dx[j];
                    maxd = maxd.max(sol.dx[j].abs());
                }
                if let Some(j) = p.ix[i] {
                    st.x[i] += sol.dx[j];
                    maxd = maxd.max(sol.dx[j].abs());
                }
                if let Some(j) = p.ih[i] {
                    st.h[i] += sol.dx[j];
                    maxd = maxd.max(sol.dx[j].abs());
                }
            }
            for kk in 0..nsetup {
                if let Some(j) = p.io[kk] {
                    st.o[kk] = norm400(st.o[kk] + sol.dx[j]);
                    maxd = maxd.max(sol.dx[j].abs() / RHO * 100.0);
                }
            }
            if let Some(j) = p.imu {
                st.mu += sol.dx[j];
            }
            if !maxd.is_finite() {
                return Err("Ausgleichung divergiert (Näherungswerte prüfen)".into());
            }
            if maxd < 1e-5 {
                it_conv = true;
                break;
            }
        }
        converged = it_conv;
        if !robust {
            break;
        }
        // robuste Neugewichtung
        robust_iterations = outer + 1;
        let mut u = Vec::with_capacity(m);
        for (r, &oi) in active.iter().enumerate() {
            let o = &net.obs[oi];
            let v = diff(o, eval(o, &st), o.value);
            let aq = qform(&a_rows, r, &sol_q);
            let red = (1.0 - aq).clamp(1e-4, 1.0);
            // standardisiert mit a-priori-Sigma (ohne Robustgewicht) und Redundanz
            u.push(v / (o.sigma * red.sqrt()));
        }
        let mut absu: Vec<f64> = u.iter().map(|x| x.abs()).collect();
        absu.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // robuste Streuung (MAD); nie kleiner als die a-priori-Genauigkeit, damit bei kleinen
        // Netzen keine regulären Messungen abgewertet werden
        let mad = 1.4826 * absu[absu.len() / 2];
        let scale = mad.max(1.0);
        let mut change: f64 = 0.0;
        for (r, &oi) in active.iter().enumerate() {
            let nw = s.robust.weight(u[r] / scale, k).max(1e-4);
            change = change.max((nw - wf[oi]).abs());
            wf[oi] = nw;
        }
        if change < 0.002 {
            break;
        }
    }

    // ---- Ergebnisse
    let redundancy = m + defect - p.n;
    let mut pvv = 0.0;
    let mut obs_res = Vec::with_capacity(m);
    for (r, &oi) in active.iter().enumerate() {
        let o = &net.obs[oi];
        let f = eval(o, &st);
        let v = diff(o, f, o.value);
        let pw = wf[oi] / (o.sigma * o.sigma);
        pvv += pw * v * v;
        let aq = qform(&a_rows, r, &sol_q);
        let red = (1.0 - aq).clamp(0.0, 1.0);
        obs_res.push(ObsResult { obs: oi, v, redundancy: red, nv: 0.0, gf: 0.0, sigma_adj: 0.0, sigma_apriori: o.sigma, w: wf[oi], outlier: false, adjusted: f });
    }
    let sigma0 = if redundancy > 0 { (pvv / redundancy as f64).sqrt() } else { f64::NAN };
    for (r, res) in obs_res.iter_mut().enumerate() {
        let o = &net.obs[res.obs];
        let aq = qform(&a_rows, r, &sol_q);
        // σ der ausgeglichenen Beobachtung (a posteriori), Gewichtsfaktor herausgerechnet
        res.sigma_adj = sigma0 * o.sigma / wf[res.obs].max(1e-6).sqrt() * aq.max(0.0).sqrt();
        res.nv = if res.redundancy > 1e-6 { res.v.abs() / (o.sigma * res.redundancy.sqrt()) } else { 0.0 };
        res.gf = if res.redundancy > 1e-6 { -res.v / res.redundancy } else { 0.0 };
        res.outlier = res.nv > s.k_crit || (robust && res.w < 0.1 && res.nv > 2.0);
    }
    let chi2_ok = if redundancy > 0 {
        let (lo, hi) = chi2_bounds(redundancy as f64);
        let t = pvv;
        Some(t >= lo && t <= hi)
    } else {
        None
    };
    let s0e = if s.ellipse_aposteriori && sigma0.is_finite() { sigma0 } else { 1.0 };
    let mut points = Vec::new();
    for i in 0..np {
        let pt = &net.pts[i];
        let (plan_adj, h_adj) = (p.iy[i].is_some(), p.ih[i].is_some());
        let mut pr = PointResult {
            idx: i,
            name: pt.name.clone(),
            y: st.y[i],
            x: st.x[i],
            h: st.h[i],
            plan_adj,
            h_adj,
            fix_plan: pt.fix_plan,
            fix_h: pt.fix_h,
            dy: st.y[i] - pt.y0,
            dx: st.x[i] - pt.x0,
            dh: st.h[i] - pt.h0,
            ..Default::default()
        };
        if let (Some(jy), Some(jx)) = (p.iy[i], p.ix[i]) {
            let (qyy, qxx, qxy) = (sol_q[(jy, jy)], sol_q[(jx, jx)], sol_q[(jy, jx)]);
            pr.sy = s0e * qyy.max(0.0).sqrt();
            pr.sx = s0e * qxx.max(0.0).sqrt();
            let w = ((qxx - qyy).powi(2) + 4.0 * qxy * qxy).sqrt();
            pr.ea = s0e * ((qyy + qxx + w) / 2.0).max(0.0).sqrt();
            pr.eb = s0e * ((qyy + qxx - w) / 2.0).max(0.0).sqrt();
            pr.et = norm400(0.5 * (2.0 * qxy).atan2(qxx - qyy) * RHO);
        }
        if let Some(jh) = p.ih[i] {
            pr.sh = s0e * sol_q[(jh, jh)].max(0.0).sqrt();
        }
        points.push(pr);
    }
    let mut orient = Vec::new();
    for kk in 0..nsetup {
        if let Some(j) = p.io[kk] {
            orient.push((net.pts[net.setup_station[kk]].name.clone(), st.o[kk], sigma0 * sol_q[(j, j)].max(0.0).sqrt() * 1000.0));
        }
    }
    let scale_ppm = p.imu.map(|j| (st.mu, sigma0 * sol_q[(j, j)].max(0.0).sqrt()));
    // Koordinaten zurück ins Netz
    for i in 0..np {
        net.pts[i].y = st.y[i];
        net.pts[i].x = st.x[i];
        net.pts[i].h = st.h[i];
    }
    Ok(PartResult {
        title: net.title.clone(),
        net,
        obs: obs_res,
        points,
        orient,
        scale_ppm,
        n_obs: m,
        n_unknown: p.n,
        defect,
        redundancy,
        sigma0,
        pvv,
        iterations,
        converged,
        robust_iterations,
        cond,
        chi2_ok,
        struck,
        notes,
        solver: s.solver,
        robust: s.robust,
        variant: s.variant,
    })
}

/// a_r · Q · a_rᵀ (gewichtete Zeile)
fn qform(a: &Mat, r: usize, q: &Mat) -> f64 {
    let row = a.row(r);
    let n = q.r;
    let mut acc = 0.0;
    for i in 0..n {
        if row[i] == 0.0 {
            continue;
        }
        let mut t = 0.0;
        for j in 0..n {
            t += q[(i, j)] * row[j];
        }
        acc += row[i] * t;
    }
    acc
}

/// zweiseitiger χ²-Test (α = 5 %) der Varianz: Grenzen für vᵀPv (Wilson-Hilferty)
pub fn chi2_bounds(f: f64) -> (f64, f64) {
    let z = 1.959_964;
    let c = |zz: f64| f * (1.0 - 2.0 / (9.0 * f) + zz * (2.0 / (9.0 * f)).sqrt()).powi(3);
    (c(-z).max(0.0), c(z))
}

// ---------------------------------------------------------------- Gesamtablauf

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub parts: Vec<PartResult>,
    pub approx_log: Vec<String>,
    pub kind: crate::io::NetKind,
}

impl Outcome {
    /// alle Punkte mit Lage und Höhe zusammengeführt (Name → (y,x,h, Lage ausgeglichen, Höhe ausgeglichen))
    pub fn merged_points(&self) -> Vec<PointResult> {
        let mut out: Vec<PointResult> = Vec::new();
        for part in &self.parts {
            for p in &part.points {
                if let Some(q) = out.iter_mut().find(|q| q.name == p.name) {
                    if p.h_adj {
                        q.h = p.h;
                        q.sh = p.sh;
                        q.h_adj = true;
                        q.dh = p.dh;
                    }
                    if p.plan_adj {
                        q.y = p.y;
                        q.x = p.x;
                        q.sy = p.sy;
                        q.sx = p.sx;
                        q.ea = p.ea;
                        q.eb = p.eb;
                        q.et = p.et;
                        q.plan_adj = true;
                        q.dy = p.dy;
                        q.dx = p.dx;
                    }
                    q.fix_h |= p.fix_h;
                } else {
                    out.push(p.clone());
                }
            }
        }
        out
    }
}

/// Komplette Berechnung für eine Projektart
pub fn run(inp: &Input, s: &Settings) -> Result<Outcome, String> {
    use crate::io::NetKind::*;
    let mut log = Vec::new();
    let mut parts = Vec::new();
    match inp.kind {
        Lage => {
            let pm = build_points(inp, s, true, false, &mut log);
            let net = build_net(inp, s, &pm, true, false, false, "Lageausgleichung")?;
            parts.push(adjust(net, s)?);
        }
        Niv | Trig => {
            let pm = build_points(inp, s, false, true, &mut log);
            let title = if inp.kind == Niv { "Höhenausgleichung (Nivellement)" } else { "Höhenausgleichung (trig. Höhen)" };
            let mut s2 = s.clone();
            if s2.variant.scale() {
                s2.variant = if s2.variant.ap_obs() { Variant::AltpunkteBeob } else { Variant::Normal };
            }
            let net = build_net(inp, &s2, &pm, false, true, false, title)?;
            parts.push(adjust(net, &s2)?);
        }
        LageHoehe => {
            let mut pm = build_points(inp, s, true, true, &mut log);
            let net = build_net(inp, s, &pm, true, false, false, "Lageausgleichung (2D)")?;
            let lage = adjust(net, s)?;
            // ausgeglichene Lage als Grundlage der Höhenausgleichung
            for p in &lage.points {
                if let Some(e) = pm.get_mut(&p.name) {
                    e.plan = Some((p.y, p.x));
                }
            }
            let mut s2 = s.clone();
            if s2.variant.scale() {
                s2.variant = if s2.variant.ap_obs() { Variant::AltpunkteBeob } else { Variant::Normal };
            }
            let netkind = Input { kind: Trig, ..*inp };
            let hnet = build_net(&netkind, &s2, &pm, false, true, false, "Höhenausgleichung (trig. Höhen + Niv.)")?;
            parts.push(lage);
            if hnet.obs.iter().any(|o| o.active) {
                match adjust(hnet, &s2) {
                    Ok(h) => parts.push(h),
                    Err(e) => log.push(format!("Höhenausgleichung nicht möglich: {}", e)),
                }
            }
        }
        Raum3D => {
            let pm = build_points(inp, s, true, true, &mut log);
            let net = build_net(inp, s, &pm, true, true, true, "Räumliche Ausgleichung (3D)")?;
            parts.push(adjust(net, s)?);
        }
        Vorwaerts => {
            let pm = build_points(inp, s, true, true, &mut log);
            let mut s2 = s.clone();
            if s2.variant != Variant::Normal {
                log.push("Vorwärtsschnitte werden mit festen Standpunkten gerechnet (Variante 1)".into());
                s2.variant = Variant::Normal;
            }
            let net = build_net(inp, &s2, &pm, true, true, true, "Räumliche Vorwärtsschnitte")?;
            parts.push(adjust(net, &s2)?);
        }
    }
    Ok(Outcome { parts, approx_log: log, kind: inp.kind })
}

/// Höhe/Zenit-Hilfsfunktion für Tests und Protokoll
#[allow(dead_code)]
pub fn horizontal(d: f64, z: f64) -> f64 {
    hstr(d, z).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::NetKind;

    #[test]
    fn sample_network_recovers_truth_all_solvers() {
        let smp = crate::sample::Sample::generate(false);
        for kind in [NetKind::Lage, NetKind::LageHoehe, NetKind::Raum3D, NetKind::Niv] {
            for solver in Solver::ALL {
                let s = Settings { solver, ..Default::default() };
                let inp = smp.input(kind);
                let out = run(&inp, &s).unwrap_or_else(|e| panic!("{:?} {:?}: {}", kind, solver, e));
                for p in out.merged_points() {
                    let t = smp.truth(&p.name);
                    if p.plan_adj {
                        assert!((p.y - t.0).abs() < 0.02 && (p.x - t.1).abs() < 0.02, "{:?} {:?} {} {:?} {:?}", kind, solver, p.name, (p.y, p.x), t);
                    }
                    if p.h_adj {
                        assert!((p.h - t.2).abs() < 0.03, "{:?} {:?} H {} {} {}", kind, solver, p.name, p.h, t.2);
                    }
                }
                let s0 = out.parts[0].sigma0;
                assert!(s0 > 0.2 && s0 < 3.0, "{:?} {:?} s0 = {}", kind, solver, s0);
            }
        }
    }

    #[test]
    fn variants_run() {
        let smp = crate::sample::Sample::generate(false);
        for v in Variant::ALL {
            for kind in [NetKind::Lage, NetKind::Raum3D, NetKind::Niv] {
                let s = Settings { variant: v, ..Default::default() };
                let out = run(&smp.input(kind), &s).unwrap_or_else(|e| panic!("{:?} {:?}: {}", v, kind, e));
                let sum_r: f64 = out.parts[0].obs.iter().map(|o| o.redundancy).sum();
                assert!((sum_r - out.parts[0].redundancy as f64).abs() < 1e-3, "{:?} {:?} Σr={} r={}", v, kind, sum_r, out.parts[0].redundancy);
            }
        }
    }

    #[test]
    fn robust_finds_gross_error() {
        let smp = crate::sample::Sample::generate(true);
        for rb in [Robust::Huber, Robust::Daenisch, Robust::IggIII, Robust::Tukey, Robust::L1] {
            let s = Settings { robust: rb, prescreen: false, ..Default::default() };
            let out = run(&smp.input(NetKind::Lage), &s).unwrap();
            let part = &out.parts[0];
            let g = part.obs.iter().find(|o| part.net.obs[o.obs].line == smp.gross_line && part.net.obs[o.obs].kind == ObsKind::Dir).unwrap();
            let flagged: Vec<String> = part.obs.iter().filter(|o| o.outlier).map(|o| format!("{} {:?} w={:.3} nv={:.2}", part.net.obs[o.obs].line, part.net.obs[o.obs].kind, o.w, o.nv)).collect();
            assert!(g.outlier, "{:?}: {:?}", rb, flagged);
            assert!(flagged.len() <= 2, "{:?}: {:?}", rb, flagged);
            let minw = part.obs.iter().map(|o| o.w).fold(f64::MAX, f64::min);
            assert!(g.w <= minw + 1e-9, "{:?} w={} min={} {:?}", rb, g.w, minw, flagged);
        }
        // klassisch: Baarda findet ihn auch
        let out = run(&smp.input(NetKind::Lage), &Settings { prescreen: false, ..Default::default() }).unwrap();
        let worst = out.parts[0].obs.iter().max_by(|a, b| a.nv.partial_cmp(&b.nv).unwrap()).unwrap();
        assert_eq!(out.parts[0].net.obs[worst.obs].line, smp.gross_line);
    }
}

/// σ eines trig. Höhenunterschieds: Gewicht 1/s² wie UNIT7.PAS up1 bzw. AUSGL.EXE 0x45e158
/// (ge = c0/s²/frh²), d. h. σ proportional zur Zielweite; σ_km gilt für 1 km.
pub fn trig_sigma(s: &Settings, dist: f64) -> f64 {
    s.sigma_trig_mm_km / 1000.0 * dist.max(1.0) / 1000.0
}
