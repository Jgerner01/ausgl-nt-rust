//! Messdaten-Editor: rechte Fensterhälfte mit editierbaren Tabellen der Messungen und Punkte.
//! Ein Klick auf eine Beobachtungslinie bzw. einen Punkt im Netzbild markiert die zugehörigen Zeilen.
//! Beobachtungen werden einzeln ein- und ausgeschaltet, Punkte als Fest- oder Neupunkt festgelegt.
//! Gespeichert wird als AUSGL-NT-Netzdatei (*.NET, siehe `netfile`).

use super::*;
use crate::adjust::{obs_key, ObsKind};
use crate::netfile::{self, Art, NetData, NetPoint};

/// Zeilenhöhe der Tabellen [Punkte]
const ROW_H: f32 = 22.0;
/// Abstand zwischen den Zeilen
const ROW_GAP: f32 = 2.0;
/// Zeilen oberhalb der ersten markierten Zeile beim Hinscrollen
const SCROLL_VORLAUF: f32 = 3.0;
/// Fangradius im Netzbild [Pixel]: Punkt bzw. Linie
const PICK_POINT_PX: f32 = 10.0;
const PICK_LINE_PX: f32 = 6.0;
/// Aufteilung Hauptfläche / Editor: Breite der Trennlinie und Mindestbreiten [Punkte]
const SPLIT_W: f32 = 8.0;
const MIN_LEFT: f32 = 280.0;
const MIN_RIGHT: f32 = 360.0;
/// Standardanteil des Editors an der Fensterbreite (rechte Fensterhälfte)
const EDITOR_ANTEIL: f32 = 0.5;
/// Markierungsfarben
const SEL: Color32 = Color32::from_rgb(245, 158, 11);
const SEL_ROW: Color32 = Color32::from_rgb(254, 243, 199);
const SETUP_ROW: Color32 = Color32::from_rgb(241, 245, 249);
/// Spaltenbreiten [Punkte]
const W_PICK: f32 = 20.0;
const W_NAME: f32 = 80.0;
const W_ANGLE: f32 = 88.0;
const W_DIST: f32 = 84.0;
const W_STAR: f32 = 12.0;
const W_V: f32 = 84.0;
const W_TH: f32 = 64.0;
const W_CODE: f32 = 52.0;
const W_FLAG: f32 = 22.0;
const W_SIGMA: f32 = 50.0;
const W_COORD: f32 = 118.0;
const W_H: f32 = 90.0;
const W_ART: f32 = 24.0;
/// Nachkommastellen in der Tabelle
const NK_WINKEL: usize = 5;
const NK_STRECKE: usize = 4;
const NK_HOEHE: usize = 4;
const NK_IH: usize = 3;
const NK_SIGMA: usize = 3;
/// Umrechnung mgon/mm ↔ gon/m
const MILLI: f64 = 1000.0;

pub(super) const NET_F: &[(&str, &[&str])] = &[("AUSGL-NT-Netzdatei", &["net", "NET"])];

#[derive(Clone, PartialEq, Default)]
pub(super) enum Sel {
    #[default]
    None,
    /// Beobachtungslinie zwischen zwei Punkten
    Line(String, String),
    Point(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum EdTab {
    #[default]
    Mess,
    Punkte,
}

#[derive(Default)]
pub(super) struct EditorState {
    pub open: bool,
    pub tab: EdTab,
    pub sel: Sel,
    /// einmalig zur ersten markierten Zeile scrollen
    pub scroll: bool,
    /// nur markierte Zeilen zeigen
    pub only_sel: bool,
    /// Anteil des Editors an der Fensterbreite (0 = Standard)
    pub frac: f32,
}

/// Zeile der Messungstabelle
#[derive(Clone, Copy)]
enum MRow {
    Setup(usize),
    Shot(usize, usize),
    NivHead,
    Niv(usize),
}

/// Meßdatei ist eine AUSGL-NT-Netzdatei (*.NET)
pub(super) fn is_netfile(f: &str) -> bool {
    Path::new(f.trim()).extension().map_or(false, |e| e.eq_ignore_ascii_case("net"))
}

impl Sel {
    fn pair(&self, a: &str, b: &str) -> bool {
        match self {
            Sel::Line(p, q) => (p == a && q == b) || (p == b && q == a),
            Sel::Point(p) => p == a || p == b,
            Sel::None => false,
        }
    }
    fn point(&self, n: &str) -> bool {
        matches!(self, Sel::Point(p) if p == n)
    }
}

/// feste Id einer Tabellenzelle (bleibt über Bildwechsel gleich)
fn cell_id(salt: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
    egui::Id::NULL.with(salt)
}

/// Abstand eines Punktes von einer Strecke
fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Zahlenfeld: gibt Some(neuer Wert) zurück, wenn die Eingabe übernommen wurde ('-' oder leer = kein Wert)
fn num_cell(ui: &mut Ui, id: egui::Id, val: Option<f64>, nk: usize, w: f32) -> Option<Option<f64>> {
    let fmt = |v: Option<f64>| v.map(|v| format!("{:.*}", nk, v)).unwrap_or_else(|| "-".into());
    let mut s = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| fmt(val));
    let r = ui.add_sized([w, ROW_H], egui::TextEdit::singleline(&mut s).id(id).align(Align2::RIGHT_CENTER).font(egui::TextStyle::Monospace));
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, s.clone()));
    }
    if r.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        let t = s.trim();
        let new = if t.is_empty() || t == "-" { Some(None) } else { t.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite()).map(Some) };
        if let Some(n) = new {
            if fmt(n) != fmt(val) {
                return Some(n);
            }
        }
    }
    None
}

