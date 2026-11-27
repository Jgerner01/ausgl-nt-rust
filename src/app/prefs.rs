//! Voreinstellungen: Firmenkopf, Protokollformat (A4 quer/hoch), Öffnen der Protokolle und
//! Standardwerte für neue Projekte. Bearbeitet wird eine Kopie; erst „Speichern“ übernimmt sie
//! und schreibt sie in die Einstellungsdatei (%APPDATA%\AUSGL-NT\einstellungen.json).

use super::*;
use crate::adjust::Settings;

/// Anzahl der Zeilen des Firmenkopfs
const KOPF_ZEILEN: usize = 4;
/// Breite der Eingabefelder [Punkte]
const FELD_BREITE: f32 = 460.0;
const ZAHL_BREITE: f32 = 80.0;
/// Platz für Titelzeile und Knöpfe des Fensters [Punkte]
const RAHMEN_HOEHE: f32 = 170.0;
const MIN_LISTE: f32 = 200.0;

#[derive(Clone, Copy, PartialEq)]
enum Aktion {
    Keine,
    Speichern,
    Abbrechen,
    Werkseinstellungen,
    AusProjekt,
    AufProjekt,
}

/// Zahlenfeld mit Einheit in einer Grid-Zeile
fn zahl(ui: &mut Ui, label: &str, v: &mut f64, unit: &str, speed: f64, range: std::ops::RangeInclusive<f64>, nk: usize) {
    ui.label(label);
    ui.add_sized([ZAHL_BREITE, 20.0], egui::DragValue::new(v).speed(speed).range(range).max_decimals(nk));
    ui.label(RichText::new(unit).small().color(MUTED));
    ui.end_row();
}

/// Genauigkeiten, Grenzwerte und Iteration übertragen (ohne Netzart, Variante, Lösungsverfahren)
fn kopiere_werte(dst: &mut Settings, src: &Settings) {
    dst.sigma_dir_mgon = src.sigma_dir_mgon;
    dst.sigma_dist_mm = src.sigma_dist_mm;
    dst.sigma_dist_ppm = src.sigma_dist_ppm;
    dst.sigma_zen_mgon = src.sigma_zen_mgon;
    dst.sigma_fix_mm = src.sigma_fix_mm;
    dst.sigma_fix_h_mm = src.sigma_fix_h_mm;
    dst.sigma_niv_mm_km = src.sigma_niv_mm_km;
    dst.sigma_trig_mm_km = src.sigma_trig_mm_km;
    dst.prescreen = src.prescreen;
    dst.limit_dir_gon = src.limit_dir_gon;
    dst.limit_dist_m = src.limit_dist_m;
    dst.limit_h_m = src.limit_h_m;
    dst.max_iter = src.max_iter;
    dst.k_crit = src.k_crit;
    dst.nk = src.nk;
}

impl AusglApp {
    /// Standardwerte für ein neues bzw. importiertes Projekt
    pub(super) fn default_settings(&self) -> Settings {
        let mut s = Settings::default();
        kopiere_werte(&mut s, &self.sys.defaults);
        s
    }

