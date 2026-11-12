//! Protokoll als Dokumentmodell (Überschriften, Angaben, Tabellen) – wird als RTF geschrieben
//! und in der Oberfläche angezeigt. Inhalt wie die Protokolldatei *.DRU des Originals
//! (Verbesserungen, Ergebnisse, Standardabweichungen, Redundanzkennzeichen), erweitert um
//! Statistik, normierte Verbesserungen und robuste Gewichte.

use crate::adjust::{ObsKind, Outcome, PartResult, Settings};
use crate::geo::f;
use crate::io::ProjectFile;
use crate::tools::{SetMeanStation, TransResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    L,
    R,
    C,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Tone {
    #[default]
    Normal,
    Warn,
    Bad,
    Muted,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub cols: Vec<(String, Align, f32)>,
    pub rows: Vec<(Vec<String>, Tone)>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Title(String),
    Heading(String),
    Para(String),
    KeyVals(Vec<(String, String)>),
    Table(Table),
    Note(String, Tone),
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub header: Vec<String>,
    pub blocks: Vec<Block>,
}

impl Doc {
    fn h(&mut self, s: &str) {
        self.blocks.push(Block::Heading(s.into()));
    }
    fn kv(&mut self, v: Vec<(&str, String)>) {
        self.blocks.push(Block::KeyVals(v.into_iter().map(|(a, b)| (a.to_string(), b)).collect()));
    }
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for l in &self.header {
            s += l;
            s += "\n";
        }
        for b in &self.blocks {
            match b {
                Block::Title(t) => s += &format!("\n{}\n{}\n", t, "=".repeat(t.chars().count())),
                Block::Heading(t) => s += &format!("\n{}\n{}\n", t, "-".repeat(t.chars().count())),
                Block::Para(t) | Block::Note(t, _) => s += &format!("{}\n", t),
                Block::KeyVals(v) => {
                    let w = v.iter().map(|(a, _)| a.chars().count()).max().unwrap_or(0);
                    for (a, b) in v {
                        s += &format!("  {:<w$} : {}\n", a, b, w = w);
                    }
                }
                Block::Table(t) => {
                    let mut wid: Vec<usize> = t.cols.iter().map(|c| c.0.chars().count()).collect();
                    for (r, _) in &t.rows {
                        for (i, c) in r.iter().enumerate() {
                            if i < wid.len() {
                                wid[i] = wid[i].max(c.chars().count());
                            }
                        }
                    }
                    let line = |cells: &[String]| -> String {
                        let mut o = String::new();
                        for (i, c) in cells.iter().enumerate() {
                            let w = wid.get(i).copied().unwrap_or(4);
                            match t.cols.get(i).map(|c| c.1).unwrap_or(Align::L) {
                                Align::L => o += &format!("{:<w$}  ", c, w = w),
                                _ => o += &format!("{:>w$}  ", c, w = w),
                            }
                        }
                        o.trim_end().to_string() + "\n"
                    };
                    s += &line(&t.cols.iter().map(|c| c.0.clone()).collect::<Vec<_>>());
                    for (r, _) in &t.rows {
                        s += &line(r);
                    }
                }
            }
        }
        s
    }
}

fn header(proj: &ProjectFile, firm: &[String]) -> Vec<String> {
    let mut h: Vec<String> = firm.iter().filter(|l| !l.trim().is_empty()).cloned().collect();
    h.push(format!("Projekt {}  –  {}", proj.name, crate::geo::today()));
    h
}

fn project_block(doc: &mut Doc, proj: &ProjectFile) {
    doc.h("Projekt");
    let mut v = vec![("Projekt", proj.name.clone())];
    if !proj.nummer.is_empty() {
        v.push(("Nummer", proj.nummer.clone()));
    }
    if !proj.beschreibung.is_empty() {
        v.push(("Beschreibung", proj.beschreibung.clone()));
    }
    if !proj.ort.is_empty() || !proj.ort1.is_empty() {
        v.push(("Ort", format!("{} {}", proj.ort, proj.ort1).trim().to_string()));
    }
    if !proj.bearbeiter.is_empty() {
        v.push(("Bearbeiter", proj.bearbeiter.clone()));
    }
    v.push(("Datum", crate::geo::today()));
    if !proj.kor.is_empty() {
        v.push(("Altpunkte", proj.kor.clone()));
    }
    if !proj.mes.is_empty() && proj.kind.uses_tachy() {
        v.push(("Meßdaten", proj.mes.clone()));
    }
    if !proj.niv.is_empty() {
        v.push(("Nivellement", proj.niv.clone()));
    }
    doc.kv(v);
}

