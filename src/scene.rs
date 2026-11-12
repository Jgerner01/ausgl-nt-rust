//! Netzbild als Szene: Weltkoordinaten (y = Rechtswert, x = Hochwert) mit Zeichengrößen in mm
//! Papier. Eine Szene wird auf dem Bildschirm, als PDF und als DXF ausgegeben (wie UNIT6.PAS
//! Fehlerellipsen und UNIT9.PAS Fehlergrafik Höhe, mit den Ebenen aus WAUSGL.SYS).

use crate::adjust::{ObsKind, Outcome};
use crate::approx::PointMap;
use crate::io::{KorPoint, NivLine, Setup};

pub type Rgb = [u8; 3];

/// Ebenen wie in WAUSGL.SYS (Ebenenvoreinstellung)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Punkte,
    Linien,
    Nummern,
    Hoehen,
    Klasse,
    Ellipsen,
    Achsen,
    Vektoren,
    Beschriftung,
    Gitter,
    Ausreisser,
}

impl Layer {
    pub const ALL: [Layer; 11] = [Layer::Punkte, Layer::Linien, Layer::Nummern, Layer::Hoehen, Layer::Klasse, Layer::Ellipsen, Layer::Achsen, Layer::Vektoren, Layer::Beschriftung, Layer::Gitter, Layer::Ausreisser];
    /// DXF-Ebenenname (Nummer aus WAUSGL.SYS + Bezeichnung)
    pub fn dxf_name(&self) -> &'static str {
        match self {
            Layer::Punkte => "11_PUNKTSYMBOLE",
            Layer::Linien => "12_LINIEN",
            Layer::Nummern => "13_PUNKTNUMMERN",
            Layer::Hoehen => "14_HOEHEN",
            Layer::Klasse => "15_KLASSE",
            Layer::Ellipsen => "16_FEHLERELLIPSEN",
            Layer::Achsen => "17_ACHSEN",
            Layer::Vektoren => "18_FEHLERVEKTOREN",
            Layer::Beschriftung => "5_BESCHRIFTUNG",
            Layer::Gitter => "21_KOORDINATENGITTER",
            Layer::Ausreisser => "19_AUSREISSER",
        }
    }
    /// CADdy-Farbe aus WAUSGL.SYS (Grafik: Farbe 1..9)
    pub fn caddy_color(&self) -> i32 {
        match self {
            Layer::Punkte => 3,
            Layer::Linien => 7,
            Layer::Nummern => 1,
            Layer::Hoehen => 6,
            Layer::Klasse => 12,
            Layer::Ellipsen => 2,
            Layer::Achsen => 4,
            Layer::Vektoren => 6,
            Layer::Beschriftung => 7,
            Layer::Gitter => 4,
            Layer::Ausreisser => 1,
        }
    }
}

