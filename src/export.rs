//! Druckausgabe der Netzbilder: PDF-Blatt (A4/A3, hoch/quer, Legendenstreifen mit Nordpfeil,
//! Maßstabsleiste und Ellipsenmaßstab – Blattaufbau wie DGM-NT) und DXF (R12, Ebenen und
//! Farben aus WAUSGL.SYS, Punkte mit Höhe als Z).

use crate::pdf::{self, Content};
use crate::scene::{self, caddy_to_aci, ellipse_pts, fmt_scale, grid, nice_step, trim, Item, Layer, Marker, NetView, Rgb, Style};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    A4,
    A3,
}

#[derive(Clone, Debug)]
pub struct PlotOptions {
    pub paper: Paper,
    pub landscape: bool,
    /// Planmaßstab 1:n, 0 = automatisch
    pub scale_n: f64,
    pub grid: bool,
    pub title: String,
    pub project: String,
    pub editor: String,
    pub info: Vec<String>,
}

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions { paper: Paper::A4, landscape: true, scale_n: 0.0, grid: true, title: "Netzbild".into(), project: String::new(), editor: String::new(), info: vec![] }
    }
}

const BLACK: Rgb = [0, 0, 0];
const GREY: Rgb = [100, 116, 139];

pub struct Sheet {
    pub w: f64,
    pub h: f64,
    pub area: (f64, f64, f64, f64),
    pub strip: (f64, f64, f64, f64),
}

pub fn sheet(o: &PlotOptions) -> Sheet {
    let (a, b) = match o.paper {
        Paper::A4 => (210.0, 297.0),
        Paper::A3 => (297.0, 420.0),
    };
    let (w, h) = if o.landscape { (b, a) } else { (a, b) };
    let m = 10.0;
    if o.landscape {
        let sw = 64.0;
        Sheet { w, h, area: (m, m, w - 2.0 * m - sw - 3.0, h - 2.0 * m), strip: (w - m - sw, m, sw, h - 2.0 * m) }
    } else {
        let sh = 52.0;
        Sheet { w, h, area: (m, m + sh + 3.0, w - 2.0 * m, h - 2.0 * m - sh - 3.0), strip: (m, m, w - 2.0 * m, sh) }
    }
}

/// gängige Planmaßstäbe
pub fn nice_scale(min_n: f64) -> f64 {
    let base = [1.0, 1.25, 2.0, 2.5, 5.0];
    let mut e = 10f64.powf(min_n.max(1.0).log10().floor());
    loop {
        for b in base {
            if b * e >= min_n * 0.999 {
                return b * e;
            }
        }
        e *= 10.0;
    }
}

pub fn auto_scale(view: &NetView, o: &PlotOptions) -> f64 {
    let sh = sheet(o);
    let Some(bb) = view.bbox() else { return 1000.0 };
    let need = ((bb[2] - bb[0]) * 1000.0 / sh.area.2).max((bb[3] - bb[1]) * 1000.0 / sh.area.3);
    nice_scale(need)
}

fn north_arrow(c: &mut Content, cx: f64, cy: f64, h: f64) {
    let tip = (cx, cy + h * 0.6);
    let (r, l, m) = ((cx + h * 0.22, cy - h * 0.4), (cx - h * 0.22, cy - h * 0.4), (cx, cy - h * 0.15));
    c.fill([40, 40, 40]);
    c.polygon(&[tip, r, m], false);
    c.stroke([40, 40, 40]);
    c.width(0.25);
    c.polyline(&[tip, m, l], true);
    c.polyline(&[tip, r, m], true);
    c.text(cx, cy + h * 0.6 + 1.2, 3.0, true, BLACK, 0.5, 0.0, "N");
}

