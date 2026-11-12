//! Schlanker PDF-Schreiber (Vektor): Pfade, Kreise, Text in Helvetica (WinAnsi). Maße in mm,
//! Ursprung links unten. Für die Netzbilder in derselben Art wie bei DGM-NT.

use crate::scene::Rgb;
use std::fmt::Write as _;

const PT: f64 = 72.0 / 25.4;
const CAP: f64 = 0.718;

/// ungefähre Helvetica-Laufweiten (1/1000 em)
fn glyph_w(c: char, bold: bool) -> f64 {
    let w = match c {
        ' ' => 278,
        'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 222,
        'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' | '/' | 'I' => 333,
        'm' | 'M' => 833,
        'w' | 'W' => 778,
        '0'..='9' | '±' | '+' | '=' | '<' | '>' => 556,
        'A'..='Z' | 'Ä' | 'Ö' | 'Ü' => 690,
        'a'..='z' | 'ä' | 'ö' | 'ü' | 'ß' => 540,
        _ => 556,
    } as f64;
    w / 1000.0 * if bold { 1.06 } else { 1.0 }
}

pub fn text_width(s: &str, size: f64, bold: bool) -> f64 {
    s.chars().map(|c| glyph_w(c, bold)).sum::<f64>() * size
}

pub fn to_cp1252(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            '€' => 0x80,
            '–' => 0x96,
            '—' => 0x97,
            '„' => 0x84,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            c if (c as u32) < 256 => c as u32 as u8,
            'σ' => b's',
            'θ' => b't',
            'χ' => b'x',
            '→' => b'>',
            '√' => b'V',
            _ => b'?',
        })
        .collect()
}

fn n(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" || s == "-" {
        "0".into()
    } else {
        s.into()
    }
}

fn rgb(c: Rgb) -> String {
    format!("{} {} {}", n(c[0] as f64 / 255.0), n(c[1] as f64 / 255.0), n(c[2] as f64 / 255.0))
}

#[derive(Default)]
pub struct Content {
    s: String,
}

impl Content {
    pub fn save(&mut self) {
        self.s += "q\n";
    }
    pub fn restore(&mut self) {
        self.s += "Q\n";
    }
    pub fn clip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let _ = writeln!(self.s, "{} {} {} {} re W n", n(x), n(y), n(w), n(h));
    }
    pub fn stroke(&mut self, c: Rgb) {
        let _ = writeln!(self.s, "{} RG", rgb(c));
    }
    pub fn fill(&mut self, c: Rgb) {
        let _ = writeln!(self.s, "{} rg", rgb(c));
    }
    pub fn width(&mut self, w: f64) {
        let _ = writeln!(self.s, "{} w", n(w.max(0.02)));
    }
    pub fn dash(&mut self, on: bool) {
        self.s += if on { "[1.6 1] 0 d\n" } else { "[] 0 d\n" };
    }
    fn path(&mut self, pts: &[(f64, f64)]) {
        for (i, p) in pts.iter().enumerate() {
            let _ = writeln!(self.s, "{} {} {}", n(p.0), n(p.1), if i == 0 { "m" } else { "l" });
        }
    }
    pub fn polyline(&mut self, pts: &[(f64, f64)], closed: bool) {
        if pts.len() < 2 {
            return;
        }
        self.path(pts);
        self.s += if closed { "s\n" } else { "S\n" };
    }
    pub fn polygon(&mut self, pts: &[(f64, f64)], stroke: bool) {
        if pts.len() < 3 {
            return;
        }
        self.path(pts);
        self.s += if stroke { "b\n" } else { "f\n" };
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: bool, stroke: bool) {
        let op = match (fill, stroke) {
            (true, true) => "B",
            (true, false) => "f",
            (false, true) => "S",
            _ => "n",
        };
        let _ = writeln!(self.s, "{} {} {} {} re {}", n(x), n(y), n(w), n(h), op);
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: bool, stroke: bool) {
        let k = 0.552_284_75 * r;
        let _ = writeln!(self.s, "{} {} m", n(cx + r), n(cy));
        for (a, b, c, d, e, f) in [
            (cx + r, cy + k, cx + k, cy + r, cx, cy + r),
            (cx - k, cy + r, cx - r, cy + k, cx - r, cy),
            (cx - r, cy - k, cx - k, cy - r, cx, cy - r),
            (cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        ] {
            let _ = writeln!(self.s, "{} {} {} {} {} {} c", n(a), n(b), n(c), n(d), n(e), n(f));
        }
        self.s += match (fill, stroke) {
            (true, true) => "b\n",
            (true, false) => "f\n",
            _ => "S\n",
        };
    }
    /// Text mit Großbuchstabenhöhe cap (mm); ax 0 links … 1 rechts, ay 0 Grundlinie … 1 oben
    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, x: f64, y: f64, cap: f64, bold: bool, c: Rgb, ax: f64, ay: f64, t: &str) {
        if t.is_empty() {
            return;
        }
        let size = cap / CAP;
        let w = text_width(t, size, bold);
        let hex: String = to_cp1252(t).iter().map(|b| format!("{:02X}", b)).collect();
        let _ = writeln!(self.s, "q {} rg BT /{} {} Tf {} {} Td <{}> Tj ET Q", rgb(c), if bold { "F2" } else { "F1" }, n(size), n(x - ax * w), n(y - ay * cap), hex);
    }
}

fn utf16_hex(s: &str) -> String {
    let mut o = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(o, "{:04X}", u);
    }
    o + ">"
}

/// PDF mit einer Seite w × h mm
pub fn build(c: &Content, w: f64, h: f64, title: &str) -> Vec<u8> {
    let body = format!("{} 0 0 {} 0 0 cm\n1 j 1 J\n{}", n(PT), n(PT), c.s);
    let z = miniz_oxide::deflate::compress_to_vec_zlib(body.as_bytes(), 6);
    let mut objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>", n(w * PT), n(h * PT)).into_bytes(),
    ];
    let mut st = format!("<< /Filter /FlateDecode /Length {} >>\nstream\n", z.len()).into_bytes();
    st.extend_from_slice(&z);
    st.extend_from_slice(b"\nendstream");
    objs.push(st);
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec());
    objs.push(format!("<< /Title {} /Creator {} /Producer (AUSGL-NT) >>", utf16_hex(title), utf16_hex("AUSGL-NT")).into_bytes());
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in &offs {
        out.extend_from_slice(format!("{:010} 00000 n \n", o).as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, objs.len(), xref).as_bytes());
    out
}