    pub(super) fn prefs_dialog(&mut self, ctx: &egui::Context) {
        if !self.dlg_params {
            return;
        }
        let mut e = self.pref_edit.take().unwrap_or_else(|| self.sys.clone());
        while e.firmenkopf.len() < KOPF_ZEILEN {
            e.firmenkopf.push(String::new());
        }
        let mut open = true;
        let mut aktion = Aktion::Keine;
        // Liste so hoch wie das Programmfenster erlaubt (Toughpad: 640 Punkte)
        let liste = (ctx.content_rect().height() - RAHMEN_HOEHE).max(MIN_LISTE);
        egui::Window::new("Voreinstellungen").open(&mut open).default_width(FELD_BREITE + 40.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(liste).show(ui, |ui| {
                ui.label(RichText::new("Firmenkopf (Protokollkopf)").strong());
                for l in e.firmenkopf.iter_mut().take(KOPF_ZEILEN) {
                    ui.add(egui::TextEdit::singleline(l).desired_width(FELD_BREITE));
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Protokoll (RTF)").strong());
                ui.horizontal(|ui| {
                    ui.label("Papierformat A4");
                    ui.selectable_value(&mut e.rtf_landscape, true, "Querformat");
                    ui.selectable_value(&mut e.rtf_landscape, false, "Hochformat");
                });
                ui.checkbox(&mut e.open_after, "Dateien nach dem Speichern öffnen");
                ui.checkbox(&mut e.rtf_word, "Protokolle mit Word bzw. LibreOffice öffnen")
                    .on_hover_text("WordPad (oft Standardprogramm für RTF) ignoriert Papierformat und Querformat und zeigt immer A4 hoch – breite Tabellen passen dann nicht auf die Seite.");
                ui.add_space(8.0);
                ui.label(RichText::new("Standardwerte für neue Projekte").strong());
                ui.label(RichText::new("Gelten für „Projekt neu anlegen“ und beim Import (WAUSGL, CAPLAN, Netzdatei). Bestehende Projekte behalten ihre Werte (Seitenleiste).").small().color(MUTED));
                let d = &mut e.defaults;
                egui::Grid::new("pref_defaults").num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
                    zahl(ui, "σ Richtung", &mut d.sigma_dir_mgon, "mgon", 0.01, 0.001..=100.0, 3);
                    zahl(ui, "σ Strecke konstant", &mut d.sigma_dist_mm, "mm", 0.1, 0.01..=1000.0, 2);
                    zahl(ui, "σ Strecke streckenabhängig", &mut d.sigma_dist_ppm, "mm/km", 0.1, 0.0..=1000.0, 2);
                    zahl(ui, "σ Zenitwinkel", &mut d.sigma_zen_mgon, "mgon", 0.01, 0.001..=100.0, 3);
                    zahl(ui, "σ Altpunkt Lage", &mut d.sigma_fix_mm, "mm", 0.1, 0.01..=1000.0, 2);
                    zahl(ui, "σ Altpunkt Höhe", &mut d.sigma_fix_h_mm, "mm", 0.1, 0.01..=1000.0, 2);
                    zahl(ui, "σ Nivellement", &mut d.sigma_niv_mm_km, "mm/√km", 0.1, 0.01..=1000.0, 2);
                    zahl(ui, "σ trig. Höhen", &mut d.sigma_trig_mm_km, "mm/km", 0.1, 0.01..=1000.0, 2);
                    ui.label("Grenzwertprüfung vor der Ausgleichung");
                    ui.checkbox(&mut d.prescreen, "");
                    ui.end_row();
                    zahl(ui, "Grenzwert Richtung", &mut d.limit_dir_gon, "gon", 0.01, 0.0001..=400.0, 4);
                    zahl(ui, "Grenzwert Strecke", &mut d.limit_dist_m, "m", 0.01, 0.0001..=1000.0, 4);
                    zahl(ui, "Grenzwert Höhe", &mut d.limit_h_m, "m", 0.01, 0.0001..=1000.0, 4);
                    zahl(ui, "Ausreißergrenze NV", &mut d.k_crit, "", 0.01, 1.0..=10.0, 2);
                    ui.label("max. Iterationen");
                    ui.add_sized([ZAHL_BREITE, 20.0], egui::DragValue::new(&mut d.max_iter).range(1..=100));
                    ui.end_row();
                    ui.label("Nachkommastellen Koordinaten");
                    ui.add_sized([ZAHL_BREITE, 20.0], egui::DragValue::new(&mut d.nk).range(0..=6));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.small_button("Werte des aktuellen Projekts übernehmen").clicked() {
                        aktion = Aktion::AusProjekt;
                    }
                    if ui.small_button("auf das aktuelle Projekt anwenden").clicked() {
                        aktion = Aktion::AufProjekt;
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(RichText::new("💾 Speichern").strong()).clicked() {
                    aktion = Aktion::Speichern;
                }
                if ui.button("Abbrechen").clicked() {
                    aktion = Aktion::Abbrechen;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Werkseinstellungen").on_hover_text("Alle Felder auf die Auslieferungswerte setzen – dauerhaft erst mit „Speichern“").clicked() {
                        aktion = Aktion::Werkseinstellungen;
                    }
                });
            });
            if e != self.sys {
                ui.label(RichText::new("Geändert – mit „Speichern“ übernehmen.").small().color(WARN));
            }
        });
        match aktion {
            Aktion::Speichern => {
                e.last_project = self.sys.last_project.clone();
                e.last_dir = self.sys.last_dir.clone();
                self.sys = e;
                self.save_sys();
                if let Some(out) = &self.outcome {
                    self.doc = Some(report::adjustment_doc(out, &self.proj.settings, &self.proj, &self.sys.firmenkopf));
                }
                self.status = "Voreinstellungen gespeichert.".into();
                self.dlg_params = false;
                return;
            }
            Aktion::Abbrechen => {
                self.dlg_params = false;
                return;
            }
            Aktion::Werkseinstellungen => {
                let keep = (e.last_project.clone(), e.last_dir.clone());
                e = SysParams::default();
                (e.last_project, e.last_dir) = keep;
            }
            Aktion::AusProjekt => kopiere_werte(&mut e.defaults, &self.proj.settings),
            Aktion::AufProjekt => {
                kopiere_werte(&mut self.proj.settings, &e.defaults);
                self.dirty_settings = true;
                self.status = "Standardwerte auf das aktuelle Projekt angewendet – Projekt speichern, um sie zu behalten.".into();
            }
            Aktion::Keine => {}
        }
        if open {
            self.pref_edit = Some(e);
        } else {
            // Fenster über ✕ geschlossen: ungespeicherte Änderungen verwerfen
            self.dlg_params = false;
        }
    }
}