fn scale_bar(c: &mut Content, x: f64, y: f64, n: f64, max_mm: f64) {
    let len_m = {
        let raw = max_mm * n / 1000.0;
        let e = raw.log10().floor();
        let mut v = 10f64.powf(e);
        for m in [5.0, 2.0, 1.0] {
            if m * 10f64.powf(e) <= raw {
                v = m * 10f64.powf(e);
                break;
            }
        }
        v
    };
    let len = len_m * 1000.0 / n;
    for i in 0..4 {
        c.fill(if i % 2 == 0 { [25, 25, 25] } else { [255, 255, 255] });
        c.rect(x + len * i as f64 / 4.0, y, len / 4.0, 1.6, true, false);
    }
    c.stroke([25, 25, 25]);
    c.width(0.2);
    c.rect(x, y, len, 1.6, false, true);
    let u = |m: f64| if m >= 1000.0 { format!("{} km", trim(m / 1000.0)) } else { format!("{} m", trim(m)) };
    c.text(x, y - 1.0, 1.9, false, BLACK, 0.5, 1.0, "0");
    c.text(x + len / 2.0, y - 1.0, 1.9, false, BLACK, 0.5, 1.0, &u(len_m / 2.0));
    c.text(x + len, y - 1.0, 1.9, false, BLACK, 0.5, 1.0, &u(len_m));
}

fn draw_items(c: &mut Content, items: &[Item], tr: &dyn Fn([f64; 2]) -> (f64, f64)) {
    for it in items {
        match it {
            Item::Line { rgb, w_mm, dash, pts, .. } => {
                c.stroke(*rgb);
                c.width(*w_mm);
                c.dash(*dash);
                let v: Vec<(f64, f64)> = pts.iter().map(|p| tr(*p)).collect();
                c.polyline(&v, false);
                c.dash(false);
            }
            Item::Ellipse { rgb, center, a_mm, b_mm, theta_gon, axes, .. } => {
                let (cx, cy) = tr(*center);
                let v: Vec<(f64, f64)> = ellipse_pts(*a_mm, *b_mm, *theta_gon, 72).iter().map(|p| (cx + p[0], cy + p[1])).collect();
                c.stroke(*rgb);
                c.width(0.3);
                c.polyline(&v, true);
                if *axes {
                    let t = theta_gon / crate::geo::RHO;
                    let (s, co) = t.sin_cos();
                    c.stroke(scene::C_AXES);
                    c.width(0.15);
                    c.polyline(&[(cx - a_mm * s, cy - a_mm * co), (cx + a_mm * s, cy + a_mm * co)], false);
                    c.polyline(&[(cx - b_mm * co, cy + b_mm * s), (cx + b_mm * co, cy - b_mm * s)], false);
                }
            }
            Item::Arrow { rgb, anchor, de_mm, dn_mm, .. } => {
                let (x0, y0) = tr(*anchor);
                let (x1, y1) = (x0 + de_mm, y0 + dn_mm);
                c.stroke(*rgb);
                c.width(0.3);
                c.polyline(&[(x0, y0), (x1, y1)], false);
                let l = (de_mm * de_mm + dn_mm * dn_mm).sqrt();
                if l > 0.5 {
                    let (ux, uy) = (de_mm / l, dn_mm / l);
                    let hs = 1.2f64.min(l * 0.4);
                    c.fill(*rgb);
                    c.polygon(&[(x1, y1), (x1 - ux * hs - uy * hs * 0.45, y1 - uy * hs + ux * hs * 0.45), (x1 - ux * hs + uy * hs * 0.45, y1 - uy * hs - ux * hs * 0.45)], false);
                }
            }
            Item::Bar { rgb, anchor, len_mm, width_mm, .. } => {
                let (x0, y0) = tr(*anchor);
                c.fill(*rgb);
                c.stroke(*rgb);
                c.width(0.15);
                c.rect(x0 + 2.2, y0, *width_mm, *len_mm, true, true);
            }
            Item::Tick { rgb, center, dir, half_mm, .. } => {
                let (x0, y0) = tr(*center);
                let (nx, ny) = (-dir[1] * half_mm, dir[0] * half_mm);
                c.stroke(*rgb);
                c.width(0.25);
                c.polyline(&[(x0 - nx, y0 - ny), (x0 + nx, y0 + ny)], false);
            }
            Item::Point { rgb, pos, marker, size_mm, filled, .. } => {
                let (x, y) = tr(*pos);
                let r = size_mm / 2.0;
                c.fill(if *filled { *rgb } else { [255, 255, 255] });
                c.stroke(*rgb);
                c.width(0.25);
                match marker {
                    Marker::Circle => c.circle(x, y, r, true, true),
                    Marker::Triangle => c.polygon(&[(x, y + r * 1.2), (x + r * 1.05, y - r * 0.65), (x - r * 1.05, y - r * 0.65)], true),
                    Marker::Square => c.rect(x - r, y - r, 2.0 * r, 2.0 * r, true, true),
                }
            }
            Item::Text { rgb, pos, h_mm, ax, ay, dx_mm, dy_mm, text, .. } => {
                let (x, y) = tr(*pos);
                c.text(x + dx_mm, y + dy_mm, *h_mm, false, *rgb, *ax, *ay, text);
            }
        }
    }
}