/// Textfeld (Punktnummer, Code, Bemerkung): Some(neuer Text) nach Übernahme
fn text_cell(ui: &mut Ui, id: egui::Id, val: &str, w: f32, allow_empty: bool) -> Option<String> {
    let mut s = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| val.to_string());
    let r = ui.add_sized([w, ROW_H], egui::TextEdit::singleline(&mut s).id(id).font(egui::TextStyle::Monospace));
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, s.clone()));
    }
    if r.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        // Leerzeichen trennen Felder in der Datei – in Namen und Codes nicht erlaubt
        let t = if allow_empty { s.trim().to_string() } else { s.split_whitespace().collect::<Vec<_>>().join("_") };
        if t != val && (allow_empty || !t.is_empty()) {
            return Some(t);
        }
    }
    None
}

/// Schalter einer Beobachtung (an = berücksichtigt)
fn flag(ui: &mut Ui, present: bool, on: bool, letter: &str, tip: &str) -> bool {
    if !present {
        ui.add_sized([W_FLAG, ROW_H], egui::Label::new(RichText::new("·").color(MUTED)));
        return false;
    }
    let txt = RichText::new(letter).strong().color(if on { Color32::WHITE } else { BAD });
    let b = egui::Button::new(txt).fill(if on { GOOD } else { Color32::from_rgb(254, 226, 226) });
    ui.add_sized([W_FLAG, ROW_H], b).on_hover_text(format!("{} – {}", tip, if on { "wird berücksichtigt (Klick = streichen)" } else { "gestrichen (Klick = wieder verwenden)" })).clicked()
}