fn settings_block(doc: &mut Doc, s: &Settings, proj: &ProjectFile) {
    doc.h("Berechnungsansatz");
    let mut v = vec![
        ("Berechnungsart", proj.kind.label().to_string()),
        ("Variante", s.variant.label().to_string()),
        ("Lösungsverfahren", s.solver.label().to_string()),
        ("Robuste Schätzung", if s.robust == crate::adjust::Robust::Keine { "keine".to_string() } else { format!("{} (k = {})", s.robust.label(), f(s.k(), 3)) }),
    ];
    use crate::io::NetKind::*;
    if matches!(proj.kind, Lage | LageHoehe | Raum3D | Vorwaerts) {
        v.push(("σ Richtung", format!("{} mgon", f(s.sigma_dir_mgon, 2))));
    }
    if matches!(proj.kind, Lage | LageHoehe | Raum3D) {
        v.push(("σ Strecke", format!("{} mm + {} mm/km", f(s.sigma_dist_mm, 1), f(s.sigma_dist_ppm, 1))));
    }
    if matches!(proj.kind, Raum3D | Vorwaerts) {
        v.push(("σ Zenitwinkel", format!("{} mgon", f(s.sigma_zen_mgon, 2))));
    }
    if matches!(proj.kind, Trig | LageHoehe | Raum3D) {
        v.push(("σ trig. Höhenunterschied", format!("{} mm/km (Gewicht 1/s²)", f(s.sigma_trig_mm_km, 1))));
    }
    if matches!(proj.kind, Niv | LageHoehe | Raum3D) || !proj.niv.is_empty() {
        v.push(("σ Nivellement", format!("{} mm/√km (Gewicht 1/s)", f(s.sigma_niv_mm_km, 1))));
    }
    if s.variant.ap_obs() {
        v.push(("σ Altpunkte", format!("Lage {} mm, Höhe {} mm", f(s.sigma_fix_mm, 1), f(s.sigma_fix_h_mm, 1))));
    }
    if s.reduce {
        v.push(("Streckenreduktion", format!("Hm = {} m, Ym = {} m", f(s.hm, 0), f(s.ym, 0))));
    }
    if s.dir_weight_dist > 0.0 {
        v.push(("Richtungsgewicht", format!("entfernungsabhängig, 1 bei {} m", f(s.dir_weight_dist, 0))));
    }
    v.push(("Ausreißertest", format!("normierte Verbesserung > {}", f(s.k_crit, 2))));
    doc.kv(v);
}

