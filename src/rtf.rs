//! RTF-Ausgabe der Protokolle (A4, für Word/LibreOffice/WordPad): Kopfzeile mit Firmenkopf,
//! Seitenzahlen, Tabellen mit Kopfzeilenwiederholung und farbig markierten Ausreißern.

use crate::report::{Align, Block, Doc, Tone};
use std::fmt::Write as _;

/// Text RTF-sicher kodieren (Sonderzeichen als \uN?)
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => o += "\\\\",
            '{' => o += "\\{",
            '}' => o += "\\}",
            '\n' => o += "\\line ",
            '\t' => o += "\\tab ",
            c if (c as u32) < 128 => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(o, "\\u{}?", *u as i16);
                }
            }
        }
    }
    o
}

const PAGE_W: i32 = 11906;
const MARGIN: i32 = 1021; // 18 mm
const LANDSCAPE_W: i32 = 16838;

pub fn to_rtf(doc: &Doc, landscape: bool) -> String {
    let (pw, ph) = if landscape { (LANDSCAPE_W, PAGE_W) } else { (PAGE_W, LANDSCAPE_W) };
    let text_w = pw - 2 * MARGIN;
    let mut s = String::new();
    s += "{\\rtf1\\ansi\\ansicpg1252\\deff0\\deflang1031\n";
    s += "{\\fonttbl{\\f0\\fswiss\\fcharset0 Arial;}{\\f1\\fmodern\\fcharset0 Consolas;}}\n";
    // 1 schwarz, 2 dunkelblau, 3 rot, 4 orange, 5 grau, 6 hellblau (Tabellenkopf), 7 hellrot, 8 hellgelb
    s += "{\\colortbl;\\red0\\green0\\blue0;\\red22\\green62\\blue120;\\red190\\green20\\blue20;\\red170\\green100\\blue0;\\red120\\green120\\blue120;\\red222\\green232\\blue245;\\red252\\green226\\blue226;\\red253\\green244\\blue214;}\n";
    let _ = writeln!(s, "{{\\info{{\\title Protokoll}}{{\\author AUSGL-NT}}}}");
    let _ = writeln!(s, "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}{}\\viewkind4\\uc1", pw, ph, MARGIN, MARGIN, 1300, 1100, if landscape { "\\landscape" } else { "" });
    // Kopf- und Fußzeile
    s += "{\\header\\pard\\plain\\f0\\fs16\\cf5 ";
    s += &esc(&doc.header.join("   |   "));
    let _ = write!(s, "\\par\\pard\\brdrb\\brdrs\\brdrw5\\brdrcf5\\fs4\\par}}\n");
    s += "{\\footer\\pard\\plain\\qr\\f0\\fs16\\cf5 Seite {\\field{\\*\\fldinst PAGE}{\\fldrslt 1}} von {\\field{\\*\\fldinst NUMPAGES}{\\fldrslt 1}}\\par}\n";
    for b in &doc.blocks {
        match b {
            Block::Title(t) => {
                let _ = writeln!(s, "\\pard\\plain\\sb240\\sa120\\keepn\\f0\\fs30\\b\\cf2 {}\\par", esc(t));
            }
            Block::Heading(t) => {
                let _ = writeln!(s, "\\pard\\plain\\sb200\\sa80\\keepn\\f0\\fs22\\b\\cf2 {}\\par", esc(t));
            }
            Block::Para(t) => {
                let _ = writeln!(s, "\\pard\\plain\\sa80\\f0\\fs17\\cf5 {}\\par", esc(t));
            }
            Block::Note(t, tone) => {
                let cf = match tone {
                    Tone::Bad => 3,
                    Tone::Warn => 4,
                    _ => 1,
                };
                let _ = writeln!(s, "\\pard\\plain\\sa60\\f0\\fs18\\cf{} {}\\par", cf, esc(t));
            }
            Block::KeyVals(v) => {
                let c1 = (text_w as f32 * 0.32) as i32;
                for (k, val) in v {
                    let _ = writeln!(s, "\\trowd\\trgaph60\\trleft0\\cellx{}\\cellx{}\\pard\\intbl\\plain\\f0\\fs18\\cf5 {}\\cell\\pard\\intbl\\plain\\f0\\fs18 {}\\cell\\row", c1, text_w, esc(k), esc(val));
                }
                s += "\\pard\\plain\\fs8\\par\n";
            }
            Block::Table(t) => {
                let total: f32 = t.cols.iter().map(|c| c.2).sum::<f32>().max(0.01);
                let mut xs = Vec::new();
                let mut acc = 0.0f32;
                for c in &t.cols {
                    acc += c.2 / total;
                    xs.push((acc * text_w as f32) as i32);
                }
                let fs = if t.cols.len() > 9 { 14 } else { 16 };
                let row = |s: &mut String, cells: &[String], head: bool, tone: Tone| {
                    s.push_str("\\trowd\\trgaph50\\trleft0\\trpaddl50\\trpaddr50");
                    if head {
                        s.push_str("\\trhdr");
                    }
                    for x in &xs {
                        if head {
                            s.push_str("\\clcbpat6\\clbrdrb\\brdrs\\brdrw10\\brdrcf2");
                        } else {
                            s.push_str("\\clbrdrb\\brdrs\\brdrw2\\brdrcf6");
                            match tone {
                                Tone::Bad => s.push_str("\\clcbpat7"),
                                Tone::Warn => s.push_str("\\clcbpat8"),
                                _ => {}
                            }
                        }
                        let _ = write!(s, "\\cellx{}", x);
                    }
                    for (i, c) in cells.iter().enumerate() {
                        let al = match t.cols.get(i).map(|c| c.1).unwrap_or(Align::L) {
                            Align::L => "\\ql",
                            Align::R => "\\qr",
                            Align::C => "\\qc",
                        };
                        let cf = match (head, tone) {
                            (true, _) => 2,
                            (_, Tone::Bad) => 3,
                            (_, Tone::Muted) => 5,
                            _ => 1,
                        };
                        let _ = write!(s, "\\pard\\intbl{}\\plain\\f0\\fs{}\\cf{}{} {}\\cell", al, fs, cf, if head { "\\b" } else { "" }, esc(c));
                    }
                    s.push_str("\\row\n");
                };
                row(&mut s, &t.cols.iter().map(|c| c.0.clone()).collect::<Vec<_>>(), true, Tone::Normal);
                for (r, tone) in &t.rows {
                    row(&mut s, r, false, *tone);
                }
                s += "\\pard\\plain\\fs8\\par\n";
            }
        }
    }
    s += "}\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rtf_encodes_umlauts() {
        let doc = Doc { header: vec!["Büro".into()], blocks: vec![Block::Title("Höhe".into())] };
        let r = to_rtf(&doc, false);
        assert!(r.starts_with("{\\rtf1"));
        assert!(r.contains("H\\u246?he"));
        let open = r.matches('{').count() - r.matches("\\{").count();
        let close = r.matches('}').count() - r.matches("\\}").count();
        assert_eq!(open, close);
    }

    /// Protokoll des Seminar-Brückennetzes als RTF schreiben (Sichtprüfung in Word/LibreOffice):
    /// AUSGL_RTF_OUT=Ordner cargo test --release -- --ignored rtf_seminar
    #[test]
    #[ignore]
    fn rtf_seminar() {
        let Ok(out_dir) = std::env::var("AUSGL_RTF_OUT") else { return };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Testdaten/Seminar Qualität von geodätischen Netzen - Dateien");
        let c = crate::caplan::parse(&crate::io::read_text(&dir.join("Bsp3 - Brückennetz mit 6 SP.N")).unwrap());
        let mut s = crate::adjust::Settings::default();
        s.variant = c.variant();
        let inp = crate::adjust::Input { kind: c.kind(), kor: &c.fixed, lagekor: &[], kon: &[], approx: &c.approx, hfix: &c.hfix, setups: &c.setups, niv: &c.niv, disabled: &[] };
        let o = crate::adjust::run(&inp, &s).unwrap();
        let proj = crate::io::ProjectFile { name: "Brückennetz".into(), kind: c.kind(), settings: s.clone(), ..Default::default() };
        let doc = crate::report::adjustment_doc(&o, &s, &proj, &["Büro".to_string()]);
        for (name, land) in [("quer", true), ("hoch", false)] {
            std::fs::write(std::path::Path::new(&out_dir).join(format!("protokoll_{}.rtf", name)), to_rtf(&doc, land)).unwrap();
        }
    }
}