fn head(ui: &mut Ui, w: f32, t: &str, tip: &str) {
    let r = ui.add_sized([w, ROW_H], egui::Label::new(RichText::new(t).small().strong().color(MUTED)));
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

/// Hintergrund einer Tabellenzeile hinter den Inhalt legen
fn row_bg<R>(ui: &mut Ui, fill: Option<Color32>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let idx = ui.painter().add(egui::Shape::Noop);
    let r = ui.horizontal(add);
    if let Some(c) = fill {
        ui.painter().set(idx, egui::Shape::rect_filled(r.response.rect.expand2(Vec2::new(2.0, 1.0)), 3, c));
    }
    r.inner
}

impl AusglApp {
    // -------------------------------------------------------------- Punkte

    /// Punkt so, wie er in der Netzdatei stehen würde (Fest- oder Näherungskoordinaten)
    fn net_point(&self, n: &str) -> NetPoint {
        let k = self.kor.iter().find(|p| p.name == n);
        let a = self.approx.iter().find(|p| p.name == n);
        let lk = self.lagekor.iter().find(|p| p.name == n);
        let pm = self.approx_pm.get(n).copied().unwrap_or_default();
        let finite = |p: &&KorPoint| p.y.is_finite() && p.x.is_finite();
        let plan = k.filter(finite).map(|p| (p.y, p.x)).or_else(|| a.filter(finite).map(|p| (p.y, p.x))).or_else(|| lk.map(|p| (p.y, p.x))).or(pm.plan);
        let h = k.and_then(|p| p.h).or_else(|| a.and_then(|p| p.h)).or(pm.h);
        let art = if self.hfix.iter().any(|x| x == n) {
            Art::H
        } else if let Some(k) = k {
            if k.h.is_some() {
                Art::F
            } else {
                Art::L
            }
        } else {
            Art::N
        };
        let code = k.or(a).map(|p| p.code.clone()).filter(|c| c != "NP" && c != "HP").unwrap_or_default();
        NetPoint { name: n.to_string(), y: plan.map(|p| p.0), x: plan.map(|p| p.1), h, art, code }
    }

    /// alle Punkte des Netzes: Fest- und Näherungspunkte, dann Punkte aus den Beobachtungen
    pub(super) fn net_points(&self) -> Vec<NetPoint> {
        let mut names: Vec<String> = Vec::new();
        let mut add = |n: &str| {
            if !names.iter().any(|x| x == n) {
                names.push(n.to_string());
            }
        };
        for p in self.kor.iter().chain(&self.approx).chain(&self.lagekor) {
            add(&p.name);
        }
        for st in &self.setups {
            add(&st.station);
            for sh in &st.shots {
                add(&sh.target);
            }
        }
        for n in &self.niv {
            add(&n.from);
            add(&n.to);
        }
        names.iter().map(|n| self.net_point(n)).collect()
    }

    /// geänderten Punkt in Festpunkte / Näherungen / Höhenfestpunkte einsortieren
    fn set_net_point(&mut self, p: NetPoint) {
        self.kor.retain(|k| k.name != p.name);
        self.approx.retain(|k| k.name != p.name);
        self.lagekor.retain(|k| k.name != p.name);
        self.kon.retain(|k| k.name != p.name);
        self.hfix.retain(|k| *k != p.name);
        let (kor, approx, hfix, notes) = netfile::split_points(&[p]);
        self.kor.extend(kor);
        self.approx.extend(approx);
        self.hfix.extend(hfix);
        if let Some(n) = notes.first() {
            self.status = n.clone();
        }
    }

    // -------------------------------------------------------------- Laden / Speichern

    /// Netzdatei des Projekts einlesen
    pub(super) fn load_netfile(&mut self) -> Option<NetData> {
        match self.read_list(&self.proj.mes) {
            Ok(Some(t)) => {
                let d = netfile::parse(&t);
                let (kor, approx, hfix, notes) = netfile::split_points(&d.points);
                self.kor = kor;
                self.approx = approx;
                self.hfix = hfix;
                self.kon.clear();
                self.lagekor.clear();
                self.setups = d.setups.clone();
                self.niv = d.niv.clone();
                // Streichungen stehen in der Datei; nur namensbezogene Schlüssel (Altpunkte als Beobachtung) bleiben
                self.proj.disabled.retain(|k| k.starts_with('P'));
                self.proj.disabled.extend(d.disabled.iter().cloned());
                self.raw = [String::new(), t, String::new()];
                self.after_load();
                self.import_notes = d.notes.iter().chain(&notes).cloned().collect();
                self.status = format!("Netzdatei eingelesen: {}.", d.summary());
                Some(d)
            }
            Ok(None) => None,
            Err(e) => {
                self.status = format!("Fehler beim Einlesen: {}", e);
                None
            }
        }
    }

    /// Netzdatei als neues Projekt öffnen
    pub(super) fn import_netfile(&mut self, p: &Path) {
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        self.proj = ProjectFile { name: stem, mes: p.to_string_lossy().to_string(), settings: self.default_settings(), ..Default::default() };
        self.proj_path = None;
        self.sample_mode = false;
        self.sys.last_dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        self.save_sys();
        let Some(d) = self.load_netfile() else { return };
        let has_h = !d.niv.is_empty() || d.setups.iter().flat_map(|s| &s.shots).any(|s| s.zenith.is_some() || s.dh.is_some());
        self.proj.kind = if d.setups.is_empty() && !d.niv.is_empty() {
            NetKind::Niv
        } else if has_h {
            NetKind::LageHoehe
        } else {
            NetKind::Lage
        };
        if self.kor.is_empty() && self.hfix.is_empty() {
            self.proj.settings.variant = Variant::FreiesNetz;
        }
        self.proj.beschreibung = "Netzdatei".into();
        self.refresh_preview();
        self.status = format!("Netzdatei {} geöffnet ({}, {}). Als *.ausgl speichern, um die Einstellungen zu behalten.", p.display(), self.proj.kind.label(), d.summary());
    }

    /// Tabelle als Netzdatei speichern; das Projekt liest danach diese Datei
    pub(super) fn save_netfile(&mut self, ask: bool) {
        let current = if is_netfile(&self.proj.mes) && !self.sample_mode { self.proj.resolve(&self.base_dir(), &self.proj.mes) } else { None };
        let path = if ask || current.is_none() { save_dialog("Netzdatei speichern", &format!("{}.net", self.proj.name), NET_F, &self.dir_str()) } else { current };
        let Some(path) = path else { return };
        let text = netfile::write(&self.proj.name, &self.net_points(), &self.setups, &self.niv, &self.proj.disabled);
        // vorige Fassung als Sicherung behalten
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("net.bak"));
        }
        if let Err(e) = std::fs::write(&path, text) {
            self.status = format!("Speichern fehlgeschlagen: {}", e);
            return;
        }
        self.proj.mes = path.to_string_lossy().to_string();
        self.proj.kor.clear();
        self.proj.niv.clear();
        self.proj.lagekor.clear();
        self.sample_mode = false;
        if self.proj_path.is_some() {
            self.save_project(false);
        }
        let had_result = self.outcome.is_some();
        self.load_data();
        self.rerun = had_result;
        self.status = format!(
            "Netzdatei gespeichert: {}{}",
            path.display(),
            if self.proj_path.is_some() { " – Projekt verweist jetzt auf diese Datei." } else { " – Projekt noch nicht gespeichert (Datei ▸ Projekt speichern)." }
        );
    }

    /// nach jeder Änderung: neu rechnen, wenn schon ein Ergebnis da ist, sonst Vorschau auffrischen
    fn after_edit(&mut self) {
        self.data_dirty = true;
        if self.outcome.is_some() {
            self.rerun = true;
        } else {
            self.refresh_preview();
        }
    }

    fn toggle_key(&mut self, k: String) {
        if let Some(i) = self.proj.disabled.iter().position(|d| *d == k) {
            self.proj.disabled.remove(i);
        } else {
            self.proj.disabled.push(k);
        }
        self.after_edit();
    }

    // -------------------------------------------------------------- Netzbild

    /// Klick ins Netzbild: Punkt oder Beobachtungslinie auswählen
    pub(super) fn pick_at(&mut self, hp: Pos2, to: &dyn Fn([f64; 2]) -> Pos2) {
        let v = &self.view;
        let pt = v.pts.iter().map(|p| (p, to([p.y, p.x]).distance(hp))).filter(|(_, d)| *d < PICK_POINT_PX).min_by(|a, b| a.1.total_cmp(&b.1));
        let sel = if let Some((p, _)) = pt {
            Sel::Point(p.name.clone())
        } else {
            let line = v
                .lines
                .iter()
                .map(|l| (l, seg_dist(hp, to([v.pts[l.a].y, v.pts[l.a].x]), to([v.pts[l.b].y, v.pts[l.b].x]))))
                .filter(|(_, d)| *d < PICK_LINE_PX)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match line {
                Some((l, _)) => Sel::Line(v.pts[l.a].name.clone(), v.pts[l.b].name.clone()),
                None => Sel::None,
            }
        };
        if sel != Sel::None {
            self.editor.open = true;
            self.editor.tab = if matches!(sel, Sel::Point(_)) { EdTab::Punkte } else { EdTab::Mess };
            self.editor.scroll = true;
        }
        self.editor.sel = sel;
    }

    /// Auswahl im Netzbild hervorheben
    pub(super) fn draw_selection(&self, painter: &egui::Painter, to: &dyn Fn([f64; 2]) -> Pos2) {
        let v = &self.view;
        let pos = |n: &str| v.pts.iter().find(|p| p.name == n).map(|p| to([p.y, p.x]));
        match &self.editor.sel {
            Sel::Line(a, b) => {
                if let (Some(pa), Some(pb)) = (pos(a), pos(b)) {
                    painter.line_segment([pa, pb], Stroke::new(5.0, SEL.gamma_multiply(0.7)));
                    for p in [pa, pb] {
                        painter.circle_stroke(p, 8.0, Stroke::new(2.0, SEL));
                    }
                }
            }
            Sel::Point(n) => {
                if let Some(p) = pos(n) {
                    painter.circle_stroke(p, 11.0, Stroke::new(3.0, SEL));
                }
            }
            Sel::None => {}
        }
    }

    /// Hauptfläche in links (Reiter) und rechts (Editor) teilen; Trennlinie mit der Maus verschiebbar
    pub(super) fn split_editor(&mut self, ui: &mut Ui) -> (Rect, Rect) {
        let full = ui.available_rect_before_wrap();
        // rechter Rand der Hauptfläche = Fensterrand; der Editor nimmt einen Anteil der Fensterbreite ein
        let win_w = full.right();
        let frac = if self.editor.frac > 0.0 { self.editor.frac } else { EDITOR_ANTEIL };
        let lo = full.left() + MIN_LEFT;
        let hi = (full.right() - MIN_RIGHT).max(lo);
        let mut x = (win_w * (1.0 - frac)).clamp(lo, hi);
        let handle = Rect::from_x_y_ranges(x - SPLIT_W / 2.0..=x + SPLIT_W / 2.0, full.y_range());
        let resp = ui.interact(handle, cell_id("editor_split"), Sense::drag());
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                x = p.x.clamp(lo, hi);
                self.editor.frac = 1.0 - x / win_w;
            }
        }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        let active = resp.hovered() || resp.dragged();
        ui.painter().line_segment([Pos2::new(x, full.top()), Pos2::new(x, full.bottom())], Stroke::new(if active { 3.0 } else { 1.0 }, if active { ACCENT } else { BORDER }));
        let left = Rect::from_min_max(full.min, Pos2::new(x - SPLIT_W / 2.0, full.max.y));
        let right = Rect::from_min_max(Pos2::new(x + SPLIT_W / 2.0, full.min.y), full.max);
        (left, right)
    }

    // -------------------------------------------------------------- Tabellen

    pub(super) fn editor_panel(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Messdaten").strong().size(15.0));
            for (t, l) in [(EdTab::Mess, "Messungen"), (EdTab::Punkte, "Punkte")] {
                if ui.selectable_label(self.editor.tab == t, l).clicked() {
                    self.editor.tab = t;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").on_hover_text("Tabelle schließen").clicked() {
                    self.editor.open = false;
                }
                if ui.button("Speichern unter…").clicked() {
                    self.save_netfile(true);
                }
                let save = ui.button(RichText::new("💾 Speichern").color(if self.data_dirty { ACCENT } else { INK }));
                if save.on_hover_text("Punkte und Messungen als Netzdatei (*.NET) speichern").clicked() {
                    self.save_netfile(false);
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.editor.only_sel, "nur markierte Zeilen");
            let txt = match &self.editor.sel {
                Sel::Line(a, b) => format!("markiert: Beobachtungen {} – {}", a, b),
                Sel::Point(p) => format!("markiert: Punkt {}", p),
                Sel::None => "Linie oder Punkt im Netzbild anklicken, um Zeilen zu markieren".into(),
            };
            ui.label(RichText::new(txt).small().color(if self.editor.sel == Sel::None { MUTED } else { SEL }));
            if self.editor.sel != Sel::None && ui.small_button("Markierung aufheben").clicked() {
                self.editor.sel = Sel::None;
            }
        });
        if self.data_dirty {
            ui.label(RichText::new("Geändert – noch nicht als Netzdatei gespeichert.").small().color(WARN));
        }
        ui.separator();
        ui.spacing_mut().item_spacing = Vec2::new(4.0, ROW_GAP);
        match self.editor.tab {
            EdTab::Mess => self.mess_table(ui),
            EdTab::Punkte => self.point_table(ui),
        }
    }

    fn mess_rows(&self) -> Vec<(MRow, bool)> {
        let sel = &self.editor.sel;
        let mut rows = Vec::new();
        for (si, st) in self.setups.iter().enumerate() {
            let mut block = Vec::new();
            for (j, sh) in st.shots.iter().enumerate() {
                block.push((MRow::Shot(si, j), sel.pair(&st.station, &sh.target)));
            }
            let any = block.iter().any(|b| b.1);
            if self.editor.only_sel && !any {
                continue;
            }
            rows.push((MRow::Setup(si), sel.point(&st.station)));
            rows.extend(block.into_iter().filter(|b| !self.editor.only_sel || b.1));
        }
        let niv: Vec<(MRow, bool)> = self.niv.iter().enumerate().map(|(k, n)| (MRow::Niv(k), sel.pair(&n.from, &n.to))).filter(|r| !self.editor.only_sel || r.1).collect();
        if !niv.is_empty() {
            rows.push((MRow::NivHead, false));
            rows.extend(niv);
        }
        rows
    }

    fn mess_table(&mut self, ui: &mut Ui) {
        if self.setups.is_empty() && self.niv.is_empty() {
            ui.label(RichText::new("Keine Messungen geladen.").color(MUTED));
            return;
        }
        let rows = self.mess_rows();
        let scroll = std::mem::take(&mut self.editor.scroll);
        // Überschrift und Zeilen scrollen gemeinsam waagrecht – die Tabelle bestimmt nicht die Fensterbreite
        egui::ScrollArea::horizontal().id_salt("mess_h").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                head(ui, W_PICK, "", "");
                head(ui, W_NAME, "Ziel", "Zielpunkt (Standpunktzeile: Standpunkt)");
                head(ui, W_ANGLE, "Hz [gon]", "Horizontalrichtung");
                head(ui, W_DIST, "Strecke [m]", "Schrägstrecke; * = Horizontalstrecke");
                head(ui, W_STAR, "", "");
                head(ui, W_V, "V / dH", "Zenitwinkel [gon] bzw. Höhenunterschied [m] bei Horizontalstrecke");
                head(ui, W_TH, "Zielh.", "Zielhöhe [m]");
                head(ui, W_CODE, "Code", "");
                head(ui, W_FLAG * 3.0 + 8.0, "R  S  H", "Richtung, Strecke, Höhe verwenden");
                head(ui, W_SIGMA, "σHz", "eigene Standardabweichung Richtung [mgon]; - = Voreinstellung");
                head(ui, W_SIGMA, "σS", "eigene Standardabweichung Strecke [mm]; - = Voreinstellung");
                head(ui, W_SIGMA, "σV", "eigene Standardabweichung Zenitwinkel [mgon] bzw. dH [mm]; - = Voreinstellung");
            });
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("mess");
            if scroll {
                if let Some(i) = rows.iter().position(|r| r.1) {
                    area = area.vertical_scroll_offset(((i as f32 - SCROLL_VORLAUF).max(0.0)) * (ROW_H + ROW_GAP));
                }
            }
            area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for &(row, hl) in &rows[range] {
                    let fill = if hl { Some(SEL_ROW) } else if matches!(row, MRow::Setup(_) | MRow::NivHead) { Some(SETUP_ROW) } else { None };
                    row_bg(ui, fill, |ui| self.mess_row(ui, row));
                }
            });
        });
    }

    fn mess_row(&mut self, ui: &mut Ui, row: MRow) {
        match row {
            MRow::Setup(si) => {
                let st = &self.setups[si];
                let (station, ih, remark) = (st.station.clone(), st.ih, st.remark.clone());
                if ui.add_sized([W_PICK, ROW_H], egui::Button::new("◉").frame(false)).on_hover_text("Standpunkt im Netzbild markieren").clicked() {
                    self.editor.sel = Sel::Point(station.clone());
                }
                if let Some(n) = text_cell(ui, cell_id(("st", si, "name")), &station, W_NAME, false) {
                    self.setups[si].station = n;
                    self.after_edit();
                }
                ui.add_sized([W_ANGLE, ROW_H], egui::Label::new(RichText::new("Standpunkt  i =").small().color(MUTED)));
                if let Some(Some(v)) = num_cell(ui, cell_id(("st", si, "ih")), Some(ih), NK_IH, W_DIST) {
                    self.setups[si].ih = v;
                    self.after_edit();
                }
                ui.add_sized([W_STAR, ROW_H], egui::Label::new(""));
                if let Some(t) = text_cell(ui, cell_id(("st", si, "rem")), &remark, W_V + W_TH + W_CODE + 8.0, true) {
                    self.setups[si].remark = t;
                    self.data_dirty = true;
                }
            }
            MRow::Shot(si, j) => {
                let station = self.setups[si].station.clone();
                let sh = self.setups[si].shots[j].clone();
                let id = |c: &str| cell_id(("sh", si, j, c.to_string()));
                if ui.add_sized([W_PICK, ROW_H], egui::Button::new("◉").frame(false)).on_hover_text("Beobachtung im Netzbild markieren").clicked() {
                    self.editor.sel = Sel::Line(station.clone(), sh.target.clone());
                }
                let mut changed = false;
                if let Some(n) = text_cell(ui, id("ziel"), &sh.target, W_NAME, false) {
                    self.setups[si].shots[j].target = n;
                    changed = true;
                }
                if let Some(v) = num_cell(ui, id("hz"), sh.hz, NK_WINKEL, W_ANGLE) {
                    self.setups[si].shots[j].hz = v;
                    changed = true;
                }
                if let Some(v) = num_cell(ui, id("s"), sh.dist, NK_STRECKE, W_DIST) {
                    self.setups[si].shots[j].dist = v.filter(|d| *d > 0.0);
                    changed = true;
                }
                ui.add_sized([W_STAR, ROW_H], egui::Label::new(if sh.dist_horizontal { "*" } else { "" }));
                if sh.dist_horizontal {
                    if let Some(v) = num_cell(ui, id("v"), sh.dh, NK_HOEHE, W_V) {
                        self.setups[si].shots[j].dh = v;
                        changed = true;
                    }
                } else if let Some(v) = num_cell(ui, id("v"), sh.zenith, NK_WINKEL, W_V) {
                    self.setups[si].shots[j].zenith = v;
                    changed = true;
                }
                if let Some(Some(v)) = num_cell(ui, id("th"), Some(sh.th), NK_IH, W_TH) {
                    self.setups[si].shots[j].th = v;
                    changed = true;
                }
                if let Some(t) = text_cell(ui, id("code"), &sh.code, W_CODE, false) {
                    self.setups[si].shots[j].code = t;
                    self.data_dirty = true;
                }
                let dis = |k: ObsKind| self.proj.disabled.contains(&obs_key('D', sh.line, k));
                let has_v = sh.zenith.is_some() || sh.dh.is_some();
                let mut toggle = None;
                if flag(ui, sh.hz.is_some(), !dis(ObsKind::Dir), "R", "Richtung") {
                    toggle = Some(ObsKind::Dir);
                }
                if flag(ui, sh.dist.is_some(), !dis(ObsKind::HDist), "S", "Strecke") {
                    toggle = Some(ObsKind::HDist);
                }
                if flag(ui, has_v, !dis(ObsKind::Zenith), "H", "Höhe / Zenitwinkel") {
                    toggle = Some(ObsKind::Zenith);
                }
                let sig = |v: Option<f64>| v.map(|v| v * MILLI);
                if let Some(v) = num_cell(ui, id("shz"), sig(sh.sigma_hz), NK_SIGMA, W_SIGMA) {
                    self.setups[si].shots[j].sigma_hz = v.filter(|v| *v > 0.0).map(|v| v / MILLI);
                    changed = true;
                }
                if let Some(v) = num_cell(ui, id("ss"), sig(sh.sigma_dist), NK_SIGMA, W_SIGMA) {
                    self.setups[si].shots[j].sigma_dist = v.filter(|v| *v > 0.0).map(|v| v / MILLI);
                    changed = true;
                }
                if let Some(v) = num_cell(ui, id("sv"), sig(sh.sigma_v), NK_SIGMA, W_SIGMA) {
                    self.setups[si].shots[j].sigma_v = v.filter(|v| *v > 0.0).map(|v| v / MILLI);
                    changed = true;
                }
                if let Some(k) = toggle {
                    self.toggle_key(obs_key('D', sh.line, k));
                } else if changed {
                    self.after_edit();
                }
            }
            MRow::NivHead => {
                ui.add_sized([W_PICK, ROW_H], egui::Label::new(""));
                for (w, t, tip) in [(W_NAME, "Von", ""), (W_NAME, "Nach", ""), (W_ANGLE, "dH [m]", "Höhenunterschied"), (W_DIST, "Strecke [m]", "Nivellementsstrecke"), (W_FLAG, "H", "verwenden"), (W_SIGMA, "σdH", "eigene Standardabweichung [mm]; - = Voreinstellung (mm je km)")] {
                    head(ui, w, t, tip);
                }
                ui.label(RichText::new("Höhenunterschiede / Nivellement").small().color(MUTED));
            }
            MRow::Niv(k) => {
                let n = self.niv[k].clone();
                let id = |c: &str| cell_id(("niv", k, c.to_string()));
                if ui.add_sized([W_PICK, ROW_H], egui::Button::new("◉").frame(false)).on_hover_text("Beobachtung im Netzbild markieren").clicked() {
                    self.editor.sel = Sel::Line(n.from.clone(), n.to.clone());
                }
                let mut changed = false;
                if let Some(t) = text_cell(ui, id("von"), &n.from, W_NAME, false) {
                    self.niv[k].from = t;
                    changed = true;
                }
                if let Some(t) = text_cell(ui, id("nach"), &n.to, W_NAME, false) {
                    self.niv[k].to = t;
                    changed = true;
                }
                if let Some(Some(v)) = num_cell(ui, id("dh"), Some(n.dh), NK_STRECKE + 1, W_ANGLE) {
                    self.niv[k].dh = v;
                    changed = true;
                }
                if let Some(Some(v)) = num_cell(ui, id("s"), Some(n.s), 2, W_DIST) {
                    self.niv[k].s = v.max(1.0);
                    changed = true;
                }
                let key = obs_key('N', n.line, ObsKind::DhNiv);
                let on = !self.proj.disabled.contains(&key);
                let toggled = flag(ui, true, on, "H", "Höhenunterschied");
                if let Some(v) = num_cell(ui, id("sig"), n.sigma.map(|v| v * MILLI), NK_SIGMA, W_SIGMA) {
                    self.niv[k].sigma = v.filter(|v| *v > 0.0).map(|v| v / MILLI);
                    changed = true;
                }
                if toggled {
                    self.toggle_key(key);
                } else if changed {
                    self.after_edit();
                }
            }
        }
    }

    fn point_table(&mut self, ui: &mut Ui) {
        let pts = self.net_points();
        if pts.is_empty() {
            ui.label(RichText::new("Keine Punkte geladen.").color(MUTED));
            return;
        }
        let sel = self.editor.sel.clone();
        let hl = |p: &NetPoint| match &sel {
            Sel::Point(n) => *n == p.name,
            Sel::Line(a, b) => *a == p.name || *b == p.name,
            Sel::None => false,
        };
        let rows: Vec<(NetPoint, bool)> = pts.into_iter().map(|p| {
            let h = hl(&p);
            (p, h)
        }).filter(|r| !self.editor.only_sel || r.1).collect();
        let scroll = std::mem::take(&mut self.editor.scroll);
        let nk = self.proj.settings.nk.max(3);
        let mut change: Option<NetPoint> = None;
        let mut select: Option<String> = None;
        egui::ScrollArea::horizontal().id_salt("punkte_h").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                head(ui, W_PICK, "", "");
                head(ui, W_NAME, "Punkt", "");
                head(ui, W_COORD, "Rechtswert [m]", "Festpunkt: gegebene Koordinate · Neupunkt: Näherung (leer = wird berechnet)");
                head(ui, W_COORD, "Hochwert [m]", "");
                head(ui, W_H, "Höhe [m]", "");
                head(ui, W_ART * 4.0 + 12.0, "F  L  H  N", "F = Lage und Höhe fest, L = nur Lage fest, H = nur Höhe fest, N = Neupunkt");
                head(ui, W_CODE, "Code", "");
            });
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("punkte");
            if scroll {
                if let Some(i) = rows.iter().position(|r| r.1) {
                    area = area.vertical_scroll_offset(((i as f32 - SCROLL_VORLAUF).max(0.0)) * (ROW_H + ROW_GAP));
                }
            }
            area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for (p, h) in &rows[range] {
                    row_bg(ui, if *h { Some(SEL_ROW) } else { None }, |ui| {
                        let id = |c: &str| cell_id(("pt", p.name.clone(), c.to_string()));
                        if ui.add_sized([W_PICK, ROW_H], egui::Button::new("◉").frame(false)).on_hover_text("Punkt im Netzbild markieren").clicked() {
                            select = Some(p.name.clone());
                        }
                        let fixed = p.art != Art::N;
                        ui.add_sized([W_NAME, ROW_H], egui::Label::new(RichText::new(&p.name).monospace().strong().color(if fixed { INK } else { ACCENT })));
                        let mut q = p.clone();
                        let mut dirty = false;
                        if let Some(v) = num_cell(ui, id("y"), p.y, nk, W_COORD) {
                            q.y = v;
                            dirty = true;
                        }
                        if let Some(v) = num_cell(ui, id("x"), p.x, nk, W_COORD) {
                            q.x = v;
                            dirty = true;
                        }
                        if let Some(v) = num_cell(ui, id("h"), p.h, nk, W_H) {
                            q.h = v;
                            dirty = true;
                        }
                        for a in Art::ALL {
                            let on = p.art == a;
                            let b = egui::Button::new(RichText::new(a.letter()).strong().color(if on { Color32::WHITE } else { MUTED })).fill(if on { if a == Art::N { ACCENT } else { INK } } else { Color32::from_rgb(241, 245, 249) });
                            if ui.add_sized([W_ART, ROW_H], b).on_hover_text(a.label()).clicked() && !on {
                                q.art = a;
                                dirty = true;
                            }
                        }
                        if let Some(t) = text_cell(ui, id("code"), &p.code, W_CODE, false) {
                            q.code = t;
                            dirty = true;
                        }
                        if dirty {
                            change = Some(q);
                        }
                    });
                }
            });
        });
        if let Some(n) = select {
            self.editor.sel = Sel::Point(n);
        }
        if let Some(p) = change {
            self.set_net_point(p);
            self.after_edit();
        }
    }
}