/// PDF-Blatt des Netzbilds
pub fn render_pdf(view: &NetView, st: &Style, o: &PlotOptions) -> (Vec<u8>, f64) {
    let sh = sheet(o);
    let n = if o.scale_n > 0.0 { o.scale_n } else { auto_scale(view, o) };
    let (ax, ay, aw, ah) = sh.area;
    let bb = view.bbox().unwrap_or([0.0, 0.0, 100.0, 100.0]);
    let mm = 1000.0 / n;
    // Weltursprung so, dass das Netz mittig liegt
    let (cy, cx) = ((bb[0] + bb[2]) / 2.0, (bb[1] + bb[3]) / 2.0);
    let (oy, ox) = (cy - aw / 2.0 / mm, cx - ah / 2.0 / mm);
    let tr = move |p: [f64; 2]| (ax + (p[0] - oy) * mm, ay + (p[1] - ox) * mm);
    let mut c = Content::default();
    // Zeichenfläche
    c.save();
    c.clip(ax, ay, aw, ah);
    if o.grid && !view.schematic {
        let step = nice_step(40.0 * n / 1000.0);
        let wb = [oy, ox, oy + aw / mm, ox + ah / mm];
        draw_items(&mut c, &grid(wb, step, 1.8), &tr);
    }
    let items = scene::build(view, st);
    draw_items(&mut c, &items, &tr);
    c.restore();
    c.stroke(BLACK);
    c.width(0.5);
    c.dash(false);
    c.rect(ax, ay, aw, ah, false, true);

    // Legendenstreifen
    let (sx, sy, sw, shh) = sh.strip;
    c.stroke([150, 150, 150]);
    c.width(0.2);
    c.rect(sx, sy, sw, shh, false, true);
    let e = st.effective_e(view);
    let pad = 3.0;
    let legend: Vec<(&str, u8)> = {
        let mut v = vec![("Altpunkt", 0u8), ("Neupunkt", 1)];
        if st.lines {
            v.push(("Richtung / Strecke", 2));
            if view.height {
                v.push(("Höhenbeobachtung", 3));
            }
        }
        if st.ellipses && view.plan && view.adjusted {
            v.push(("Fehlerellipse", 4));
        }
        if st.bars && view.height && view.adjusted {
            v.push(("Höhen-Standardabw.", 5));
        }
        if st.vectors && view.adjusted {
            v.push(("Verschiebung", 6));
        }
        if st.outliers && view.lines.iter().any(|l| l.outlier) {
            v.push(("Ausreißer", 7));
        }
        v
    };
    let draw_sym = |c: &mut Content, k: u8, x: f64, y: f64| match k {
        0 => {
            c.fill(scene::C_FIX);
            c.stroke(scene::C_FIX);
            c.polygon(&[(x + 2.5, y + 2.6), (x + 3.8, y + 0.2), (x + 1.2, y + 0.2)], true);
        }
        1 => {
            c.fill(scene::C_NEW);
            c.stroke(scene::C_NEW);
            c.circle(x + 2.5, y + 1.3, 1.0, true, true);
        }
        2 | 3 | 7 => {
            let col = match k {
                2 => scene::C_LINE,
                3 => scene::C_LINE_H,
                _ => scene::C_BAD,
            };
            c.stroke(col);
            c.width(if k == 7 { 0.45 } else { 0.3 });
            c.polyline(&[(x, y + 1.3), (x + 5.0, y + 1.3)], false);
        }
        4 => {
            c.stroke(scene::C_ELL);
            c.width(0.3);
            let v: Vec<(f64, f64)> = ellipse_pts(2.4, 1.1, 100.0, 36).iter().map(|p| (x + 2.5 + p[0], y + 1.3 + p[1])).collect();
            c.polyline(&v, true);
        }
        5 => {
            c.fill(scene::C_HEIGHT);
            c.rect(x + 2.0, y, 1.2, 2.8, true, false);
        }
        _ => {
            c.stroke(scene::C_VEC);
            c.width(0.3);
            c.polyline(&[(x, y + 0.3), (x + 4.5, y + 2.3)], false);
        }
    };
    let mut lines: Vec<String> = o.info.clone();
    if st.ellipses && view.plan && view.adjusted {
        lines.push(format!("Ellipsenmaßstab {}{}", fmt_scale(e), if (st.conf - 1.0).abs() > 1e-6 { format!(" (×{})", trim(st.conf)) } else { String::new() }));
    }
    if view.schematic {
        lines.push("schematische Darstellung (keine Lagekoordinaten)".into());
    }
    let date = crate::geo::today();
    if o.landscape {
        let mut y = sy + shh - pad - 4.0;
        c.text(sx + pad, y, 2.0, true, GREY, 0.0, 0.0, "AUSGL-NT");
        y -= 6.0;
        for part in wrap(&o.project, 3.0, sw - 2.0 * pad) {
            c.text(sx + pad, y, 3.0, true, BLACK, 0.0, 0.0, &part);
            y -= 5.0;
        }
        for part in wrap(&o.title, 2.5, sw - 2.0 * pad) {
            c.text(sx + pad, y, 2.5, true, scene::C_AXES, 0.0, 0.0, &part);
            y -= 4.4;
        }
        y -= 2.0;
        for l in &lines {
            for part in wrap(l, 2.0, sw - 2.0 * pad) {
                c.text(sx + pad, y, 2.0, false, BLACK, 0.0, 0.0, &part);
                y -= 3.6;
            }
        }
        y -= 3.0;
        c.text(sx + pad, y, 2.1, true, BLACK, 0.0, 0.0, "Legende");
        y -= 5.0;
        for (t, k) in &legend {
            draw_sym(&mut c, *k, sx + pad, y - 0.3);
            c.text(sx + pad + 8.0, y, 2.0, false, BLACK, 0.0, 0.0, t);
            y -= 4.5;
        }
        if st.ellipses && view.plan && view.adjusted {
            // Referenzbalken wie Original (0x45710c): „10 cm“ bei Maßstab > 1:5, sonst „1 cm“
            let (refm, reftxt) = scene::ref_bar(e);
            let len = refm * 1000.0 / e * st.conf;
            y -= 2.0;
            c.stroke(scene::C_ELL);
            c.width(0.4);
            c.polyline(&[(sx + pad, y), (sx + pad + len, y)], false);
            c.polyline(&[(sx + pad, y - 1.0), (sx + pad, y + 1.0)], false);
            c.polyline(&[(sx + pad + len, y - 1.0), (sx + pad + len, y + 1.0)], false);
            c.text(sx + pad + len + 2.0, y - 0.9, 2.0, false, BLACK, 0.0, 0.0, reftxt);
        }
        let cx = sx + sw / 2.0;
        north_arrow(&mut c, cx, sy + 46.0, 12.0);
        c.text(cx, sy + 31.0, 2.8, true, BLACK, 0.5, 0.0, &format!("Maßstab 1:{}", trim(n)));
        c.text(cx, sy + 26.0, 2.0, false, BLACK, 0.5, 0.0, &format!("Stand: {}{}", date, if o.editor.is_empty() { String::new() } else { format!("  ·  {}", o.editor) }));
        scale_bar(&mut c, sx + pad + 3.0, sy + 13.0, n, (sw - 2.0 * pad - 8.0).min(48.0));
    } else {
        // drei Spalten: Titel/Info | Legende | Maßstab + Nordpfeil
        let (wa, wb) = (sw * 0.42, sw * 0.30);
        let top = sy + shh - pad - 3.0;
        let mut y = top;
        c.text(sx + pad, y, 2.0, true, GREY, 0.0, 0.0, "AUSGL-NT");
        y -= 5.5;
        c.text(sx + pad, y, 3.0, true, BLACK, 0.0, 0.0, &o.project);
        y -= 4.8;
        c.text(sx + pad, y, 2.4, true, scene::C_AXES, 0.0, 0.0, &o.title);
        y -= 4.5;
        for l in &lines {
            if y < sy + 3.0 {
                break;
            }
            c.text(sx + pad, y, 1.9, false, BLACK, 0.0, 0.0, l);
            y -= 3.4;
        }
        let mut y = top;
        let xb = sx + wa + pad;
        c.text(xb, y, 2.1, true, BLACK, 0.0, 0.0, "Legende");
        y -= 4.8;
        for (t, k) in &legend {
            if y < sy + 3.0 {
                break;
            }
            draw_sym(&mut c, *k, xb, y - 0.3);
            c.text(xb + 8.0, y, 1.9, false, BLACK, 0.0, 0.0, t);
            y -= 4.2;
        }
        let xc = sx + wa + wb + pad;
        let wc = sw - wa - wb - 2.0 * pad;
        c.stroke([200, 200, 200]);
        c.width(0.15);
        c.polyline(&[(xb - pad, sy + 2.0), (xb - pad, sy + shh - 2.0)], false);
        c.polyline(&[(xc - pad, sy + 2.0), (xc - pad, sy + shh - 2.0)], false);
        north_arrow(&mut c, xc + wc - 7.0, sy + shh - 15.0, 11.0);
        c.text(xc, sy + shh - 7.0, 2.8, true, BLACK, 0.0, 0.0, &format!("Maßstab 1:{}", trim(n)));
        c.text(xc, sy + shh - 12.0, 1.9, false, BLACK, 0.0, 0.0, &format!("Stand: {}", date));
        if st.ellipses && view.plan && view.adjusted {
            c.text(xc, sy + shh - 16.5, 1.9, false, BLACK, 0.0, 0.0, &format!("Ellipsen {}", fmt_scale(e)));
        }
        scale_bar(&mut c, xc + 1.0, sy + 10.0, n, (wc - 6.0).min(42.0));
    }
    (pdf::build(&c, sh.w, sh.h, &format!("{} – {}", o.project, o.title)), n)
}