fn part_blocks(doc: &mut Doc, part: &PartResult, s: &Settings) {
    let nk = s.nk;
    doc.blocks.push(Block::Title(part.title.clone()));
    // Statistik
    doc.h("Statistik");
    let mut v = vec![
        ("Beobachtungen", part.n_obs.to_string()),
        ("Unbekannte", part.n_unknown.to_string()),
        ("Datumsdefekt", part.defect.to_string()),
        ("Redundanz", part.redundancy.to_string()),
        ("[pvv]", f(part.pvv, 4)),
        ("Standardabweichung s0", f(part.sigma0, 3)),
        (
            "Globaltest (χ², 5 %)",
            match part.chi2_ok {
                Some(true) => "bestanden".into(),
                Some(false) => "nicht bestanden – a-priori-Genauigkeiten oder grobe Fehler prüfen".into(),
                None => "-".into(),
            },
        ),
        ("Iterationen", format!("{}{}", part.iterations, if part.converged { "" } else { " (nicht konvergiert!)" })),
    ];
    if part.robust_iterations > 0 {
        v.push(("robuste Iterationen", part.robust_iterations.to_string()));
    }
    if let Some(c) = part.cond {
        v.push(("Konditionszahl (SVD)", format!("{:.3e}", c)));
    }
    if let Some((m, sm)) = part.scale_ppm {
        v.push(("Maßstabsverbesserung dM", format!("{} ppm ± {} ppm", f(m, 2), f(sm, 2))));
    }
    v.push(("Ausreißer", part.outliers().to_string()));
    doc.kv(v);
    for n in &part.notes {
        doc.blocks.push(Block::Note(n.clone(), Tone::Warn));
    }
    if !part.struck.is_empty() {
        doc.h("Vor der Ausgleichung gestrichen (Grenzwerte)");
        let rows = part.struck.iter().map(|(k, r)| (vec![k.clone(), r.clone()], Tone::Warn)).collect();
        doc.blocks.push(Block::Table(Table { cols: vec![("Schlüssel".into(), Align::L, 0.2), ("Grund".into(), Align::L, 0.8)], rows }));
    }
    // Orientierungen
    if !part.orient.is_empty() {
        doc.h("Orientierungsunbekannte (t-r)");
        let rows = part.orient.iter().map(|(n, o, so)| (vec![n.clone(), f(*o, 5), f(*so, 2)], Tone::Normal)).collect();
        doc.blocks.push(Block::Table(Table { cols: vec![("Standpunkt".into(), Align::L, 0.4), ("Orientierung [gon]".into(), Align::R, 0.35), ("σ [mgon]".into(), Align::R, 0.25)], rows }));
    }
    // Verbesserungen
    doc.h("Beobachtungen und Verbesserungen");
    doc.blocks.push(Block::Para("Redundanzanteile: # 0,03 < r ≤ 0,1   ## r ≤ 0,03 (schlecht kontrolliert);  * Ausreißer (NV > kritischer Wert bzw. robust abgewertet)".into()));
    let mut rows = Vec::new();
    for o in &part.obs {
        let ob = &part.net.obs[o.obs];
        let (st, tg) = (&part.net.pts[ob.from].name, &part.net.pts[ob.to].name);
        let ang = ob.kind.angular();
        let (val, vv, sa, gf) = if ang {
            (f(ob.value, 5), f(o.v * 1000.0, 2), f(o.sigma_adj * 1000.0, 2), f(o.gf * 1000.0, 1))
        } else {
            (f(ob.value, 4), f(o.v * 1000.0, 2), f(o.sigma_adj * 1000.0, 2), f(o.gf * 1000.0, 1))
        };
        let tone = if o.outlier {
            Tone::Bad
        } else if o.w < 0.9 {
            Tone::Warn
        } else {
            Tone::Normal
        };
        let kz = format!("{}{}", o.mark(), if o.outlier { " *" } else { "" });
        rows.push((
            vec![
                st.clone(),
                if matches!(ob.kind, ObsKind::CoordY | ObsKind::CoordX | ObsKind::CoordH) { String::new() } else { tg.clone() },
                ob.kind.label().into(),
                val,
                vv,
                ob.kind.unit().into(),
                sa,
                f(o.redundancy, 2),
                f(o.nv, 2),
                gf,
                f(o.w, 2),
                kz,
            ],
            tone,
        ));
    }
    doc.blocks.push(Block::Table(Table {
        cols: vec![
            ("Stand".into(), Align::L, 0.09),
            ("Ziel".into(), Align::L, 0.09),
            ("Art".into(), Align::L, 0.12),
            ("Beobachtung".into(), Align::R, 0.12),
            ("v".into(), Align::R, 0.07),
            ("Einh.".into(), Align::L, 0.06),
            ("σ ausg.".into(), Align::R, 0.07),
            ("r".into(), Align::R, 0.06),
            ("NV".into(), Align::R, 0.06),
            ("GF".into(), Align::R, 0.07),
            ("w".into(), Align::R, 0.06),
            ("Kz".into(), Align::L, 0.05),
        ],
        rows,
    }));
    // Koordinaten
    let plan = part.net.plan;
    let height = part.net.height;
    doc.h(if plan && height {
        "Ausgeglichene Koordinaten und Höhen"
    } else if plan {
        "Ausgeglichene Koordinaten, Standardabweichungen und Koordinatenänderungen"
    } else {
        "Ausgeglichene Höhen, Standardabweichungen und Höhenänderungen"
    });
    let mut cols: Vec<(String, Align, f32)> = vec![("Punkt".into(), Align::L, 0.1)];
    if plan {
        cols.extend([("Rechtswert Y".into(), Align::R, 0.13), ("Hochwert X".into(), Align::R, 0.13)]);
    }
    if height {
        cols.push(("Höhe H".into(), Align::R, 0.09));
    }
    if plan {
        cols.extend([("sy".into(), Align::R, 0.06), ("sx".into(), Align::R, 0.06), ("A".into(), Align::R, 0.06), ("B".into(), Align::R, 0.06), ("θ [gon]".into(), Align::R, 0.07)]);
    }
    if height {
        cols.push(("sh".into(), Align::R, 0.06));
    }
    if plan {
        cols.extend([("dy".into(), Align::R, 0.06), ("dx".into(), Align::R, 0.06)]);
    }
    if height {
        cols.push(("dh".into(), Align::R, 0.06));
    }
    cols.push(("Art".into(), Align::L, 0.06));
    let mut rows = Vec::new();
    for p in &part.points {
        let adj = (plan && p.plan_adj) || (height && p.h_adj);
        let fixed = (!plan || p.fix_plan) && (!height || p.fix_h);
        if !adj && !fixed {
            continue;
        }
        let mut r = vec![p.name.clone()];
        if plan {
            r.extend([f(p.y, nk), f(p.x, nk)]);
        }
        if height {
            r.push(if p.h.is_finite() { f(p.h, nk) } else { "-".into() });
        }
        let mmv = |v: f64, on: bool| if on { f(v * 1000.0, 1) } else { String::new() };
        if plan {
            r.extend([mmv(p.sy, p.plan_adj), mmv(p.sx, p.plan_adj), mmv(p.ea, p.plan_adj), mmv(p.eb, p.plan_adj), if p.plan_adj { f(p.et, 1) } else { String::new() }]);
        }
        if height {
            r.push(mmv(p.sh, p.h_adj));
        }
        if plan {
            r.extend([mmv(p.dy, p.plan_adj), mmv(p.dx, p.plan_adj)]);
        }
        if height {
            r.push(mmv(p.dh, p.h_adj));
        }
        r.push(if fixed && !adj { "AP".into() } else if fixed { "AP*".into() } else { "NP".into() });
        rows.push((r, if adj { Tone::Normal } else { Tone::Muted }));
    }
    doc.blocks.push(Block::Table(Table { cols, rows }));
    doc.blocks.push(Block::Para("Standardabweichungen, Ellipsenhalbachsen A/B und Änderungen gegenüber Näherung bzw. Altkoordinate in mm; θ = Richtungswinkel der großen Halbachse; AP* = Altpunkt, der als Beobachtung bzw. im freien Netz mitbestimmt wurde.".into()));
    // Ausreißer
    let outl: Vec<_> = part.obs.iter().filter(|o| o.outlier).collect();
    if !outl.is_empty() {
        doc.h("Ausreißer / grobe Fehler");
        let rows = outl
            .iter()
            .map(|o| {
                let ob = &part.net.obs[o.obs];
                (
                    vec![
                        format!("{} → {}", part.net.pts[ob.from].name, part.net.pts[ob.to].name),
                        ob.kind.label().into(),
                        if ob.line > 0 { ob.line.to_string() } else { String::new() },
                        format!("{} {}", f(o.gf * 1000.0, 1), ob.kind.unit()),
                        f(o.nv, 2),
                        f(o.w, 3),
                    ],
                    Tone::Bad,
                )
            })
            .collect();
        doc.blocks.push(Block::Table(Table {
            cols: vec![("Beobachtung".into(), Align::L, 0.28), ("Art".into(), Align::L, 0.16), ("Zeile".into(), Align::R, 0.1), ("geschätzter Fehler".into(), Align::R, 0.18), ("NV".into(), Align::R, 0.12), ("w".into(), Align::R, 0.12)], rows }));
    }
}