/// CADdy-Farbnummer → AutoCAD-Farbindex
pub fn caddy_to_aci(c: i32) -> i32 {
    match c {
        1 => 1,
        2 => 3,
        3 => 2,
        4 => 5,
        5 => 6,
        6 => 4,
        7 => 7,
        8 => 8,
        9 => 14,
        10 => 94,
        11 => 34,
        12 => 174,
        c => c.clamp(1, 255),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    Triangle,
    Circle,
    Square,
}

#[derive(Clone, Debug)]
pub enum Item {
    Line { layer: Layer, rgb: Rgb, w_mm: f64, dash: bool, pts: Vec<[f64; 2]> },
    Point { layer: Layer, rgb: Rgb, pos: [f64; 2], z: f64, marker: Marker, size_mm: f64, filled: bool },
    Text { layer: Layer, rgb: Rgb, pos: [f64; 2], h_mm: f64, ax: f64, ay: f64, dx_mm: f64, dy_mm: f64, text: String },
    /// Fehlerellipse: Halbachsen in mm Papier, Winkel der großen Halbachse (gon, von Nord)
    Ellipse { layer: Layer, rgb: Rgb, center: [f64; 2], a_mm: f64, b_mm: f64, theta_gon: f64, axes: bool },
    /// Pfeil vom Punkt aus, Länge in mm Papier (Ost, Nord)
    Arrow { layer: Layer, rgb: Rgb, anchor: [f64; 2], de_mm: f64, dn_mm: f64 },
    /// Fehlersäule (Höhe): senkrechter Balken nach oben (+) bzw. unten
    Bar { layer: Layer, rgb: Rgb, anchor: [f64; 2], len_mm: f64, width_mm: f64 },
    /// Querstrich (Strecke gemessen): quer zur Richtung `dir` (Einheitsvektor Ost/Nord)
    Tick { layer: Layer, rgb: Rgb, center: [f64; 2], dir: [f64; 2], half_mm: f64 },
}

/// Anteil der durchgezogenen Linie (UNIT6.PAS / 0x45710c: Konstante 0.8 @0x458e08)
pub const LINE_SOLID: f64 = 0.8;
/// halbe Länge des Streckenstrichs in mm Papier (Original wx/200 Bildbreite, hier fest)
pub const TICK_HALF_MM: f64 = 1.2;
/// Startwert Ellipsenmaßstab des Originals (Hilfe „Fehlerellipsen“: 1:10)
pub const ORIG_ELLIPSE_E: f64 = 10.0;
/// Bezugsstrich: „10 cm Ellipsen“ wenn Ellipsenmaßstab > 5, sonst „1 cm“ (0x45710c, Konstanten 5 / 0.1 / 0.01)
pub fn ref_bar(e: f64) -> (f64, &'static str) {
    if e > 5.0 {
        (0.1, "10 cm Fehler")
    } else if e > 0.5 {
        (0.01, "1 cm Fehler")
    } else {
        (0.001, "1 mm Fehler")
    }
}

pub const C_FIX: Rgb = [30, 41, 59];
pub const C_NEW: Rgb = [220, 38, 38];
pub const C_LINE: Rgb = [148, 163, 184];
pub const C_LINE_H: Rgb = [14, 165, 233];
pub const C_ELL: Rgb = [22, 163, 74];
pub const C_AXES: Rgb = [37, 99, 235];
pub const C_VEC: Rgb = [8, 145, 178];
pub const C_BAD: Rgb = [225, 29, 72];
pub const C_TEXT: Rgb = [15, 23, 42];
pub const C_HEIGHT: Rgb = [3, 105, 161];
pub const C_GRID: Rgb = [203, 213, 225];
pub const C_OFF: Rgb = [203, 213, 225];

#[derive(Clone, Debug)]
pub struct VPoint {
    pub name: String,
    pub y: f64,
    pub x: f64,
    pub h: Option<f64>,
    pub fixed: bool,
    pub adjusted: bool,
    pub ea: f64,
    pub eb: f64,
    pub et: f64,
    pub sh: f64,
    pub dy: f64,
    pub dx: f64,
    pub dh: f64,
    pub sy: f64,
    pub sx: f64,
    pub code: String,
}

#[derive(Clone, Debug, Default)]
pub struct VLine {
    pub a: usize,
    pub b: usize,
    pub dir: bool,
    pub dist: bool,
    pub height: bool,
    pub niv: bool,
    pub outlier: bool,
    pub inactive: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NetView {
    pub pts: Vec<VPoint>,
    pub lines: Vec<VLine>,
    pub plan: bool,
    pub height: bool,
    pub schematic: bool,
    pub adjusted: bool,
    pub sigma0: Option<f64>,
}

fn add_line(lines: &mut Vec<VLine>, a: usize, b: usize, k: ObsKind, outlier: bool, inactive: bool) {
    if a == b {
        return;
    }
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    let l = match lines.iter_mut().position(|l| l.a == lo && l.b == hi) {
        Some(i) => &mut lines[i],
        None => {
            lines.push(VLine { a: lo, b: hi, inactive: true, ..Default::default() });
            lines.last_mut().unwrap()
        }
    };
    match k {
        ObsKind::Dir => l.dir = true,
        ObsKind::HDist | ObsKind::SDist => l.dist = true,
        ObsKind::Zenith | ObsKind::DhTrig => l.height = true,
        ObsKind::DhNiv => l.niv = true,
        _ => {}
    }
    l.outlier |= outlier;
    l.inactive &= inactive;
}

impl NetView {
    pub fn from_outcome(out: &Outcome) -> NetView {
        let merged = out.merged_points();
        let mut v = NetView { adjusted: true, ..Default::default() };
        let mut names: Vec<String> = Vec::new();
        for p in &merged {
            names.push(p.name.clone());
            v.pts.push(VPoint {
                name: p.name.clone(),
                y: p.y,
                x: p.x,
                h: if p.h.is_finite() { Some(p.h) } else { None },
                fixed: p.fix_plan || p.fix_h,
                adjusted: p.plan_adj || p.h_adj,
                ea: if p.plan_adj { p.ea } else { 0.0 },
                eb: if p.plan_adj { p.eb } else { 0.0 },
                et: p.et,
                sh: if p.h_adj { p.sh } else { 0.0 },
                dy: if p.plan_adj { p.dy } else { 0.0 },
                dx: if p.plan_adj { p.dx } else { 0.0 },
                dh: if p.h_adj { p.dh } else { 0.0 },
                sy: p.sy,
                sx: p.sx,
                code: String::new(),
            });
        }
        for part in &out.parts {
            v.plan |= part.net.plan;
            v.height |= part.net.height;
            if v.sigma0.is_none() {
                v.sigma0 = Some(part.sigma0);
            }
            let res: std::collections::HashMap<usize, bool> = part.obs.iter().map(|o| (o.obs, o.outlier)).collect();
            for (i, o) in part.net.obs.iter().enumerate() {
                let a = names.iter().position(|n| *n == part.net.pts[o.from].name).unwrap_or(0);
                let b = names.iter().position(|n| *n == part.net.pts[o.to].name).unwrap_or(0);
                add_line(&mut v.lines, a, b, o.kind, res.get(&i).copied().unwrap_or(false), !o.active);
            }
        }
        v.fix_layout();
        v
    }

    /// Ansicht aus den eingelesenen Daten (vor der Ausgleichung)
    pub fn from_data(kor: &[KorPoint], pm: &PointMap, setups: &[Setup], niv: &[NivLine], use_tachy: bool) -> NetView {
        let mut v = NetView { plan: true, height: true, ..Default::default() };
        let mut names: Vec<String> = Vec::new();
        let mut idx = |v: &mut NetView, n: &str| -> usize {
            if let Some(i) = names.iter().position(|x| x == n) {
                return i;
            }
            let k = kor.iter().find(|p| p.name == n);
            let a = pm.get(n).copied().unwrap_or_default();
            let (y, x) = a.plan.unwrap_or((f64::NAN, f64::NAN));
            v.pts.push(VPoint { name: n.into(), y, x, h: a.h, fixed: k.is_some(), adjusted: false, ea: 0.0, eb: 0.0, et: 0.0, sh: 0.0, dy: 0.0, dx: 0.0, dh: 0.0, sy: 0.0, sx: 0.0, code: k.map(|k| k.code.clone()).unwrap_or_default() });
            names.push(n.into());
            names.len() - 1
        };
        for p in kor {
            idx(&mut v, &p.name);
        }
        if use_tachy {
            for st in setups {
                let a = idx(&mut v, &st.station);
                for sh in &st.shots {
                    let b = idx(&mut v, &sh.target);
                    if sh.hz.is_some() {
                        add_line(&mut v.lines, a, b, ObsKind::Dir, false, false);
                    }
                    if sh.dist.is_some() {
                        add_line(&mut v.lines, a, b, ObsKind::HDist, false, false);
                    }
                }
            }
        }
        for n in niv {
            let a = idx(&mut v, &n.from);
            let b = idx(&mut v, &n.to);
            add_line(&mut v.lines, a, b, ObsKind::DhNiv, false, false);
        }
        // nur Punkte zeigen, die in Beobachtungen vorkommen oder Altpunkte sind
        v.fix_layout();
        v
    }

    /// Punkte ohne Lagekoordinaten schematisch anordnen
    fn fix_layout(&mut self) {
        let have = self.pts.iter().filter(|p| p.y.is_finite() && p.x.is_finite()).count();
        if self.pts.is_empty() {
            return;
        }
        if have * 2 < self.pts.len() {
            self.schematic = true;
            let n = self.pts.len() as f64;
            let r = 50.0 * n.max(3.0);
            for (i, p) in self.pts.iter_mut().enumerate() {
                let a = i as f64 / n * std::f64::consts::TAU;
                p.y = r * a.sin();
                p.x = r * a.cos();
            }
        } else {
            let (cy, cx) = {
                let f: Vec<&VPoint> = self.pts.iter().filter(|p| p.y.is_finite()).collect();
                (f.iter().map(|p| p.y).sum::<f64>() / f.len() as f64, f.iter().map(|p| p.x).sum::<f64>() / f.len() as f64)
            };
            for p in self.pts.iter_mut().filter(|p| !p.y.is_finite() || !p.x.is_finite()) {
                p.y = cy;
                p.x = cx;
                p.code = "?".into();
            }
        }
    }

    pub fn bbox(&self) -> Option<[f64; 4]> {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in &self.pts {
            if !p.y.is_finite() {
                continue;
            }
            b = [b[0].min(p.y), b[1].min(p.x), b[2].max(p.y), b[3].max(p.x)];
        }
        if b[0] > b[2] {
            return None;
        }
        let pad = ((b[2] - b[0]).max(b[3] - b[1]) * 0.06).max(5.0);
        Some([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
    }

    /// größte Ellipsenhalbachse bzw. Höhen-σ (m)
    pub fn max_error(&self) -> f64 {
        self.pts.iter().map(|p| p.ea.max(p.sh)).fold(0.0, f64::max)
    }
}

#[derive(Clone, Debug)]
pub struct Style {
    pub numbers: bool,
    pub heights: bool,
    pub lines: bool,
    pub ellipses: bool,
    pub axes: bool,
    pub vectors: bool,
    pub bars: bool,
    pub outliers: bool,
    /// Ellipsenmaßstab 1 : e (mm Papier je m Ellipse = 1000 / e)
    pub ell_e: f64,
    /// Vergrößerung für Konfidenz (1 = Standardellipse, 2.45 = 95 %)
    pub conf: f64,
    pub text_mm: f64,
}

impl Default for Style {
    fn default() -> Self {
        Style { numbers: true, heights: false, lines: true, ellipses: true, axes: false, vectors: false, bars: true, outliers: true, ell_e: 0.0, conf: 1.0, text_mm: 2.2 }
    }
}

impl Style {
    /// Ellipsenmaßstab automatisch: größte Ellipse etwa 8 mm
    pub fn effective_e(&self, view: &NetView) -> f64 {
        if self.ell_e > 0.0 {
            return self.ell_e;
        }
        auto_ellipse_e(view.max_error() * self.conf)
    }
}

/// "schöner" Ellipsenmaßstab, so dass `max_m` etwa 8 mm groß wird
pub fn auto_ellipse_e(max_m: f64) -> f64 {
    if max_m <= 0.0 {
        return 1.0;
    }
    let raw = max_m * 1000.0 / 8.0;
    let e = raw.log10().floor();
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        let c = m * 10f64.powf(e);
        if c >= raw * 0.999 {
            return c;
        }
    }
    10f64.powf(e + 1.0)
}

pub fn fmt_scale(e: f64) -> String {
    if e >= 1.0 {
        format!("1:{}", trim(e))
    } else {
        format!("{}:1", trim(1.0 / e))
    }
}

pub fn trim(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Szene des Netzbilds
pub fn build(view: &NetView, st: &Style) -> Vec<Item> {
    let mut items = Vec::new();
    let e = st.effective_e(view);
    let k = 1000.0 / e * st.conf; // mm Papier je m Fehler
    if st.lines {
        for l in &view.lines {
            let (a, b) = (&view.pts[l.a], &view.pts[l.b]);
            let bad = st.outliers && l.outlier;
            let (rgb, w, dash, layer) = if bad {
                (C_BAD, 0.45, false, Layer::Ausreisser)
            } else if l.inactive {
                (C_OFF, 0.15, true, Layer::Linien)
            } else if !l.dir && !l.dist && (l.height || l.niv) {
                (C_LINE_H, 0.25, l.niv, Layer::Linien)
            } else {
                (C_LINE, if l.dist { 0.25 } else { 0.18 }, !l.dist, Layer::Linien)
            };
            if l.dir && !bad && !l.inactive {
                // wie UNIT6.PAS (0x45710c): Linie bis 80 % durchgezogen, Rest gestrichelt (Zielrichtung)
                let m = [a.y + LINE_SOLID * (b.y - a.y), a.x + LINE_SOLID * (b.x - a.x)];
                items.push(Item::Line { layer, rgb, w_mm: w, dash: false, pts: vec![[a.y, a.x], m] });
                items.push(Item::Line { layer, rgb, w_mm: w, dash: true, pts: vec![m, [b.y, b.x]] });
            } else {
                items.push(Item::Line { layer, rgb, w_mm: w, dash, pts: vec![[a.y, a.x], [b.y, b.x]] });
            }
            if l.dist && !l.inactive {
                // Querstrich in der Mitte, wenn eine Strecke gemessen ist (0x45710c, Länge wx/200)
                let (dy, dx) = (b.y - a.y, b.x - a.x);
                let s = (dy * dy + dx * dx).sqrt().max(1e-9);
                items.push(Item::Tick { layer, rgb, center: [(a.y + b.y) / 2.0, (a.x + b.x) / 2.0], dir: [dy / s, dx / s], half_mm: TICK_HALF_MM });
            }
        }
    }
    if st.ellipses && view.plan {
        for p in view.pts.iter().filter(|p| p.ea > 0.0) {
            items.push(Item::Ellipse { layer: Layer::Ellipsen, rgb: C_ELL, center: [p.y, p.x], a_mm: p.ea * k, b_mm: p.eb * k, theta_gon: p.et, axes: st.axes });
        }
    }
    if st.vectors {
        for p in view.pts.iter().filter(|p| p.adjusted && (p.dy != 0.0 || p.dx != 0.0)) {
            items.push(Item::Arrow { layer: Layer::Vektoren, rgb: C_VEC, anchor: [p.y, p.x], de_mm: p.dy * k, dn_mm: p.dx * k });
        }
    }
    if st.bars && view.height {
        for p in view.pts.iter().filter(|p| p.sh > 0.0) {
            items.push(Item::Bar { layer: Layer::Vektoren, rgb: C_HEIGHT, anchor: [p.y, p.x], len_mm: p.sh * k, width_mm: 1.2 });
        }
    }
    for p in &view.pts {
        let (marker, rgb, filled) = if p.fixed { (Marker::Triangle, C_FIX, true) } else if p.code == "?" { (Marker::Square, C_BAD, false) } else { (Marker::Circle, C_NEW, true) };
        items.push(Item::Point { layer: Layer::Punkte, rgb, pos: [p.y, p.x], z: p.h.unwrap_or(0.0), marker, size_mm: if p.fixed { 2.4 } else { 1.8 }, filled });
        if st.numbers {
            items.push(Item::Text { layer: Layer::Nummern, rgb: C_TEXT, pos: [p.y, p.x], h_mm: st.text_mm, ax: 0.0, ay: 0.0, dx_mm: 1.8, dy_mm: 1.2, text: p.name.clone() });
        }
        if st.heights {
            if let Some(h) = p.h {
                items.push(Item::Text { layer: Layer::Hoehen, rgb: C_HEIGHT, pos: [p.y, p.x], h_mm: st.text_mm * 0.8, ax: 0.0, ay: 1.0, dx_mm: 1.8, dy_mm: -1.0, text: format!("{:.3}", h) });
            }
        }
    }
    items
}

/// Koordinatengitter (Weltkoordinaten), Schrittweite `step` m, mit Beschriftung
pub fn grid(bb: [f64; 4], step: f64, text_mm: f64) -> Vec<Item> {
    let mut items = Vec::new();
    if step <= 0.0 {
        return items;
    }
    let mut y = (bb[0] / step).ceil() * step;
    while y <= bb[2] {
        items.push(Item::Line { layer: Layer::Gitter, rgb: C_GRID, w_mm: 0.12, dash: false, pts: vec![[y, bb[1]], [y, bb[3]]] });
        items.push(Item::Text { layer: Layer::Gitter, rgb: [100, 116, 139], pos: [y, bb[1]], h_mm: text_mm, ax: 0.5, ay: 0.0, dx_mm: 0.0, dy_mm: 1.0, text: trim(y) });
        y += step;
    }
    let mut x = (bb[1] / step).ceil() * step;
    while x <= bb[3] {
        items.push(Item::Line { layer: Layer::Gitter, rgb: C_GRID, w_mm: 0.12, dash: false, pts: vec![[bb[0], x], [bb[2], x]] });
        items.push(Item::Text { layer: Layer::Gitter, rgb: [100, 116, 139], pos: [bb[0], x], h_mm: text_mm, ax: 0.0, ay: 0.0, dx_mm: 1.0, dy_mm: 1.0, text: trim(x) });
        x += step;
    }
    items
}

/// schöne Schrittweite ≥ raw
pub fn nice_step(raw: f64) -> f64 {
    if raw <= 0.0 {
        return 1.0;
    }
    let e = raw.log10().floor();
    for m in [1.0, 2.0, 5.0, 10.0] {
        let c = m * 10f64.powf(e);
        if c >= raw {
            return c;
        }
    }
    10f64.powf(e + 1.0)
}

/// Ellipsen-Polygon in mm-Offsets (Ost, Nord)
pub fn ellipse_pts(a_mm: f64, b_mm: f64, theta_gon: f64, n: usize) -> Vec<[f64; 2]> {
    let t = theta_gon / crate::geo::RHO;
    // große Halbachse in Richtung t (von Nord im Uhrzeigersinn): Ost = sin t, Nord = cos t
    let (ua, va) = (t.sin(), t.cos());
    let (ub, vb) = (t.cos(), -t.sin());
    (0..=n)
        .map(|i| {
            let w = i as f64 / n as f64 * std::f64::consts::TAU;
            let (c, s) = (w.cos() * a_mm, w.sin() * b_mm);
            [c * ua + s * ub, c * va + s * vb]
        })
        .collect()
}