fn wrap(text: &str, cap: f64, maxw: f64) -> Vec<String> {
    let size = cap / 0.718;
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        let t = if cur.is_empty() { w.to_string() } else { format!("{} {}", cur, w) };
        if cur.is_empty() || pdf::text_width(&t, size, false) <= maxw {
            cur = t;
        } else {
            out.push(std::mem::take(&mut cur));
            cur = w.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

// ------------------------------------------------------------------ DXF

struct Dxf {
    s: String,
}

impl Dxf {
    fn g(&mut self, code: i32, v: impl std::fmt::Display) {
        let _ = write!(self.s, "{}\r\n{}\r\n", code, v);
    }
    fn f(&mut self, code: i32, v: f64) {
        let _ = write!(self.s, "{}\r\n{:.4}\r\n", code, v);
    }
    fn ent(&mut self, kind: &str, layer: Layer) {
        self.g(0, kind);
        self.g(8, layer.dxf_name());
    }
    fn line(&mut self, layer: Layer, a: [f64; 3], b: [f64; 3], dashed: bool) {
        self.ent("LINE", layer);
        if dashed {
            self.g(6, "DASHED");
        }
        self.f(10, a[0]);
        self.f(20, a[1]);
        self.f(30, a[2]);
        self.f(11, b[0]);
        self.f(21, b[1]);
        self.f(31, b[2]);
    }
    fn poly(&mut self, layer: Layer, pts: &[[f64; 2]], closed: bool) {
        self.ent("POLYLINE", layer);
        self.g(66, 1);
        self.f(10, 0.0);
        self.f(20, 0.0);
        self.f(30, 0.0);
        self.g(70, if closed { 1 } else { 0 });
        for p in pts {
            self.ent("VERTEX", layer);
            self.f(10, p[0]);
            self.f(20, p[1]);
            self.f(30, 0.0);
        }
        self.ent("SEQEND", layer);
    }
    fn text(&mut self, layer: Layer, p: [f64; 2], h: f64, ax: f64, ay: f64, t: &str) {
        self.ent("TEXT", layer);
        self.f(10, p[0]);
        self.f(20, p[1]);
        self.f(30, 0.0);
        self.f(40, h);
        self.g(1, t);
        let ha = if ax > 0.75 { 2 } else if ax > 0.25 { 1 } else { 0 };
        let va = if ay > 0.75 { 3 } else if ay > 0.25 { 2 } else { 0 };
        if ha != 0 || va != 0 {
            self.g(72, ha);
            self.g(73, va);
            self.f(11, p[0]);
            self.f(21, p[1]);
            self.f(31, 0.0);
        }
    }
}

/// DXF des Netzbilds; Zeichengrößen in mm × n/1000 = m
pub fn to_dxf(view: &NetView, st: &Style, n: f64, with_grid: bool) -> Vec<u8> {
    let k = n / 1000.0;
    let mut d = Dxf { s: String::new() };
    d.g(0, "SECTION");
    d.g(2, "HEADER");
    d.g(9, "$ACADVER");
    d.g(1, "AC1009");
    d.g(9, "$DWGCODEPAGE");
    d.g(3, "ANSI_1252");
    if let Some(bb) = view.bbox() {
        d.g(9, "$EXTMIN");
        d.f(10, bb[0]);
        d.f(20, bb[1]);
        d.g(9, "$EXTMAX");
        d.f(10, bb[2]);
        d.f(20, bb[3]);
    }
    d.g(0, "ENDSEC");
    d.g(0, "SECTION");
    d.g(2, "TABLES");
    d.g(0, "TABLE");
    d.g(2, "LTYPE");
    d.g(70, 2);
    for (name, desc, pat) in [("CONTINUOUS", "Solid line", vec![]), ("DASHED", "__ __ __", vec![0.5 * k * 4.0, -0.25 * k * 4.0])] {
        d.g(0, "LTYPE");
        d.g(2, name);
        d.g(70, 0);
        d.g(3, desc);
        d.g(72, 65);
        d.g(73, pat.len());
        d.f(40, pat.iter().map(|x: &f64| x.abs()).sum());
        for p in pat {
            d.f(49, p);
        }
    }
    d.g(0, "ENDTAB");
    d.g(0, "TABLE");
    d.g(2, "LAYER");
    d.g(70, Layer::ALL.len());
    for l in Layer::ALL {
        d.g(0, "LAYER");
        d.g(2, l.dxf_name());
        d.g(70, 0);
        d.g(62, caddy_to_aci(l.caddy_color()));
        d.g(6, "CONTINUOUS");
    }
    d.g(0, "ENDTAB");
    d.g(0, "ENDSEC");
    d.g(0, "SECTION");
    d.g(2, "ENTITIES");
    let mut items = Vec::new();
    if with_grid && !view.schematic {
        if let Some(bb) = view.bbox() {
            items.extend(grid(bb, nice_step(40.0 * k), 1.8));
        }
    }
    items.extend(scene::build(view, st));
    for it in &items {
        match it {
            Item::Line { layer, dash, pts, .. } => {
                for w in pts.windows(2) {
                    d.line(*layer, [w[0][0], w[0][1], 0.0], [w[1][0], w[1][1], 0.0], *dash);
                }
            }
            Item::Point { layer, pos, z, marker, size_mm, .. } => {
                d.ent("POINT", *layer);
                d.f(10, pos[0]);
                d.f(20, pos[1]);
                d.f(30, *z);
                let r = size_mm / 2.0 * k;
                match marker {
                    Marker::Circle => {
                        d.ent("CIRCLE", *layer);
                        d.f(10, pos[0]);
                        d.f(20, pos[1]);
                        d.f(30, 0.0);
                        d.f(40, r);
                    }
                    Marker::Triangle => d.poly(*layer, &[[pos[0], pos[1] + r * 1.2], [pos[0] + r * 1.05, pos[1] - r * 0.65], [pos[0] - r * 1.05, pos[1] - r * 0.65]], true),
                    Marker::Square => d.poly(*layer, &[[pos[0] - r, pos[1] - r], [pos[0] + r, pos[1] - r], [pos[0] + r, pos[1] + r], [pos[0] - r, pos[1] + r]], true),
                }
            }
            Item::Text { layer, pos, h_mm, ax, ay, dx_mm, dy_mm, text, .. } => d.text(*layer, [pos[0] + dx_mm * k, pos[1] + dy_mm * k], h_mm * k, *ax, *ay, text),
            Item::Ellipse { layer, center, a_mm, b_mm, theta_gon, axes, .. } => {
                let pts: Vec<[f64; 2]> = ellipse_pts(*a_mm, *b_mm, *theta_gon, 72).iter().map(|p| [center[0] + p[0] * k, center[1] + p[1] * k]).collect();
                d.poly(*layer, &pts[..pts.len() - 1], true);
                if *axes {
                    let t = theta_gon / crate::geo::RHO;
                    let (s, c) = t.sin_cos();
                    let (a, b) = (a_mm * k, b_mm * k);
                    d.line(Layer::Achsen, [center[0] - a * s, center[1] - a * c, 0.0], [center[0] + a * s, center[1] + a * c, 0.0], false);
                    d.line(Layer::Achsen, [center[0] - b * c, center[1] + b * s, 0.0], [center[0] + b * c, center[1] - b * s, 0.0], false);
                }
            }
            Item::Arrow { layer, anchor, de_mm, dn_mm, .. } => {
                let e = [anchor[0] + de_mm * k, anchor[1] + dn_mm * k];
                d.line(*layer, [anchor[0], anchor[1], 0.0], [e[0], e[1], 0.0], false);
                let l = (de_mm * de_mm + dn_mm * dn_mm).sqrt();
                if l > 0.5 {
                    let (ux, uy) = (de_mm / l, dn_mm / l);
                    let hs = 1.2f64.min(l * 0.4) * k;
                    d.poly(*layer, &[e, [e[0] - ux * hs - uy * hs * 0.45, e[1] - uy * hs + ux * hs * 0.45], [e[0] - ux * hs + uy * hs * 0.45, e[1] - uy * hs - ux * hs * 0.45]], true);
                }
            }
            Item::Bar { layer, anchor, len_mm, width_mm, .. } => {
                let x0 = anchor[0] + 2.2 * k;
                let (w, h) = (width_mm * k, len_mm * k);
                d.poly(*layer, &[[x0, anchor[1]], [x0 + w, anchor[1]], [x0 + w, anchor[1] + h], [x0, anchor[1] + h]], true);
            }
            Item::Tick { layer, center, dir, half_mm, .. } => {
                let (nx, ny) = (-dir[1] * half_mm * k, dir[0] * half_mm * k);
                d.line(*layer, [center[0] - nx, center[1] - ny, 0.0], [center[0] + nx, center[1] + ny, 0.0], false);
            }
        }
    }
    d.g(0, "ENDSEC");
    d.g(0, "EOF");
    pdf::to_cp1252(&d.s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_and_dxf_from_sample() {
        let smp = crate::sample::Sample::generate(true);
        let out = crate::adjust::run(&smp.input(crate::io::NetKind::LageHoehe), &crate::adjust::Settings::default()).unwrap();
        let view = NetView::from_outcome(&out);
        let st = Style { vectors: true, heights: true, axes: true, ..Default::default() };
        for land in [true, false] {
            let o = PlotOptions { landscape: land, project: "Test".into(), ..Default::default() };
            let (pdf, n) = render_pdf(&view, &st, &o);
            assert!(pdf.starts_with(b"%PDF") && pdf.ends_with(b"%%EOF\n"));
            assert!(n >= 1000.0);
        }
        let dxf = to_dxf(&view, &st, 2000.0, true);
        let s = String::from_utf8_lossy(&dxf);
        assert!(s.contains("16_FEHLERELLIPSEN") && s.ends_with("EOF\r\n"));
        assert!(nice_scale(1234.0) == 1250.0 && nice_scale(1000.0) == 1000.0);
    }
}