/// Protokoll einer Ausgleichung
pub fn adjustment_doc(out: &Outcome, s: &Settings, proj: &ProjectFile, firm: &[String]) -> Doc {
    let mut doc = Doc { header: header(proj, firm), blocks: vec![] };
    let title = match out.kind {
        crate::io::NetKind::Lage => "Ausgleichung eines Lagenetzes nach vermittelnden Beobachtungen",
        crate::io::NetKind::Niv | crate::io::NetKind::Trig => "Ausgleichung eines Höhennetzes nach vermittelnden Beobachtungen",
        crate::io::NetKind::LageHoehe => "Lageausgleichung kombiniert mit Höhenausgleichung",
        crate::io::NetKind::Raum3D => "Räumliche Netzausgleichung (3D)",
        crate::io::NetKind::Vorwaerts => "Räumliche Vorwärtsschnitte",
    };
    doc.blocks.push(Block::Title(title.into()));
    project_block(&mut doc, proj);
    settings_block(&mut doc, s, proj);
    if !out.approx_log.is_empty() {
        doc.h("Näherungswerte");
        doc.blocks.push(Block::Para(out.approx_log.join("; ")));
    }
    for p in &out.parts {
        part_blocks(&mut doc, p, s);
    }
    doc
}

pub fn set_mean_doc(st: &[SetMeanStation], proj: &ProjectFile, firm: &[String]) -> Doc {
    let mut doc = Doc { header: header(proj, firm), blocks: vec![Block::Title("Satzmittelbildung".into())] };
    for s in st {
        doc.h(&format!("Standpunkt {}  (i = {} m, {} Sätze, s Richtung = {} mgon)", s.station, f(s.ih, 3), s.n_sets, f(s.s_r_mgon, 2)));
        let mut cols = vec![("Ziel".into(), Align::L, 0.12), ("Hz-Mittel [gon]".into(), Align::R, 0.15), ("s [mgon]".into(), Align::R, 0.09), ("Zenit [gon]".into(), Align::R, 0.13), ("Index [mgon]".into(), Align::R, 0.1), ("Strecke [m]".into(), Align::R, 0.12)];
        for k in 0..s.n_sets {
            cols.push((format!("v{} [mgon]", k + 1), Align::R, 0.29 / s.n_sets as f32));
        }
        let rows = s
            .rows
            .iter()
            .map(|r| {
                let mut v = vec![r.target.clone(), f(r.hz, 5), f(r.s_hz_mgon, 2), r.zenith.map(|z| f(z, 5)).unwrap_or_default(), r.index_err_mgon.map(|z| f(z, 1)).unwrap_or_default(), r.dist.map(|z| f(z, 4)).unwrap_or_default()];
                for x in &r.v {
                    v.push(x.map(|x| f(x, 2)).unwrap_or_default());
                }
                (v, Tone::Normal)
            })
            .collect();
        doc.blocks.push(Block::Table(Table { cols, rows }));
        for w in &s.warnings {
            doc.blocks.push(Block::Note(w.clone(), Tone::Warn));
        }
    }
    doc
}

pub fn trans_doc(t: &TransResult, src: &str, dst: &str, proj: &ProjectFile, firm: &[String]) -> Doc {
    let mut doc = Doc { header: header(proj, firm), blocks: vec![Block::Title("Räumliche Transformation (Helmert)".into())] };
    doc.kv(vec![
        ("Startsystem", src.to_string()),
        ("Zielsystem", dst.to_string()),
        ("Parameter", if t.with_scale { "7 (3 Verschiebungen, 3 Drehungen, Maßstab)".into() } else { "6 (3 Verschiebungen, 3 Drehungen)".into() }),
        ("identische Punkte", t.identical.to_string()),
        ("Verschiebung Y / X / H", format!("{} / {} / {} m", f(t.t[0], 4), f(t.t[1], 4), f(t.t[2], 4))),
        ("Drehungen ω / φ / κ", format!("{} / {} / {} gon", f(t.angles[0], 6), f(t.angles[1], 6), f(t.angles[2], 6))),
        ("Maßstab", format!("{} ({} ppm)", f(t.scale, 9), f((t.scale - 1.0) * 1e6, 2))),
        ("s0 (Punkt, je Koordinate)", format!("{} mm", f(t.s0 * 1000.0, 1))),
    ]);
    doc.h("Restklaffen an den identischen Punkten");
    let rows = t.residuals.iter().map(|r| (vec![r.0.clone(), f(r.1 * 1000.0, 1), f(r.2 * 1000.0, 1), f(r.3 * 1000.0, 1)], if (r.1 * r.1 + r.2 * r.2 + r.3 * r.3).sqrt() > 3.0 * t.s0.max(1e-9) { Tone::Bad } else { Tone::Normal })).collect();
    doc.blocks.push(Block::Table(Table { cols: vec![("Punkt".into(), Align::L, 0.25), ("vY [mm]".into(), Align::R, 0.25), ("vX [mm]".into(), Align::R, 0.25), ("vH [mm]".into(), Align::R, 0.25)], rows }));
    doc.h("Transformierte Punkte");
    let rows = t.transformed.iter().map(|p| (vec![p.name.clone(), f(p.y, 4), f(p.x, 4), f(p.h.unwrap_or(0.0), 4)], Tone::Normal)).collect();
    doc.blocks.push(Block::Table(Table { cols: vec![("Punkt".into(), Align::L, 0.25), ("Y".into(), Align::R, 0.25), ("X".into(), Align::R, 0.25), ("H".into(), Align::R, 0.25)], rows }));
    doc
}
