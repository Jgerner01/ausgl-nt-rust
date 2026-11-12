//! Oberfläche: Menüs wie WAUSGL (Datei, Lagenetz, Höhennetz, 3D-Netz, Vorwärtsschnitte, Tools),
//! Seitenleiste für Projekt und Berechnung, Bereiche Netzbild / Beobachtungen / Koordinaten /
//! Protokoll / Daten sowie Dialoge für PDF, DXF, Satzmittel und Transformation.

use crate::adjust::{self, Input, Outcome, Robust, Variant};
use crate::approx::PointMap;
use crate::export::{self, Paper, PlotOptions};
use crate::io::{self, KorPoint, NetKind, NivLine, ProjectFile, Setup};
use crate::linalg::Solver;
use crate::report::{self, Align, Block, Doc, Tone};
use crate::scene::{self, ellipse_pts, fmt_scale, Item, Marker, NetView, Style};
use crate::tools::{self, SetMeanStation, TransResult};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const BORDER: Color32 = Color32::from_rgb(226, 232, 240);
const MUTED: Color32 = Color32::from_rgb(100, 116, 139);
const BAD: Color32 = Color32::from_rgb(225, 29, 72);
const WARN: Color32 = Color32::from_rgb(217, 119, 6);
const GOOD: Color32 = Color32::from_rgb(22, 163, 74);
const INK: Color32 = Color32::from_rgb(15, 23, 42);
const PX_MM: f32 = 3.78;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Netz,
    Beob,
    Koord,
    Protokoll,
    Daten,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct SysParams {
    firmenkopf: Vec<String>,
    last_project: String,
    last_dir: String,
}

impl Default for SysParams {
    fn default() -> Self {
        SysParams { firmenkopf: vec!["Vermessungsbüro Muster".into(), "Musterstraße 1 · 12345 Musterstadt".into(), String::new(), String::new()], last_project: String::new(), last_dir: String::new() }
    }
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(|d| PathBuf::from(d).join("AUSGL-NT").join("einstellungen.json"))
}

struct Cam {
    cy: f64,
    cx: f64,
    ppm: f64,
    fitted: bool,
}

pub struct AusglApp {
    proj: ProjectFile,
    proj_path: Option<PathBuf>,
    sample_mode: bool,
    kor: Vec<KorPoint>,
    lagekor: Vec<KorPoint>,
    kon: Vec<KorPoint>,
    setups: Vec<Setup>,
    niv: Vec<NivLine>,
    raw: [String; 3],
    approx_pm: PointMap,
    outcome: Option<Outcome>,
    doc: Option<Doc>,
    error: Option<String>,
    view: NetView,
    style: Style,
    show_grid: bool,
    cam: Cam,
    tab: Tab,
    data_tab: usize,
    sys: SysParams,
    status: String,
    dirty_settings: bool,
    rerun: bool,
    only_outliers: bool,
    sort_nv: bool,
    dlg_project: bool,
    dlg_params: bool,
    dlg_pdf: bool,
    dlg_dxf: bool,
    dlg_satz: bool,
    dlg_trans: bool,
    dlg_help: bool,
    dlg_about: bool,
    pdf_opts: PlotOptions,
    pdf_auto: bool,
    open_after: bool,
    dxf_n: f64,
    dxf_grid: bool,
    satz_file: String,
    satz: Option<(Vec<SetMeanStation>, Vec<Setup>)>,
    trans_src: String,
    trans_dst: String,
    trans_scale: bool,
    trans: Option<Result<TransResult, String>>,
    screenshot: Option<PathBuf>,
    frame: u64,
    shot_requested: bool,
    shot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = Color32::from_rgb(244, 246, 250);
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(241, 245, 249);
    v.selection.bg_fill = Color32::from_rgb(191, 219, 254);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = egui::CornerRadius::same(10);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(6);
    }
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(238, 242, 247);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(224, 231, 255);
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
    });
}

fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(13.5).color(INK));
            ui.add_space(2.0);
            add(ui)
        })
        .inner
}

fn tone_color(t: Tone) -> Color32 {
    match t {
        Tone::Bad => BAD,
        Tone::Warn => WARN,
        Tone::Muted => MUTED,
        Tone::Normal => INK,
    }
}

fn pick(title: &str, filters: &[(&str, &[&str])], dir: &str) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title);
    for (n, e) in filters {
        d = d.add_filter(*n, e);
    }
    if !dir.is_empty() {
        d = d.set_directory(dir);
    }
    d.pick_file()
}

fn save_dialog(title: &str, name: &str, filters: &[(&str, &[&str])], dir: &str) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title).set_file_name(name);
    for (n, e) in filters {
        d = d.add_filter(*n, e);
    }
    if !dir.is_empty() {
        d = d.set_directory(dir);
    }
    d.save_file()
}

fn open_file(p: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(p).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    }
}

const KOR_F: &[(&str, &[&str])] = &[("Koordinaten", &["kor", "KOR", "kon", "KON", "txt"]), ("Alle Dateien", &["*"])];
const DAT_F: &[(&str, &[&str])] = &[("Meßdaten", &["dat", "DAT", "dam", "DAM", "txt"]), ("Alle Dateien", &["*"])];
const PRJ_F: &[(&str, &[&str])] = &[("AUSGL-NT-Projekt", &["ausgl"]), ("WAUSGL-Projekt", &["prj", "PRJ"])];

impl AusglApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        setup_style(&cc.egui_ctx);
        let sys: SysParams = config_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut app = AusglApp {
            proj: ProjectFile { name: "Neues Projekt".into(), ..Default::default() },
            proj_path: None,
            sample_mode: false,
            kor: vec![],
            lagekor: vec![],
            kon: vec![],
            setups: vec![],
            niv: vec![],
            raw: Default::default(),
            approx_pm: PointMap::new(),
            outcome: None,
            doc: None,
            error: None,
            view: NetView::default(),
            style: Style::default(),
            show_grid: true,
            cam: Cam { cy: 0.0, cx: 0.0, ppm: 1.0, fitted: false },
            tab: Tab::Netz,
            data_tab: 0,
            sys,
            status: "Bereit – Projekt öffnen oder Beispielprojekt laden.".into(),
            dirty_settings: false,
            rerun: false,
            only_outliers: false,
            sort_nv: false,
            dlg_project: false,
            dlg_params: false,
            dlg_pdf: false,
            dlg_dxf: false,
            dlg_satz: false,
            dlg_trans: false,
            dlg_help: false,
            dlg_about: false,
            pdf_opts: PlotOptions::default(),
            pdf_auto: true,
            open_after: true,
            dxf_n: 1000.0,
            dxf_grid: true,
            satz_file: String::new(),
            satz: None,
            trans_src: String::new(),
            trans_dst: String::new(),
            trans_scale: true,
            trans: None,
            screenshot: screenshot.clone(),
            frame: 0,
            shot_requested: false,
            shot: Arc::new(Mutex::new(None)),
        };
        if screenshot.is_some() {
            app.load_sample();
            app.proj.kind = NetKind::Raum3D;
            app.proj.settings.robust = Robust::Huber;
            app.proj.settings.solver = Solver::Cholesky;
            app.style.axes = true;
            app.run();
        } else if let Some(p) = open {
            app.open_any(&p);
        } else if !app.sys.last_project.is_empty() && Path::new(&app.sys.last_project).exists() {
            let p = PathBuf::from(&app.sys.last_project);
            app.open_project(&p);
        } else {
            app.load_sample();
        }
        app
    }

    fn save_sys(&self) {
        if self.screenshot.is_some() {
            return;
        }
        if let Some(p) = config_path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(p, serde_json::to_string_pretty(&self.sys).unwrap_or_default());
        }
    }

    fn base_dir(&self) -> PathBuf {
        self.proj_path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from(&self.sys.last_dir))
    }

    fn dir_str(&self) -> String {
        self.base_dir().to_string_lossy().to_string()
    }

    // ------------------------------------------------------------ Projekt und Daten

    fn load_sample(&mut self) {
        let s = crate::sample::Sample::generate(true);
        self.proj = ProjectFile {
            name: "Beispielnetz".into(),
            nummer: "2026-01".into(),
            beschreibung: "Erfundenes Übungsnetz: 4 Altpunkte, 5 Neupunkte, Tachymeter und Nivellement – ein grober Richtungsfehler ist eingebaut.".into(),
            ort: "Musterstadt".into(),
            bearbeiter: "AUSGL-NT".into(),
            kind: NetKind::LageHoehe,
            ..Default::default()
        };
        self.raw = [s.kor_text(), s.dat_text(), s.niv_text()];
        self.kor = s.kor;
        self.setups = s.setups;
        self.niv = s.niv;
        self.lagekor.clear();
        self.kon.clear();
        self.proj_path = None;
        self.sample_mode = true;
        self.after_load();
        self.status = "Beispielprojekt geladen (ein grober Fehler ist eingebaut – mit robuster Schätzung aufspüren).".into();
    }

    fn after_load(&mut self) {
        self.outcome = None;
        self.doc = None;
        self.error = None;
        self.refresh_preview();
        self.cam.fitted = false;
    }

    fn refresh_preview(&mut self) {
        let inp = Input { kind: self.proj.kind, kor: &self.kor, lagekor: &self.lagekor, kon: &self.kon, setups: &self.setups, niv: &self.niv, disabled: &self.proj.disabled };
        let mut log = Vec::new();
        let pm = adjust::build_points(&inp, &self.proj.settings, true, true, &mut log);
        self.view = NetView::from_data(&self.kor, &pm, &self.setups, &self.niv, self.proj.kind.uses_tachy());
        self.approx_pm = pm;
    }

    fn read_list(&self, f: &str) -> Result<Option<String>, String> {
        match self.proj.resolve(&self.base_dir(), f) {
            None => Ok(None),
            Some(p) => io::read_text(&p).map(Some).map_err(|e| format!("{}: {}", p.display(), e)),
        }
    }

    fn load_data(&mut self) {
        if self.sample_mode {
            self.after_load();
            return;
        }
        let mut errs = Vec::new();
        let kor = self.read_list(&self.proj.kor);
        let mes = self.read_list(&self.proj.mes);
        let niv = self.read_list(&self.proj.niv);
        let lk = self.read_list(&self.proj.lagekor);
        self.kor = match &kor {
            Ok(Some(t)) => io::parse_kor(t),
            Ok(None) => vec![],
            Err(e) => {
                errs.push(e.clone());
                vec![]
            }
        };
        self.setups = match &mes {
            Ok(Some(t)) if self.proj.kind != NetKind::Niv => io::parse_tachy(t),
            Err(e) => {
                errs.push(e.clone());
                vec![]
            }
            _ => vec![],
        };
        let niv_text = match (&niv, &mes, self.proj.kind) {
            (Ok(Some(t)), _, _) => Some(t.clone()),
            (_, Ok(Some(t)), NetKind::Niv) => Some(t.clone()),
            _ => None,
        };
        if let Err(e) = &niv {
            errs.push(e.clone());
        }
        self.niv = niv_text.as_deref().map(io::parse_niv).unwrap_or_default();
        self.lagekor = match &lk {
            Ok(Some(t)) => io::parse_kor(t),
            _ => vec![],
        };
        let konp = self.base_dir().join(format!("{}.kon", self.proj.name));
        self.kon = io::read_text(&konp).map(|t| io::parse_kor(&t)).unwrap_or_default();
        self.raw = [kor.ok().flatten().unwrap_or_default(), mes.ok().flatten().unwrap_or_default(), niv_text.unwrap_or_default()];
        self.after_load();
        self.status = if errs.is_empty() {
            format!("{} Altpunkte, {} Standpunkte, {} Nivellementslinien eingelesen.", self.kor.len(), self.setups.len(), self.niv.len())
        } else {
            format!("Fehler beim Einlesen: {}", errs.join("; "))
        };
    }

    fn open_any(&mut self, p: &Path) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "prj" {
            self.import_prj(p);
        } else {
            self.open_project(p);
        }
    }

    fn open_project(&mut self, p: &Path) {
        match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<ProjectFile>(&s).map_err(|e| e.to_string())) {
            Ok(pf) => {
                self.proj = pf;
                self.proj_path = Some(p.to_path_buf());
                self.sample_mode = false;
                self.sys.last_project = p.to_string_lossy().to_string();
                self.sys.last_dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                self.save_sys();
                self.load_data();
            }
            Err(e) => self.status = format!("Projekt konnte nicht geöffnet werden: {}", e),
        }
    }

    fn import_prj(&mut self, p: &Path) {
        match std::fs::read(p).ok().and_then(|b| io::import_legacy_prj(&b)) {
            Some(mut pf) => {
                if pf.name.is_empty() {
                    pf.name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                }
                self.proj = pf;
                self.proj_path = None;
                self.sample_mode = false;
                self.sys.last_dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                self.load_data();
                self.status = format!("WAUSGL-Projekt {} importiert – Pfade prüfen und als *.ausgl speichern.", p.display());
                self.dlg_project = true;
            }
            None => self.status = "Die *.PRJ-Datei konnte nicht gelesen werden.".into(),
        }
    }

    fn save_project(&mut self, ask: bool) {
        let path = if ask || self.proj_path.is_none() {
            save_dialog("Projekt speichern", &format!("{}.ausgl", self.proj.name), &[("AUSGL-NT-Projekt", &["ausgl"])], &self.dir_str())
        } else {
            self.proj_path.clone()
        };
        let Some(path) = path else { return };
        if self.sample_mode {
            let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "beispiel".into());
            let files = [format!("{}.kor", stem), format!("{}.dat", stem), format!("{}_niv.dat", stem)];
            for (i, f) in files.iter().enumerate() {
                let _ = std::fs::write(dir.join(f), &self.raw[i]);
            }
            self.proj.kor = dir.join(&files[0]).to_string_lossy().to_string();
            self.proj.mes = dir.join(&files[1]).to_string_lossy().to_string();
            self.proj.niv = dir.join(&files[2]).to_string_lossy().to_string();
            self.sample_mode = false;
        }
        self.proj.datum = crate::geo::today();
        match std::fs::write(&path, serde_json::to_string_pretty(&self.proj).unwrap_or_default()) {
            Ok(_) => {
                self.status = format!("Projekt gespeichert: {}", path.display());
                self.sys.last_project = path.to_string_lossy().to_string();
                self.sys.last_dir = path.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                self.proj_path = Some(path);
                self.save_sys();
            }
            Err(e) => self.status = format!("Speichern fehlgeschlagen: {}", e),
        }
    }

    // ------------------------------------------------------------ Rechnen

    fn run(&mut self) {
        self.dirty_settings = false;
        let inp = Input { kind: self.proj.kind, kor: &self.kor, lagekor: &self.lagekor, kon: &self.kon, setups: &self.setups, niv: &self.niv, disabled: &self.proj.disabled };
        let t0 = std::time::Instant::now();
        let res = adjust::run(&inp, &self.proj.settings);
        match res {
            Ok(out) => {
                let doc = report::adjustment_doc(&out, &self.proj.settings, &self.proj, &self.sys.firmenkopf);
                let p = &out.parts[0];
                self.status = format!(
                    "{} gerechnet ({}, {}): s0 = {:.3}, Redundanz {}, {} Ausreißer – {:.0} ms",
                    self.proj.kind.label(),
                    self.proj.settings.solver.short(),
                    self.proj.settings.variant.label(),
                    p.sigma0,
                    p.redundancy,
                    out.parts.iter().map(|p| p.outliers()).sum::<usize>(),
                    t0.elapsed().as_secs_f64() * 1000.0
                );
                self.view = NetView::from_outcome(&out);
                self.outcome = Some(out);
                self.doc = Some(doc);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e.clone());
                self.outcome = None;
                self.doc = None;
                self.status = format!("Ausgleichung nicht möglich: {}", e.lines().next().unwrap_or(""));
                self.refresh_preview();
            }
        }
    }

    fn run_with(&mut self, kind: NetKind, variant: Variant) {
        let reload = (kind == NetKind::Niv) != (self.proj.kind == NetKind::Niv);
        self.proj.kind = kind;
        self.proj.settings.variant = variant;
        if reload && !self.sample_mode {
            self.load_data();
        }
        self.run();
    }

    fn strike_outliers(&mut self) {
        let Some(out) = &self.outcome else { return };
        let mut n = 0;
        for part in &out.parts {
            for o in part.obs.iter().filter(|o| o.outlier) {
                let k = part.net.obs[o.obs].key.clone();
                if !self.proj.disabled.contains(&k) {
                    self.proj.disabled.push(k);
                    n += 1;
                }
            }
        }
        self.run();
        self.status = format!("{} Ausreißer gestrichen und neu gerechnet. {}", n, self.status);
    }

    fn save_kon(&mut self) {
        let Some(out) = &self.outcome else {
            self.status = "Erst rechnen, dann Neupunkte speichern.".into();
            return;
        };
        let pts: Vec<KorPoint> = out
            .merged_points()
            .iter()
            .filter(|p| (p.plan_adj && !p.fix_plan) || (p.h_adj && !p.fix_h))
            .map(|p| KorPoint { name: p.name.clone(), y: p.y, x: p.x, h: if p.h.is_finite() { Some(p.h) } else { None }, code: "009".into() })
            .collect();
        if let Some(path) = save_dialog("Neupunkte speichern", &format!("{}.kon", self.proj.name), &[("Koordinaten", &["kon", "kor", "txt"])], &self.dir_str()) {
            match std::fs::write(&path, io::write_kor(&pts, self.proj.settings.nk)) {
                Ok(_) => self.status = format!("{} Neupunkte gespeichert (Code 009): {}", pts.len(), path.display()),
                Err(e) => self.status = format!("Fehler: {}", e),
            }
        }
    }

    fn export_rtf(&mut self) {
        let Some(doc) = &self.doc else {
            self.status = "Noch kein Protokoll – zuerst ausgleichen.".into();
            return;
        };
        if let Some(path) = save_dialog("Protokoll als RTF speichern", &format!("{}_protokoll.rtf", self.proj.name), &[("Rich Text", &["rtf"])], &self.dir_str()) {
            match std::fs::write(&path, crate::rtf::to_rtf(doc, true)) {
                Ok(_) => {
                    self.status = format!("Protokoll gespeichert: {}", path.display());
                    if self.open_after {
                        open_file(&path);
                    }
                }
                Err(e) => self.status = format!("Fehler: {}", e),
            }
        }
    }

    fn plot_title(&self) -> String {
        match self.proj.kind {
            NetKind::Niv | NetKind::Trig => "Höhennetz mit Fehlersäulen".into(),
            NetKind::Vorwaerts => "Netzbild räumliche Vorwärtsschnitte".into(),
            NetKind::Raum3D => "Netzbild 3D-Ausgleichung mit Fehlerellipsen".into(),
            _ => "Netzbild mit Fehlerellipsen".into(),
        }
    }

    fn plot_info(&self) -> Vec<String> {
        let mut v = vec![self.proj.kind.label().to_string()];
        if let Some(out) = &self.outcome {
            let s = &self.proj.settings;
            v.push(format!("{} · {}", s.variant.label(), s.solver.short()));
            if s.robust != Robust::Keine {
                v.push(format!("robust: {}", s.robust.label()));
            }
            for p in &out.parts {
                v.push(format!("{}: s0 = {:.3}, r = {}, Ausreißer {}", p.title, p.sigma0, p.redundancy, p.outliers()));
            }
        } else {
            v.push("vor der Ausgleichung".into());
        }
        if !self.proj.ort.is_empty() {
            v.push(format!("Ort: {}", self.proj.ort));
        }
        v
    }

    fn export_pdf(&mut self) {
        let mut o = self.pdf_opts.clone();
        o.project = self.proj.name.clone();
        o.editor = self.proj.bearbeiter.clone();
        o.info = self.plot_info();
        if self.pdf_auto {
            o.scale_n = 0.0;
        }
        if let Some(path) = save_dialog("Netzbild als PDF speichern", &format!("{}_netzbild.pdf", self.proj.name), &[("PDF", &["pdf"])], &self.dir_str()) {
            let (bytes, n) = export::render_pdf(&self.view, &self.style, &o);
            match std::fs::write(&path, bytes) {
                Ok(_) => {
                    self.status = format!("PDF gespeichert (Maßstab 1:{}): {}", scene::trim(n), path.display());
                    if self.open_after {
                        open_file(&path);
                    }
                }
                Err(e) => self.status = format!("Fehler: {}", e),
            }
        }
    }

    fn export_dxf(&mut self) {
        if let Some(path) = save_dialog("Netzbild als DXF speichern", &format!("{}_netzbild.dxf", self.proj.name), &[("DXF", &["dxf"])], &self.dir_str()) {
            let bytes = export::to_dxf(&self.view, &self.style, self.dxf_n, self.dxf_grid);
            match std::fs::write(&path, bytes) {
                Ok(_) => self.status = format!("DXF gespeichert (Zeichengrößen für 1:{}): {}", scene::trim(self.dxf_n), path.display()),
                Err(e) => self.status = format!("Fehler: {}", e),
            }
        }
    }

    fn new_project(&mut self) {
        self.proj = ProjectFile { name: "Neues Projekt".into(), ..Default::default() };
        self.proj_path = None;
        self.sample_mode = false;
        self.kor.clear();
        self.setups.clear();
        self.niv.clear();
        self.lagekor.clear();
        self.kon.clear();
        self.raw = Default::default();
        self.after_load();
        self.dlg_project = true;
    }

    // ------------------------------------------------------------ Menü

    fn menu(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Datei", |ui| {
                if ui.button("Projekt neu anlegen…").clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button("Projekt öffnen…").clicked() {
                    ui.close();
                    if let Some(p) = pick("Projekt öffnen", PRJ_F, &self.dir_str()) {
                        self.open_any(&p);
                    }
                }
                if ui.button("WAUSGL-Projekt (*.PRJ) importieren…").clicked() {
                    ui.close();
                    if let Some(p) = pick("WAUSGL-Projekt importieren", &[("WAUSGL-Projekt", &["prj", "PRJ"])], &self.dir_str()) {
                        self.import_prj(&p);
                    }
                }
                if ui.button("Projekt speichern").clicked() {
                    ui.close();
                    self.save_project(false);
                }
                if ui.button("Projekt speichern unter…").clicked() {
                    ui.close();
                    self.save_project(true);
                }
                ui.separator();
                if ui.button("Projektbeschrieb…").clicked() {
                    self.dlg_project = true;
                    ui.close();
                }
                if ui.button("Daten neu einlesen").clicked() {
                    self.load_data();
                    ui.close();
                }
                if ui.button("Beispielprojekt laden").clicked() {
                    self.load_sample();
                    ui.close();
                }
                ui.separator();
                if ui.button("Voreinstellungen…").clicked() {
                    self.dlg_params = true;
                    ui.close();
                }
                ui.separator();
                if ui.button("Ende").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Lagenetz", |ui| {
                ui.label(RichText::new("Berechnung").small().color(MUTED));
                for v in Variant::ALL {
                    if ui.button(v.label()).clicked() {
                        self.run_with(NetKind::Lage, v);
                        self.tab = Tab::Netz;
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Beobachtungen streichen…").clicked() {
                    self.tab = Tab::Beob;
                    ui.close();
                }
                if ui.button("Neupunkte speichern (*.KON)…").clicked() {
                    ui.close();
                    self.save_kon();
                }
                if ui.button("Fehlerellipsen").clicked() {
                    self.style.ellipses = true;
                    self.tab = Tab::Netz;
                    ui.close();
                }
            });
            ui.menu_button("Höhennetz", |ui| {
                for (kind, name) in [(NetKind::Niv, "Nivellement"), (NetKind::Trig, "trig. Höhennetz")] {
                    ui.label(RichText::new(name).small().color(MUTED));
                    for v in [Variant::Normal, Variant::AltpunkteBeob, Variant::FreiesNetz] {
                        if ui.button(format!("{} – {}", name, v.label())).clicked() {
                            self.run_with(kind, v);
                            self.tab = Tab::Netz;
                            ui.close();
                        }
                    }
                }
                ui.separator();
                if ui.button("Neupunkte speichern (*.KON)…").clicked() {
                    ui.close();
                    self.save_kon();
                }
                if ui.button("Fehlergrafik").clicked() {
                    self.style.bars = true;
                    self.tab = Tab::Netz;
                    ui.close();
                }
            });
            ui.menu_button("3D-Netz", |ui| {
                ui.label(RichText::new("Volle räumliche Ausgleichung").small().color(MUTED));
                for v in Variant::ALL {
                    if ui.button(format!("3D – {}", v.label())).clicked() {
                        self.run_with(NetKind::Raum3D, v);
                        self.tab = Tab::Netz;
                        ui.close();
                    }
                }
                ui.separator();
                ui.label(RichText::new("2D-Lage kombiniert mit Höhenausgleichung").small().color(MUTED));
                for v in Variant::ALL {
                    if ui.button(format!("2D+H – {}", v.label())).clicked() {
                        self.run_with(NetKind::LageHoehe, v);
                        self.tab = Tab::Netz;
                        ui.close();
                    }
                }
            });
            ui.menu_button("Vorwärtsschnitte", |ui| {
                if ui.button("Alle Punkte berechnen").clicked() {
                    self.run_with(NetKind::Vorwaerts, Variant::Normal);
                    self.tab = Tab::Koord;
                    ui.close();
                }
                if ui.button("Neupunkte speichern (*.KON)…").clicked() {
                    ui.close();
                    self.save_kon();
                }
                if ui.button("Netzbild").clicked() {
                    self.tab = Tab::Netz;
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Satzmittelbildung…").clicked() {
                    self.dlg_satz = true;
                    if self.satz_file.is_empty() {
                        self.satz_file = self.proj.mes.clone();
                    }
                    ui.close();
                }
                if ui.button("Räumliche Transformation…").clicked() {
                    self.dlg_trans = true;
                    ui.close();
                }
            });
            ui.menu_button("Ausgabe", |ui| {
                if ui.button("Protokoll als RTF…").clicked() {
                    ui.close();
                    self.export_rtf();
                }
                if ui.button("Netzbild als PDF…").clicked() {
                    self.dlg_pdf = true;
                    ui.close();
                }
                if ui.button("Netzbild als DXF…").clicked() {
                    self.dxf_n = export::auto_scale(&self.view, &self.pdf_opts);
                    self.dlg_dxf = true;
                    ui.close();
                }
                if ui.button("Neupunkte als *.KON…").clicked() {
                    ui.close();
                    self.save_kon();
                }
                ui.separator();
                ui.checkbox(&mut self.open_after, "Datei nach dem Speichern öffnen");
            });
            ui.menu_button("Hilfe", |ui| {
                if ui.button("Arbeitsweise und Dateiformate").clicked() {
                    self.dlg_help = true;
                    ui.close();
                }
                if ui.button("Über AUSGL-NT").clicked() {
                    self.dlg_about = true;
                    ui.close();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            if ui.button("📂 Öffnen").on_hover_text("Projekt öffnen").clicked() {
                if let Some(p) = pick("Projekt öffnen", PRJ_F, &self.dir_str()) {
                    self.open_any(&p);
                }
            }
            if ui.button("💾 Speichern").clicked() {
                self.save_project(false);
            }
            if ui.button("📝 Projekt").on_hover_text("Projektbeschrieb und Dateien").clicked() {
                self.dlg_project = true;
            }
            ui.separator();
            let run = egui::Button::new(RichText::new("▶  Ausgleichen").strong().color(Color32::WHITE)).fill(ACCENT);
            if ui.add(run).on_hover_text("Ausgleichung mit den Einstellungen links rechnen (F5)").clicked() {
                self.rerun = true;
            }
            ui.separator();
            if ui.button("📄 Protokoll RTF").clicked() {
                self.export_rtf();
            }
            if ui.button("🖨 PDF").clicked() {
                self.dlg_pdf = true;
            }
            if ui.button("📐 DXF").clicked() {
                self.dxf_n = export::auto_scale(&self.view, &self.pdf_opts);
                self.dlg_dxf = true;
            }
            if ui.button("📍 Neupunkte").on_hover_text("Neupunkte als *.KON speichern").clicked() {
                self.save_kon();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{}  ·  {}", self.proj.name, self.proj.kind.label())).color(MUTED));
            });
        });
    }

    // ------------------------------------------------------------ Seitenleiste

    fn side(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            let mut open_dlg = false;
            let mut reload = false;
            let np = self.view.pts.iter().filter(|p| !p.fixed).count();
            let nobs: usize = self.setups.iter().map(|s| s.shots.len()).sum();
            card(ui, "Projekt", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.proj.name).size(15.0).strong());
                    if self.sample_mode {
                        ui.label(RichText::new("Beispiel").small().color(ACCENT));
                    }
                });
                if !self.proj.beschreibung.is_empty() {
                    ui.label(RichText::new(&self.proj.beschreibung).small().color(MUTED));
                }
                ui.add_space(4.0);
                egui::Grid::new("counts").num_columns(4).spacing([10.0, 2.0]).show(ui, |ui| {
                    for (i, (k, v)) in [("Altpunkte", self.kor.len()), ("Neupunkte", np), ("Standpunkte", self.setups.len()), ("Zielungen", nobs), ("Niv.-Linien", self.niv.len())].iter().enumerate() {
                        ui.label(RichText::new(*k).small().color(MUTED));
                        ui.label(RichText::new(v.to_string()).strong());
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.small_button("Dateien…").clicked() {
                        open_dlg = true;
                    }
                    if ui.small_button("neu einlesen").clicked() {
                        reload = true;
                    }
                });
            });
            if open_dlg {
                self.dlg_project = true;
            }
            if reload {
                self.load_data();
            }
            ui.add_space(8.0);
            let mut changed = false;
            let mut go = false;
            let dirty = self.dirty_settings && self.outcome.is_some();
            let before = self.proj.kind;
            card(ui, "Berechnung", |ui| {
                ui.label(RichText::new("Berechnungsart").small().color(MUTED));
                egui::ComboBox::from_id_salt("kind").width(ui.available_width()).selected_text(self.proj.kind.label()).show_ui(ui, |ui| {
                    for k in NetKind::ALL {
                        ui.selectable_value(&mut self.proj.kind, k, k.label());
                    }
                });
                let s = &mut self.proj.settings;
                ui.label(RichText::new("Variante").small().color(MUTED));
                egui::ComboBox::from_id_salt("variant").width(ui.available_width()).selected_text(s.variant.label()).show_ui(ui, |ui| {
                    for v in Variant::ALL {
                        changed |= ui.selectable_value(&mut s.variant, v, v.label()).changed();
                    }
                });
                ui.label(RichText::new("Lösungsverfahren").small().color(MUTED));
                egui::ComboBox::from_id_salt("solver").width(ui.available_width()).selected_text(s.solver.label()).show_ui(ui, |ui| {
                    for v in Solver::ALL {
                        changed |= ui.selectable_value(&mut s.solver, v, v.label()).changed();
                    }
                });
                ui.label(RichText::new("Robuste Schätzung (Ausreißersuche)").small().color(MUTED));
                egui::ComboBox::from_id_salt("robust").width(ui.available_width()).selected_text(s.robust.label()).show_ui(ui, |ui| {
                    for v in Robust::ALL {
                        changed |= ui.selectable_value(&mut s.robust, v, v.label()).changed();
                    }
                });
                if s.robust != Robust::Keine && s.robust != Robust::L1 {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Abstimmkonstante k").small());
                        let mut k = s.k();
                        if ui.add(egui::DragValue::new(&mut k).speed(0.05).range(0.5..=10.0)).changed() {
                            s.robust_k = k;
                            changed = true;
                        }
                        if ui.small_button("Standard").clicked() {
                            s.robust_k = 0.0;
                            changed = true;
                        }
                    });
                }
                ui.add_space(6.0);
                let btn = egui::Button::new(RichText::new("▶  Ausgleichung rechnen").strong().size(14.5).color(Color32::WHITE)).fill(ACCENT).min_size(Vec2::new(ui.available_width(), 32.0));
                if ui.add(btn).clicked() {
                    go = true;
                }
                if dirty {
                    ui.label(RichText::new("Einstellungen geändert – neu rechnen").small().color(WARN));
                }
            });
            if before != self.proj.kind {
                changed = true;
                if (before == NetKind::Niv) != (self.proj.kind == NetKind::Niv) && !self.sample_mode {
                    self.load_data();
                } else {
                    self.refresh_preview();
                }
            }
            if go {
                self.rerun = true;
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new(RichText::new("Genauigkeiten (a priori)").strong()).default_open(false).show(ui, |ui| {
                let s = &mut self.proj.settings;
                egui::Grid::new("apriori").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    let mut row = |ui: &mut Ui, l: &str, v: &mut f64, suf: &str, sp: f64| {
                        ui.label(l);
                        changed |= ui.add(egui::DragValue::new(v).speed(sp).range(0.0..=10000.0).suffix(suf)).changed();
                        ui.end_row();
                    };
                    row(ui, "Richtung", &mut s.sigma_dir_mgon, " mgon", 0.05);
                    row(ui, "Strecke konst.", &mut s.sigma_dist_mm, " mm", 0.1);
                    row(ui, "Strecke prop.", &mut s.sigma_dist_ppm, " mm/km", 0.1);
                    row(ui, "Zenitwinkel", &mut s.sigma_zen_mgon, " mgon", 0.05);
                    row(ui, "trig. Höhe", &mut s.sigma_trig_mm_km, " mm/km", 0.5);
                    row(ui, "Nivellement", &mut s.sigma_niv_mm_km, " mm/√km", 0.1);
                    row(ui, "Altpunkt Lage", &mut s.sigma_fix_mm, " mm", 0.5);
                    row(ui, "Altpunkt Höhe", &mut s.sigma_fix_h_mm, " mm", 0.5);
                    row(ui, "Richtungsgew. ab", &mut s.dir_weight_dist, " m", 10.0);
                });
            });
            egui::CollapsingHeader::new(RichText::new("Optionen").strong()).default_open(false).show(ui, |ui| {
                let s = &mut self.proj.settings;
                changed |= ui.checkbox(&mut s.prescreen, "Grenzwertprüfung vor der Ausgleichung").changed();
                if s.prescreen {
                    egui::Grid::new("limits").num_columns(2).show(ui, |ui| {
                        ui.label("Richtung/Zenit");
                        changed |= ui.add(egui::DragValue::new(&mut s.limit_dir_gon).speed(0.01).range(0.001..=10.0).suffix(" gon")).changed();
                        ui.end_row();
                        ui.label("Strecke");
                        changed |= ui.add(egui::DragValue::new(&mut s.limit_dist_m).speed(0.01).range(0.001..=100.0).suffix(" m")).changed();
                        ui.end_row();
                        ui.label("Höhe");
                        changed |= ui.add(egui::DragValue::new(&mut s.limit_h_m).speed(0.01).range(0.001..=100.0).suffix(" m")).changed();
                        ui.end_row();
                    });
                }
                changed |= ui.checkbox(&mut s.reduce, "Strecken wegen Höhe und GK reduzieren").changed();
                if s.reduce {
                    ui.horizontal(|ui| {
                        ui.label("Hm");
                        changed |= ui.add(egui::DragValue::new(&mut s.hm).speed(1.0).suffix(" m")).changed();
                        ui.label("Ym");
                        changed |= ui.add(egui::DragValue::new(&mut s.ym).speed(100.0).suffix(" m")).changed();
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("kritische NV");
                    changed |= ui.add(egui::DragValue::new(&mut s.k_crit).speed(0.05).range(1.0..=10.0)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Nachkommastellen");
                    changed |= ui.add(egui::DragValue::new(&mut s.nk).range(0..=6)).changed();
                });
                changed |= ui.checkbox(&mut s.ellipse_aposteriori, "Ellipsen mit s0 a posteriori").changed();
                changed |= ui.checkbox(&mut s.use_kon, "Näherungen aus *.KON übernehmen").changed();
                ui.horizontal(|ui| {
                    ui.label("max. Iterationen");
                    changed |= ui.add(egui::DragValue::new(&mut s.max_iter).range(1..=50)).changed();
                });
            });
            if changed {
                self.dirty_settings = true;
            }
            ui.add_space(8.0);
            self.result_card(ui);
        });
    }

    fn result_card(&mut self, ui: &mut Ui) {
        if let Some(e) = &self.error {
            let e = e.clone();
            card(ui, "Ergebnis", |ui| {
                ui.label(RichText::new(e).color(BAD));
            });
            return;
        }
        let Some(out) = &self.outcome else { return };
        let mut strike = false;
        let mut to_beob = false;
        let mut reactivate = false;
        let ndis = self.proj.disabled.len();
        card(ui, "Ergebnis", |ui| {
            for p in &out.parts {
                ui.label(RichText::new(&p.title).strong().color(ACCENT));
                egui::Grid::new(format!("res{}", p.title)).num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                    ui.label(RichText::new("s0 a posteriori").color(MUTED));
                    ui.label(RichText::new(format!("{:.3}", p.sigma0)).strong());
                    ui.end_row();
                    ui.label(RichText::new("Beob. / Unbek. / Red.").color(MUTED));
                    ui.label(format!("{} / {} / {}", p.n_obs, p.n_unknown, p.redundancy));
                    ui.end_row();
                    ui.label(RichText::new("Globaltest").color(MUTED));
                    match p.chi2_ok {
                        Some(true) => ui.label(RichText::new("✔ bestanden").color(GOOD)),
                        Some(false) => ui.label(RichText::new("✖ nicht bestanden").color(BAD)),
                        None => ui.label("–"),
                    };
                    ui.end_row();
                    if let Some((m, sm)) = p.scale_ppm {
                        ui.label(RichText::new("Maßstab").color(MUTED));
                        ui.label(format!("{:.1} ± {:.1} ppm", m, sm));
                        ui.end_row();
                    }
                    ui.label(RichText::new("Ausreißer").color(MUTED));
                    let n = p.outliers();
                    ui.label(RichText::new(n.to_string()).strong().color(if n > 0 { BAD } else { GOOD }));
                    ui.end_row();
                    if !p.struck.is_empty() {
                        ui.label(RichText::new("vorab gestrichen").color(MUTED));
                        ui.label(RichText::new(p.struck.len().to_string()).color(WARN));
                        ui.end_row();
                    }
                    if !p.converged {
                        ui.label(RichText::new("Konvergenz").color(MUTED));
                        ui.label(RichText::new("nicht erreicht").color(WARN));
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
            }
            let n: usize = out.parts.iter().map(|p| p.outliers()).sum();
            if n > 0 {
                if ui.button(RichText::new("Ausreißer streichen & neu rechnen").color(BAD)).clicked() {
                    strike = true;
                }
                if ui.small_button("Ausreißer ansehen").clicked() {
                    to_beob = true;
                }
            }
            if ndis > 0 {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} Beobachtungen gestrichen", ndis)).small().color(MUTED));
                    if ui.small_button("alle wieder aktivieren").clicked() {
                        reactivate = true;
                    }
                });
            }
        });
        if strike {
            self.strike_outliers();
        }
        if to_beob {
            self.tab = Tab::Beob;
            self.only_outliers = true;
        }
        if reactivate {
            self.proj.disabled.clear();
            self.rerun = true;
        }
    }

    // ------------------------------------------------------------ Netzbild

    fn fit(&mut self, rect: Rect) {
        if let Some(bb) = self.view.bbox() {
            self.cam.cy = (bb[0] + bb[2]) / 2.0;
            self.cam.cx = (bb[1] + bb[3]) / 2.0;
            let w = (bb[2] - bb[0]).max(1e-3);
            let h = (bb[3] - bb[1]).max(1e-3);
            self.cam.ppm = ((rect.width() as f64 - 80.0) / w).min((rect.height() as f64 - 80.0) / h).max(1e-6);
            self.cam.fitted = true;
        }
    }

    fn net_view(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let st = &mut self.style;
            ui.toggle_value(&mut st.numbers, "Nummern");
            ui.toggle_value(&mut st.heights, "Höhen");
            ui.toggle_value(&mut st.lines, "Beobachtungen");
            ui.toggle_value(&mut st.ellipses, "Fehlerellipsen");
            ui.toggle_value(&mut st.axes, "Achsen");
            ui.toggle_value(&mut st.bars, "Fehlersäulen");
            ui.toggle_value(&mut st.vectors, "Verschiebungen");
            ui.toggle_value(&mut st.outliers, "Ausreißer");
            ui.toggle_value(&mut self.show_grid, "Gitter");
            ui.separator();
            ui.label("Fehlermaßstab");
            let e = st.effective_e(&self.view);
            egui::ComboBox::from_id_salt("ell").width(96.0).selected_text(if st.ell_e > 0.0 { fmt_scale(st.ell_e) } else { format!("auto {}", fmt_scale(e)) }).show_ui(ui, |ui| {
                ui.selectable_value(&mut st.ell_e, 0.0, "automatisch");
                for v in [0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0] {
                    ui.selectable_value(&mut st.ell_e, v, fmt_scale(v));
                }
            });
            egui::ComboBox::from_id_salt("conf").width(80.0).selected_text(if st.conf > 1.0 { "95 %" } else { "1 σ" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut st.conf, 1.0, "1 σ (Standard)");
                ui.selectable_value(&mut st.conf, 2.4477, "95 % Konfidenz");
            });
            ui.separator();
            if ui.button("Einpassen").clicked() {
                self.cam.fitted = false;
            }
        });
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 8, Color32::WHITE);
        painter.rect_stroke(rect, 8, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
        if self.view.pts.is_empty() {
            painter.text(rect.center() - Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, "Noch keine Daten", FontId::proportional(20.0), MUTED);
            painter.text(rect.center() + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, "Datei ▸ Projekt öffnen … oder Datei ▸ Beispielprojekt laden", FontId::proportional(14.0), MUTED);
            return;
        }
        if !self.cam.fitted {
            self.fit(rect);
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.cam.cy -= d.x as f64 / self.cam.ppm;
            self.cam.cx += d.y as f64 / self.cam.ppm;
        }
        if let Some(hp) = resp.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = (scroll as f64 / 300.0).exp() * zoom as f64;
            if (f - 1.0).abs() > 1e-6 {
                let wy = self.cam.cy + (hp.x - rect.center().x) as f64 / self.cam.ppm;
                let wx = self.cam.cx - (hp.y - rect.center().y) as f64 / self.cam.ppm;
                self.cam.ppm = (self.cam.ppm * f).clamp(1e-6, 1e6);
                self.cam.cy = wy - (hp.x - rect.center().x) as f64 / self.cam.ppm;
                self.cam.cx = wx + (hp.y - rect.center().y) as f64 / self.cam.ppm;
            }
        }
        if resp.double_clicked() {
            self.cam.fitted = false;
        }
        let (cy, cx, ppm, c) = (self.cam.cy, self.cam.cx, self.cam.ppm, rect.center());
        let to = move |p: [f64; 2]| Pos2::new(c.x + ((p[0] - cy) * ppm) as f32, c.y - ((p[1] - cx) * ppm) as f32);
        let painter = painter.with_clip_rect(rect.shrink(1.0));
        if self.show_grid && !self.view.schematic {
            let step = scene::nice_step(90.0 / ppm);
            let wb = [cy - rect.width() as f64 / 2.0 / ppm, cx - rect.height() as f64 / 2.0 / ppm, cy + rect.width() as f64 / 2.0 / ppm, cx + rect.height() as f64 / 2.0 / ppm];
            let gs = Stroke::new(1.0, Color32::from_rgb(237, 241, 246));
            let gt = Color32::from_rgb(148, 163, 184);
            let mut y = (wb[0] / step).ceil() * step;
            while y <= wb[2] {
                let a = to([y, wb[1]]);
                painter.line_segment([Pos2::new(a.x, rect.top()), Pos2::new(a.x, rect.bottom())], gs);
                painter.text(Pos2::new(a.x + 3.0, rect.top() + 4.0), Align2::LEFT_TOP, scene::trim(y), FontId::proportional(10.0), gt);
                y += step;
            }
            let mut x = (wb[1] / step).ceil() * step;
            while x <= wb[3] {
                let a = to([wb[0], x]);
                painter.line_segment([Pos2::new(rect.left(), a.y), Pos2::new(rect.right(), a.y)], gs);
                painter.text(Pos2::new(rect.left() + 4.0, a.y - 2.0), Align2::LEFT_BOTTOM, scene::trim(x), FontId::proportional(10.0), gt);
                x += step;
            }
        }
        let items = scene::build(&self.view, &self.style);
        let col = |c: scene::Rgb| Color32::from_rgb(c[0], c[1], c[2]);
        for it in &items {
            match it {
                Item::Line { rgb, w_mm, dash, pts, .. } => {
                    let v: Vec<Pos2> = pts.iter().map(|p| to(*p)).collect();
                    let s = Stroke::new((*w_mm as f32 * PX_MM).max(1.0), col(*rgb));
                    if *dash {
                        painter.extend(egui::Shape::dashed_line(&v, s, 6.0, 4.0));
                    } else {
                        painter.line(v, s);
                    }
                }
                Item::Ellipse { rgb, center, a_mm, b_mm, theta_gon, axes, .. } => {
                    let c0 = to(*center);
                    let v: Vec<Pos2> = ellipse_pts(*a_mm, *b_mm, *theta_gon, 64).iter().take(64).map(|p| Pos2::new(c0.x + p[0] as f32 * PX_MM, c0.y - p[1] as f32 * PX_MM)).collect();
                    painter.add(egui::Shape::convex_polygon(v, Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 30), Stroke::new(1.6, col(*rgb))));
                    if *axes {
                        let t = theta_gon / crate::geo::RHO;
                        let (s, co) = (t.sin() as f32, t.cos() as f32);
                        let (a, b) = (*a_mm as f32 * PX_MM, *b_mm as f32 * PX_MM);
                        let ax = Stroke::new(1.0, col(scene::C_AXES));
                        painter.line_segment([c0 + Vec2::new(-a * s, a * co), c0 + Vec2::new(a * s, -a * co)], ax);
                        painter.line_segment([c0 + Vec2::new(-b * co, -b * s), c0 + Vec2::new(b * co, b * s)], ax);
                    }
                }
                Item::Arrow { rgb, anchor, de_mm, dn_mm, .. } => {
                    let a = to(*anchor);
                    painter.arrow(a, Vec2::new(*de_mm as f32 * PX_MM, -*dn_mm as f32 * PX_MM), Stroke::new(1.6, col(*rgb)));
                }
                Item::Bar { rgb, anchor, len_mm, width_mm, .. } => {
                    let a = to(*anchor);
                    let r = Rect::from_min_max(Pos2::new(a.x + 9.0, a.y - *len_mm as f32 * PX_MM), Pos2::new(a.x + 9.0 + *width_mm as f32 * PX_MM * 1.6, a.y));
                    painter.rect_filled(r, 1, col(*rgb));
                }
                Item::Tick { rgb, center, dir, half_mm, .. } => {
                    let c0 = to(*center);
                    let n = Vec2::new(-dir[1] as f32, -dir[0] as f32) * (*half_mm as f32 * PX_MM * 1.4);
                    painter.line_segment([c0 - n, c0 + n], Stroke::new(1.3, col(*rgb)));
                }
                Item::Point { rgb, pos, marker, size_mm, filled, .. } => {
                    let p = to(*pos);
                    let r = (*size_mm as f32 * PX_MM / 2.0).max(3.5) * 1.15;
                    let fill = if *filled { col(*rgb) } else { Color32::WHITE };
                    match marker {
                        Marker::Circle => {
                            painter.circle_filled(p, r, fill);
                            painter.circle_stroke(p, r, Stroke::new(1.5, Color32::WHITE));
                        }
                        Marker::Triangle => {
                            painter.add(egui::Shape::convex_polygon(vec![p + Vec2::new(0.0, -r * 1.25), p + Vec2::new(r * 1.1, r * 0.7), p + Vec2::new(-r * 1.1, r * 0.7)], fill, Stroke::new(1.2, Color32::WHITE)));
                        }
                        Marker::Square => {
                            painter.rect(Rect::from_center_size(p, Vec2::splat(2.0 * r)), 1, fill, Stroke::new(1.5, col(*rgb)), egui::StrokeKind::Middle);
                        }
                    }
                }
                Item::Text { rgb, pos, h_mm, ax, ay, dx_mm, dy_mm, text, .. } => {
                    let p = to(*pos) + Vec2::new(*dx_mm as f32 * PX_MM * 1.3, -*dy_mm as f32 * PX_MM * 1.3);
                    let hx = if *ax > 0.75 { egui::Align::Max } else if *ax > 0.25 { egui::Align::Center } else { egui::Align::Min };
                    let vy = if *ay > 0.75 { egui::Align::Min } else if *ay > 0.25 { egui::Align::Center } else { egui::Align::Max };
                    let al = Align2([hx, vy]);
                    let size = (*h_mm as f32 / 0.718 * PX_MM * 1.1).max(10.0);
                    for o in [Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0)] {
                        painter.text(p + o, al, text, FontId::proportional(size), Color32::from_rgba_unmultiplied(255, 255, 255, 230));
                    }
                    painter.text(p, al, text, FontId::proportional(size), col(*rgb));
                }
            }
        }
        // Nordpfeil
        let ne = Pos2::new(rect.right() - 30.0, rect.top() + 40.0);
        let dark = Color32::from_rgb(40, 40, 40);
        painter.add(egui::Shape::convex_polygon(vec![ne + Vec2::new(0.0, -16.0), ne + Vec2::new(7.0, 10.0), ne + Vec2::new(0.0, 4.0)], dark, Stroke::NONE));
        painter.add(egui::Shape::closed_line(vec![ne + Vec2::new(0.0, -16.0), ne + Vec2::new(-7.0, 10.0), ne + Vec2::new(0.0, 4.0)], Stroke::new(1.0, dark)));
        painter.text(ne + Vec2::new(0.0, -20.0), Align2::CENTER_BOTTOM, "N", FontId::proportional(13.0), dark);
        // Maßstabsleiste
        let len_m = {
            let raw = 140.0 / ppm;
            let e = raw.log10().floor();
            [5.0, 2.0, 1.0].iter().map(|m| m * 10f64.powf(e)).find(|v| *v <= raw).unwrap_or(10f64.powf(e))
        };
        let lp = (len_m * ppm) as f32;
        let sb = Pos2::new(rect.right() - 24.0 - lp, rect.bottom() - 22.0);
        painter.rect_filled(Rect::from_min_size(sb - Vec2::new(8.0, 20.0), Vec2::new(lp + 16.0, 30.0)), 6, Color32::from_rgba_unmultiplied(255, 255, 255, 220));
        painter.line_segment([sb, sb + Vec2::new(lp, 0.0)], Stroke::new(3.0, Color32::from_rgb(30, 41, 59)));
        painter.text(sb + Vec2::new(lp / 2.0, -5.0), Align2::CENTER_BOTTOM, format!("{} m", scene::trim(len_m)), FontId::proportional(11.0), Color32::from_rgb(30, 41, 59));
        // Legende
        let mut leg: Vec<(Color32, &str, u8)> = vec![(col(scene::C_FIX), "Altpunkt", 0), (col(scene::C_NEW), "Neupunkt", 1)];
        if self.style.lines {
            leg.push((col(scene::C_LINE), "Beobachtung", 2));
        }
        if self.view.adjusted && self.view.plan && self.style.ellipses {
            leg.push((col(scene::C_ELL), "Fehlerellipse", 3));
        }
        if self.view.adjusted && self.view.height && self.style.bars {
            leg.push((col(scene::C_HEIGHT), "Höhen-σ (Säule)", 4));
        }
        if self.style.vectors && self.view.adjusted {
            leg.push((col(scene::C_VEC), "Verschiebung", 2));
        }
        if self.style.outliers && self.view.lines.iter().any(|l| l.outlier) {
            leg.push((BAD, "Ausreißer", 2));
        }
        let lh = 18.0;
        let foot = if self.view.adjusted { 20.0 } else { 6.0 };
        let lr = Rect::from_min_size(Pos2::new(rect.left() + 12.0, rect.bottom() - 12.0 - lh * leg.len() as f32 - 10.0 - foot), Vec2::new(160.0, lh * leg.len() as f32 + 10.0 + foot));
        painter.rect(lr, 8, Color32::from_rgba_unmultiplied(255, 255, 255, 235), Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
        for (i, (c, t, k)) in leg.iter().enumerate() {
            let y = lr.top() + 14.0 + i as f32 * lh;
            let p = Pos2::new(lr.left() + 18.0, y);
            match k {
                0 => {
                    painter.add(egui::Shape::convex_polygon(vec![p + Vec2::new(0.0, -6.0), p + Vec2::new(5.5, 4.0), p + Vec2::new(-5.5, 4.0)], *c, Stroke::NONE));
                }
                1 => {
                    painter.circle_filled(p, 4.5, *c);
                }
                3 => {
                    painter.add(egui::Shape::closed_line(ellipse_pts(2.4, 1.2, 100.0, 24).iter().map(|q| Pos2::new(p.x + q[0] as f32 * 3.0, p.y - q[1] as f32 * 3.0)).collect(), Stroke::new(1.6, *c)));
                }
                4 => {
                    painter.rect_filled(Rect::from_center_size(p, Vec2::new(5.0, 12.0)), 1, *c);
                }
                _ => {
                    painter.line_segment([p - Vec2::new(9.0, 0.0), p + Vec2::new(9.0, 0.0)], Stroke::new(2.0, *c));
                }
            }
            painter.text(Pos2::new(lr.left() + 34.0, y), Align2::LEFT_CENTER, *t, FontId::proportional(12.0), Color32::from_rgb(30, 41, 59));
        }
        if self.view.adjusted {
            let e = self.style.effective_e(&self.view);
            painter.text(Pos2::new(lr.left() + 10.0, lr.bottom() - 11.0), Align2::LEFT_CENTER, format!("Fehlermaßstab {}{}", fmt_scale(e), if self.style.conf > 1.0 { " · 95 %" } else { "" }), FontId::proportional(11.0), MUTED);
        }
        if self.view.schematic {
            painter.text(Pos2::new(rect.left() + 12.0, rect.top() + 12.0), Align2::LEFT_TOP, "schematisch – keine Lagekoordinaten", FontId::proportional(12.0), WARN);
        } else if !self.view.adjusted {
            painter.text(Pos2::new(rect.left() + 12.0, rect.top() + 22.0), Align2::LEFT_TOP, "Näherungskoordinaten – noch nicht ausgeglichen", FontId::proportional(12.0), MUTED);
        }
        // Hover-Info
        if let Some(hp) = resp.hover_pos() {
            let best = self.view.pts.iter().map(|p| (p, to([p.y, p.x]).distance(hp))).filter(|(_, d)| *d < 12.0).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            if let Some((p, _)) = best {
                let nk = self.proj.settings.nk;
                let mut lines = vec![format!("{}  ({})", p.name, if p.fixed { "Altpunkt" } else { "Neupunkt" })];
                if !self.view.schematic {
                    lines.push(format!("Y = {:.*}   X = {:.*}", nk, p.y, nk, p.x));
                }
                if let Some(h) = p.h {
                    lines.push(format!("H = {:.*}", nk, h));
                }
                if p.ea > 0.0 {
                    lines.push(format!("sy = {:.1} mm   sx = {:.1} mm", p.sy * 1000.0, p.sx * 1000.0));
                    lines.push(format!("Ellipse A = {:.1} mm  B = {:.1} mm  θ = {:.1} gon", p.ea * 1000.0, p.eb * 1000.0, p.et));
                }
                if p.sh > 0.0 {
                    lines.push(format!("sh = {:.1} mm", p.sh * 1000.0));
                }
                let galley = painter.layout_no_wrap(lines.join("\n"), FontId::proportional(12.0), INK);
                let r = Rect::from_min_size(hp + Vec2::new(14.0, 10.0), galley.size() + Vec2::new(16.0, 12.0));
                painter.rect(r, 6, Color32::from_rgba_unmultiplied(255, 255, 255, 245), Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                painter.galley(r.min + Vec2::new(8.0, 6.0), galley, INK);
            }
        }
    }

    // ------------------------------------------------------------ Tabellen

    fn obs_table(&mut self, ui: &mut Ui) {
        let Some(out) = &self.outcome else {
            ui.add_space(20.0);
            ui.label(RichText::new("Noch keine Ausgleichung – erst rechnen (▶ Ausgleichen). Danach können hier Beobachtungen gestrichen und wieder aktiviert werden.").color(MUTED));
            return;
        };
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.only_outliers, "nur Ausreißer und abgewertete");
            ui.checkbox(&mut self.sort_nv, "nach normierter Verbesserung sortieren");
            ui.label(RichText::new("Haken entfernen = Beobachtung streichen (sofort neu gerechnet)").small().color(MUTED));
        });
        let mut toggle: Option<String> = None;
        let disabled = &self.proj.disabled;
        let (only, sort) = (self.only_outliers, self.sort_nv);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for part in &out.parts {
                ui.add_space(4.0);
                ui.label(RichText::new(&part.title).strong().size(14.0).color(ACCENT));
                let res: std::collections::HashMap<usize, &adjust::ObsResult> = part.obs.iter().map(|o| (o.obs, o)).collect();
                let mut idx: Vec<usize> = (0..part.net.obs.len()).collect();
                if sort {
                    idx.sort_by(|a, b| {
                        let na = res.get(a).map(|r| r.nv).unwrap_or(-1.0);
                        let nb = res.get(b).map(|r| r.nv).unwrap_or(-1.0);
                        nb.partial_cmp(&na).unwrap()
                    });
                }
                egui::Grid::new(format!("obs{}", part.title)).striped(true).num_columns(13).spacing([12.0, 3.0]).show(ui, |ui| {
                    for h in ["", "Stand", "Ziel", "Art", "Beobachtung", "v", "", "σ ausg.", "r", "NV", "GF", "w", "Kz"] {
                        ui.label(RichText::new(h).strong().small());
                    }
                    ui.end_row();
                    for i in idx {
                        let o = &part.net.obs[i];
                        let r = res.get(&i);
                        if only && !r.map_or(false, |r| r.outlier || r.w < 0.9) {
                            continue;
                        }
                        let is_dis = disabled.contains(&o.key);
                        let mut on = !is_dis && o.active;
                        let auto_struck = !is_dis && !o.active;
                        let tone = match r {
                            Some(r) if r.outlier => BAD,
                            Some(r) if r.w < 0.9 => WARN,
                            None => MUTED,
                            _ => INK,
                        };
                        if ui.checkbox(&mut on, "").on_hover_text(if auto_struck { "durch Grenzwertprüfung gestrichen" } else { "Beobachtung verwenden" }).changed() {
                            toggle = Some(o.key.clone());
                        }
                        let t = |s: String| RichText::new(s).color(tone).monospace();
                        ui.label(t(part.net.pts[o.from].name.clone()));
                        ui.label(t(if o.from == o.to { String::new() } else { part.net.pts[o.to].name.clone() }));
                        ui.label(t(o.kind.label().into()));
                        ui.label(t(if o.kind.angular() { format!("{:.5}", o.value) } else { format!("{:.4}", o.value) }));
                        if let Some(r) = r {
                            ui.label(t(format!("{:8.2}", r.v * 1000.0)));
                            ui.label(RichText::new(o.kind.unit()).small().color(MUTED));
                            ui.label(t(format!("{:.2}", r.sigma_adj * 1000.0)));
                            ui.label(t(format!("{:.2}", r.redundancy)));
                            ui.label(t(format!("{:.2}", r.nv)));
                            ui.label(t(format!("{:.1}", r.gf * 1000.0)));
                            ui.label(t(format!("{:.2}", r.w)));
                            ui.label(t(format!("{}{}", r.mark(), if r.outlier { " *" } else { "" })));
                        } else {
                            ui.label(t(if auto_struck { "gestrichen (Grenzwert)".into() } else { "gestrichen".into() }));
                            for _ in 0..7 {
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(10.0);
            }
            ui.label(RichText::new("v, σ und GF in mgon bzw. mm · r = Redundanzanteil (# ≤ 0,1, ## ≤ 0,03) · NV = normierte Verbesserung · GF = geschätzter grober Fehler · w = robustes Gewicht · * = Ausreißer").small().color(MUTED));
        });
        if let Some(k) = toggle {
            if let Some(p) = self.proj.disabled.iter().position(|d| *d == k) {
                self.proj.disabled.remove(p);
            } else {
                self.proj.disabled.push(k);
            }
            self.rerun = true;
        }
    }

    fn coord_table(&mut self, ui: &mut Ui) {
        let Some(out) = &self.outcome else {
            ui.add_space(20.0);
            ui.label(RichText::new("Noch keine Ausgleichung – Näherungswerte siehe Netzbild und Daten.").color(MUTED));
            return;
        };
        let nk = self.proj.settings.nk;
        let pts = out.merged_points();
        let plan = out.parts.iter().any(|p| p.net.plan);
        let height = out.parts.iter().any(|p| p.net.height);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("coords").striped(true).spacing([14.0, 3.0]).show(ui, |ui| {
                let mut head = vec!["Punkt", "Art"];
                if plan {
                    head.extend(["Rechtswert Y", "Hochwert X"]);
                }
                if height {
                    head.push("Höhe H");
                }
                if plan {
                    head.extend(["sy [mm]", "sx [mm]", "A [mm]", "B [mm]", "θ [gon]", "dy [mm]", "dx [mm]"]);
                }
                if height {
                    head.extend(["sh [mm]", "dh [mm]"]);
                }
                for h in head {
                    ui.label(RichText::new(h).strong().small());
                }
                ui.end_row();
                for p in &pts {
                    let adj = p.plan_adj || p.h_adj;
                    let c = if adj { INK } else { MUTED };
                    let t = |s: String| RichText::new(s).color(c).monospace();
                    ui.label(RichText::new(&p.name).strong().color(c));
                    ui.label(RichText::new(if (p.fix_plan || p.fix_h) && !adj { "AP" } else if p.fix_plan || p.fix_h { "AP*" } else { "NP" }).small().color(if adj { ACCENT } else { MUTED }));
                    if plan {
                        ui.label(t(format!("{:.*}", nk, p.y)));
                        ui.label(t(format!("{:.*}", nk, p.x)));
                    }
                    if height {
                        ui.label(t(if p.h.is_finite() { format!("{:.*}", nk, p.h) } else { "-".into() }));
                    }
                    let mm = |v: f64, on: bool| if on { format!("{:.1}", v * 1000.0) } else { String::new() };
                    if plan {
                        for v in [mm(p.sy, p.plan_adj), mm(p.sx, p.plan_adj), mm(p.ea, p.plan_adj), mm(p.eb, p.plan_adj), if p.plan_adj { format!("{:.1}", p.et) } else { String::new() }, mm(p.dy, p.plan_adj), mm(p.dx, p.plan_adj)] {
                            ui.label(t(v));
                        }
                    }
                    if height {
                        ui.label(t(mm(p.sh, p.h_adj)));
                        ui.label(t(mm(p.dh, p.h_adj)));
                    }
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            for part in &out.parts {
                if !part.orient.is_empty() {
                    ui.label(RichText::new(format!("Orientierungen (t-r) – {}", part.title)).strong());
                    egui::Grid::new(format!("ori{}", part.title)).striped(true).spacing([14.0, 2.0]).show(ui, |ui| {
                        ui.label(RichText::new("Standpunkt").strong().small());
                        ui.label(RichText::new("Orientierung [gon]").strong().small());
                        ui.label(RichText::new("σ [mgon]").strong().small());
                        ui.end_row();
                        for (n, o, s) in &part.orient {
                            ui.label(n);
                            ui.label(RichText::new(format!("{:.5}", o)).monospace());
                            ui.label(RichText::new(format!("{:.2}", s)).monospace());
                            ui.end_row();
                        }
                    });
                }
            }
        });
    }

    fn render_doc(ui: &mut Ui, doc: &Doc) {
        for l in &doc.header {
            ui.label(RichText::new(l).small().color(MUTED));
        }
        for (bi, b) in doc.blocks.iter().enumerate() {
            match b {
                Block::Title(t) => {
                    ui.add_space(10.0);
                    ui.label(RichText::new(t).size(18.0).strong().color(Color32::from_rgb(22, 62, 120)));
                }
                Block::Heading(t) => {
                    ui.add_space(6.0);
                    ui.label(RichText::new(t).size(14.5).strong().color(Color32::from_rgb(22, 62, 120)));
                }
                Block::Para(t) => {
                    ui.label(RichText::new(t).small().color(MUTED));
                }
                Block::Note(t, tone) => {
                    ui.label(RichText::new(t).color(tone_color(*tone)));
                }
                Block::KeyVals(v) => {
                    egui::Grid::new(format!("kv{}", bi)).num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
                        for (k, val) in v {
                            ui.label(RichText::new(k).color(MUTED));
                            ui.label(val);
                            ui.end_row();
                        }
                    });
                }
                Block::Table(t) => {
                    egui::Grid::new(format!("tb{}", bi)).striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
                        for (h, _, _) in &t.cols {
                            ui.label(RichText::new(h).strong().small());
                        }
                        ui.end_row();
                        for (r, tone) in &t.rows {
                            for (i, c) in r.iter().enumerate() {
                                let txt = RichText::new(c).monospace().color(tone_color(*tone));
                                if t.cols.get(i).map(|c| c.1) == Some(Align::R) {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(txt));
                                } else {
                                    ui.label(txt);
                                }
                            }
                            ui.end_row();
                        }
                    });
                }
            }
        }
    }

    fn protocol(&mut self, ui: &mut Ui) {
        let Some(doc) = &self.doc else {
            ui.add_space(20.0);
            ui.label(RichText::new("Das Protokoll entsteht nach der Ausgleichung und kann als RTF gespeichert werden.").color(MUTED));
            return;
        };
        let mut save = false;
        let mut copied = false;
        ui.horizontal(|ui| {
            if ui.button("📄 Als RTF speichern…").clicked() {
                save = true;
            }
            if ui.button("📋 Als Text kopieren").clicked() {
                ui.ctx().copy_text(doc.plain_text());
                copied = true;
            }
        });
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, BORDER)).corner_radius(8).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
                Self::render_doc(ui, doc);
            });
        });
        if copied {
            self.status = "Protokoll als Text in die Zwischenablage kopiert.".into();
        }
        if save {
            self.export_rtf();
        }
    }

    fn data_view(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (i, t) in ["Altpunkte (KOR)", "Tachymeter (DAT)", "Nivellement", "Näherungswerte"].iter().enumerate() {
                if ui.selectable_label(self.data_tab == i, *t).clicked() {
                    self.data_tab = i;
                }
            }
        });
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            if self.data_tab < 3 {
                let mut t = self.raw[self.data_tab].as_str();
                if t.is_empty() {
                    ui.label(RichText::new("keine Datei").color(MUTED));
                } else {
                    ui.add(egui::TextEdit::multiline(&mut t).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
                }
            } else {
                let nk = self.proj.settings.nk;
                egui::Grid::new("approx").striped(true).spacing([14.0, 2.0]).show(ui, |ui| {
                    for h in ["Punkt", "Y", "X", "H"] {
                        ui.label(RichText::new(h).strong().small());
                    }
                    ui.end_row();
                    let mut names: Vec<&String> = self.approx_pm.keys().collect();
                    names.sort();
                    for n in names {
                        let a = self.approx_pm[n];
                        ui.label(n);
                        ui.label(RichText::new(a.plan.map(|p| format!("{:.*}", nk, p.0)).unwrap_or_else(|| "-".into())).monospace());
                        ui.label(RichText::new(a.plan.map(|p| format!("{:.*}", nk, p.1)).unwrap_or_else(|| "-".into())).monospace());
                        ui.label(RichText::new(a.h.map(|h| format!("{:.*}", nk, h)).unwrap_or_else(|| "-".into())).monospace());
                        ui.end_row();
                    }
                });
                if let Some(out) = &self.outcome {
                    ui.add_space(8.0);
                    ui.label(RichText::new("Bestimmung der Näherungswerte").strong());
                    for l in &out.approx_log {
                        ui.label(RichText::new(l).small().color(MUTED));
                    }
                }
            }
        });
    }

    // ------------------------------------------------------------ Dialoge

    fn file_row(ui: &mut Ui, label: &str, val: &mut String, filters: &[(&str, &[&str])], dir: &str) -> bool {
        let mut changed = false;
        ui.label(label);
        ui.horizontal(|ui| {
            changed |= ui.add(egui::TextEdit::singleline(val).desired_width(330.0)).lost_focus();
            if ui.button("…").clicked() {
                if let Some(p) = pick(label, filters, dir) {
                    *val = p.to_string_lossy().to_string();
                    changed = true;
                }
            }
        });
        ui.end_row();
        changed
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let dir = self.dir_str();
        if self.dlg_project {
            let mut open = true;
            let mut reload = false;
            let mut save = false;
            egui::Window::new("Projektbeschrieb").open(&mut open).default_width(520.0).show(ctx, |ui| {
                egui::Grid::new("pj").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    let p = &mut self.proj;
                    for (l, v) in [("Projektname", &mut p.name), ("Nummer", &mut p.nummer), ("Ort", &mut p.ort), ("Ort (Zusatz)", &mut p.ort1), ("Bearbeiter", &mut p.bearbeiter)] {
                        ui.label(l);
                        ui.add(egui::TextEdit::singleline(v).desired_width(360.0));
                        ui.end_row();
                    }
                    ui.label("Beschreibung");
                    ui.add(egui::TextEdit::multiline(&mut p.beschreibung).desired_rows(2).desired_width(360.0));
                    ui.end_row();
                    ui.label("Berechnungsart");
                    egui::ComboBox::from_id_salt("pkind").selected_text(p.kind.label()).show_ui(ui, |ui| {
                        for k in NetKind::ALL {
                            ui.selectable_value(&mut p.kind, k, k.label());
                        }
                    });
                    ui.end_row();
                    reload |= Self::file_row(ui, "Altpunkte (*.KOR)", &mut p.kor, KOR_F, &dir);
                    reload |= Self::file_row(ui, "Meßdaten (*.DAT/*.DAM)", &mut p.mes, DAT_F, &dir);
                    reload |= Self::file_row(ui, "Nivellement (optional)", &mut p.niv, DAT_F, &dir);
                    reload |= Self::file_row(ui, "Lagekoordinaten (trig. Höhen)", &mut p.lagekor, KOR_F, &dir);
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Bei der Projektart „Nivellement“ darf die Meßdatei auch die Nivellementsdatei sein.").small().color(MUTED));
                ui.horizontal(|ui| {
                    if ui.button("Daten einlesen").clicked() {
                        reload = true;
                    }
                    if ui.button("Projekt speichern…").clicked() {
                        save = true;
                    }
                });
            });
            if reload {
                self.sample_mode = false;
                self.load_data();
            }
            if save {
                let ask = self.proj_path.is_none();
                self.save_project(ask);
            }
            self.dlg_project &= open;
        }
        if self.dlg_params {
            let mut open = true;
            let mut apply = false;
            egui::Window::new("Voreinstellungen").open(&mut open).default_width(460.0).show(ctx, |ui| {
                ui.label(RichText::new("Firmenkopf (Protokollkopf)").strong());
                while self.sys.firmenkopf.len() < 4 {
                    self.sys.firmenkopf.push(String::new());
                }
                for l in self.sys.firmenkopf.iter_mut().take(4) {
                    ui.add(egui::TextEdit::singleline(l).desired_width(420.0));
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Genauigkeiten, Grenzwerte und Nachkommastellen stehen in der Seitenleiste und werden mit dem Projekt gespeichert.").small().color(MUTED));
                if ui.button("Übernehmen").clicked() {
                    apply = true;
                }
            });
            if apply || !open {
                self.save_sys();
                if let Some(out) = &self.outcome {
                    self.doc = Some(report::adjustment_doc(out, &self.proj.settings, &self.proj, &self.sys.firmenkopf));
                }
            }
            self.dlg_params &= open;
        }
        if self.dlg_pdf {
            let mut open = true;
            let mut go = false;
            let auto_n = export::auto_scale(&self.view, &self.pdf_opts);
            let title = self.plot_title();
            let e = self.style.effective_e(&self.view);
            egui::Window::new("Netzbild als PDF").open(&mut open).default_width(360.0).show(ctx, |ui| {
                let o = &mut self.pdf_opts;
                ui.horizontal(|ui| {
                    ui.label("Papier");
                    ui.selectable_value(&mut o.paper, Paper::A4, "A4");
                    ui.selectable_value(&mut o.paper, Paper::A3, "A3");
                    ui.separator();
                    ui.selectable_value(&mut o.landscape, true, "quer");
                    ui.selectable_value(&mut o.landscape, false, "hoch");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.pdf_auto, "Maßstab automatisch");
                    if !self.pdf_auto {
                        ui.label("1 :");
                        if o.scale_n <= 0.0 {
                            o.scale_n = auto_n;
                        }
                        ui.add(egui::DragValue::new(&mut o.scale_n).speed(50.0).range(1.0..=1_000_000.0));
                    } else {
                        ui.label(RichText::new(format!("(1:{})", scene::trim(auto_n))).color(MUTED));
                    }
                });
                ui.checkbox(&mut o.grid, "Koordinatengitter");
                ui.horizontal(|ui| {
                    ui.label("Titel");
                    if o.title == "Netzbild" {
                        o.title = title.clone();
                    }
                    ui.add(egui::TextEdit::singleline(&mut o.title).desired_width(250.0));
                });
                ui.label(RichText::new(format!("Fehlerellipsen {} – Inhalt wie im Netzbild (Schalter über der Grafik)", fmt_scale(e))).small().color(MUTED));
                ui.checkbox(&mut self.open_after, "nach dem Speichern öffnen");
                if ui.add(egui::Button::new(RichText::new("PDF speichern…").strong().color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    go = true;
                }
            });
            if go {
                self.export_pdf();
            }
            self.dlg_pdf &= open;
        }
        if self.dlg_dxf {
            let mut open = true;
            let mut go = false;
            egui::Window::new("Netzbild als DXF").open(&mut open).default_width(340.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Zeichengrößen für Maßstab 1 :");
                    ui.add(egui::DragValue::new(&mut self.dxf_n).speed(50.0).range(1.0..=1_000_000.0));
                });
                ui.checkbox(&mut self.dxf_grid, "Koordinatengitter");
                ui.label(RichText::new("Ebenen wie WAUSGL.SYS: 11 Punktsymbole, 12 Linien, 13 Punktnummern, 14 Höhen, 16 Fehlerellipsen, 17 Achsen, 18 Fehlervektoren, 19 Ausreißer, 21 Gitter. Punkte mit Höhe als Z.").small().color(MUTED));
                if ui.add(egui::Button::new(RichText::new("DXF speichern…").strong().color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    go = true;
                }
            });
            if go {
                self.export_dxf();
            }
            self.dlg_dxf &= open;
        }
        if self.dlg_satz {
            self.satz_dialog(ctx, &dir);
        }
        if self.dlg_trans {
            self.trans_dialog(ctx, &dir);
        }
        if self.dlg_help {
            let mut open = true;
            egui::Window::new("Arbeitsweise und Dateiformate").open(&mut open).default_width(560.0).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    ui.label(RichText::new("Arbeitsablauf").strong());
                    ui.label("1. Projekt anlegen (Datei ▸ Projekt neu anlegen) und Altpunkt- sowie Meßdatei wählen, oder ein altes WAUSGL-Projekt (*.PRJ) importieren.");
                    ui.label("2. Berechnungsart wählen: Lagenetz, Höhennetz (Nivellement / trig.), 2D-Lage + Höhe, volle 3D-Ausgleichung oder Vorwärtsschnitte.");
                    ui.label("3. Variante (normal, Maßstab, Altpunkte als Beobachtung, freies Netz), Lösungsverfahren und bei Bedarf eine robuste Schätzung wählen – dann ▶ Ausgleichen.");
                    ui.label("4. Im Bereich Beobachtungen Ausreißer prüfen und per Haken streichen; das Netz wird sofort neu gerechnet.");
                    ui.label("5. Ausgabe: Protokoll als RTF, Netzbild als PDF oder DXF, Neupunkte als *.KON (Code 009).");
                    ui.add_space(6.0);
                    ui.label(RichText::new("Altpunkte (*.KOR)").strong());
                    ui.label(RichText::new("Nummer  Rechtswert  Hochwert  [Höhe]  [Code]").monospace());
                    ui.label(RichText::new("Meßdaten (*.DAT, CADdy)").strong());
                    ui.label(RichText::new("-Standpunkt  Instrumentenhöhe  [Bemerkung]\nZiel  Hz  Schrägstrecke  Zenitwinkel  Zielhöhe  Code\nZiel  Hz  Horizontalstrecke*  dH  Zielhöhe  Code").monospace());
                    ui.label(RichText::new("Nivellement").strong());
                    ui.label(RichText::new("-Standpunkt  0\nZiel  dH  Strecke[m]").monospace());
                    ui.add_space(6.0);
                    ui.label(RichText::new("Robuste Schätzer").strong());
                    ui.label("Huber, Dänische Methode, IGG-III, Tukey und L1 gewichten Beobachtungen mit großen standardisierten Verbesserungen schrittweise ab. Abgewertete Beobachtungen erscheinen orange, Ausreißer rot (im Netzbild als rote Linien).");
                    ui.label(RichText::new("Lösungsverfahren").strong());
                    ui.label("Gauß-Jordan entspricht dem Original. Cholesky ist schnell, QR (Householder) rechnet ohne Normalgleichungen und ist numerisch am stabilsten, SVD zeigt zusätzlich die Konditionszahl.");
                });
            });
            self.dlg_help &= open;
        }
        if self.dlg_about {
            let mut open = true;
            egui::Window::new("Über AUSGL-NT").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(RichText::new("AUSGL-NT 1.0").size(18.0).strong());
                ui.label("Geodätische Netzausgleichung nach vermittelnden Beobachtungen");
                ui.label("Lagenetz · Höhennetz · 2D+Höhe · 3D · Vorwärtsschnitte · Satzmittel · Transformation");
                ui.label(RichText::new("Nachfolger von WAUSGL / AUSGL (Delphi, 1996–1998)").color(MUTED));
            });
            self.dlg_about &= open;
        }
    }

    fn satz_dialog(&mut self, ctx: &egui::Context, dir: &str) {
        let mut open = true;
        let mut compute = false;
        let mut save_dam = false;
        let mut save_rtf = false;
        let mut use_it = false;
        egui::Window::new("Satzmittelbildung").open(&mut open).default_size([760.0, 520.0]).show(ctx, |ui| {
            egui::Grid::new("sz").num_columns(2).show(ui, |ui| {
                Self::file_row(ui, "Meßdatei mit Sätzen", &mut self.satz_file, DAT_F, dir);
            });
            ui.horizontal(|ui| {
                if ui.button("Satzmittel bilden").clicked() {
                    compute = true;
                }
                ui.label(RichText::new("Gleiche Standpunkte hintereinander = Sätze; Lage II (Zenit > 200 gon) wird mit Lage I gemittelt. Leer = aktuelle Meßdaten.").small().color(MUTED));
            });
            if let Some((st, _)) = &self.satz {
                ui.horizontal(|ui| {
                    if ui.button("Als *.DAM speichern…").clicked() {
                        save_dam = true;
                    }
                    if ui.button("Protokoll RTF…").clicked() {
                        save_rtf = true;
                    }
                    if ui.button("Im Projekt verwenden").clicked() {
                        use_it = true;
                    }
                });
                egui::ScrollArea::both().show(ui, |ui| {
                    let doc = report::set_mean_doc(st, &self.proj, &[]);
                    Self::render_doc(ui, &doc);
                });
            }
        });
        if compute {
            let text = if self.satz_file.is_empty() {
                Ok(self.raw[1].clone())
            } else {
                match self.proj.resolve(&self.base_dir(), &self.satz_file) {
                    Some(p) => io::read_text(&p).map_err(|e| e.to_string()),
                    None => Ok(self.raw[1].clone()),
                }
            };
            match text {
                Ok(t) => {
                    let setups = io::parse_tachy(&t);
                    let res = tools::set_means(&setups);
                    self.status = format!("Satzmittel für {} Standpunkte gebildet.", res.0.len());
                    self.satz = Some(res);
                }
                Err(e) => self.status = format!("Meßdatei nicht lesbar: {}", e),
            }
        }
        if save_dam {
            if let Some((_, means)) = &self.satz {
                if let Some(p) = save_dialog("Satzmittel speichern", &format!("{}.dam", self.proj.name), &[("Meßdaten", &["dam", "DAM"])], dir) {
                    let _ = std::fs::write(&p, io::write_tachy(means));
                    self.status = format!("Satzmittel gespeichert: {}", p.display());
                }
            }
        }
        if save_rtf {
            if let Some((st, _)) = &self.satz {
                let doc = report::set_mean_doc(st, &self.proj, &self.sys.firmenkopf);
                if let Some(p) = save_dialog("Protokoll Satzmittel", &format!("{}_satzmittel.rtf", self.proj.name), &[("Rich Text", &["rtf"])], dir) {
                    let _ = std::fs::write(&p, crate::rtf::to_rtf(&doc, true));
                    if self.open_after {
                        open_file(&p);
                    }
                }
            }
        }
        if use_it {
            if let Some((_, means)) = self.satz.clone() {
                self.setups = means.clone();
                self.raw[1] = io::write_tachy(&means);
                self.after_load();
                self.status = "Satzmittel werden als Meßdaten verwendet.".into();
            }
        }
        self.dlg_satz &= open;
    }

    fn trans_dialog(&mut self, ctx: &egui::Context, dir: &str) {
        let mut open = true;
        let mut compute = false;
        let mut save_kor = false;
        let mut save_rtf = false;
        egui::Window::new("Räumliche Transformation").open(&mut open).default_size([640.0, 520.0]).show(ctx, |ui| {
            egui::Grid::new("tr").num_columns(2).show(ui, |ui| {
                Self::file_row(ui, "Startsystem (*.KOR)", &mut self.trans_src, KOR_F, dir);
                Self::file_row(ui, "Zielsystem (*.KOR)", &mut self.trans_dst, KOR_F, dir);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.trans_scale, "Maßstab schätzen (7 Parameter)");
                if ui.button("Transformieren").clicked() {
                    compute = true;
                }
            });
            ui.label(RichText::new("Gleichnamige Punkte beider Dateien sind identische Punkte (mindestens 3).").small().color(MUTED));
            match &self.trans {
                Some(Ok(t)) => {
                    ui.horizontal(|ui| {
                        if ui.button("Transformierte Punkte speichern…").clicked() {
                            save_kor = true;
                        }
                        if ui.button("Protokoll RTF…").clicked() {
                            save_rtf = true;
                        }
                    });
                    egui::ScrollArea::both().show(ui, |ui| {
                        let doc = report::trans_doc(t, &self.trans_src, &self.trans_dst, &self.proj, &[]);
                        Self::render_doc(ui, &doc);
                    });
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(e).color(BAD));
                }
                None => {}
            }
        });
        if compute {
            let rd = |f: &str| io::read_text(Path::new(f)).map(|t| io::parse_kor(&t)).map_err(|e| format!("{}: {}", f, e));
            self.trans = Some(match (rd(&self.trans_src), rd(&self.trans_dst)) {
                (Ok(a), Ok(b)) => tools::helmert3d(&a, &b, self.trans_scale),
                (Err(e), _) | (_, Err(e)) => Err(e),
            });
        }
        if let Some(Ok(t)) = &self.trans {
            if save_kor {
                if let Some(p) = save_dialog("Transformierte Punkte", "transformiert.kor", &[("Koordinaten", &["kor"])], dir) {
                    let _ = std::fs::write(&p, io::write_kor(&t.transformed, 4));
                    self.status = format!("Transformierte Punkte gespeichert: {}", p.display());
                }
            }
            if save_rtf {
                let doc = report::trans_doc(t, &self.trans_src, &self.trans_dst, &self.proj, &self.sys.firmenkopf);
                if let Some(p) = save_dialog("Protokoll Transformation", "transformation.rtf", &[("Rich Text", &["rtf"])], dir) {
                    let _ = std::fs::write(&p, crate::rtf::to_rtf(&doc, false));
                    if self.open_after {
                        open_file(&p);
                    }
                }
            }
        }
        self.dlg_trans &= open;
    }
}

impl eframe::App for AusglApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.rerun = true;
        }
        egui::Panel::top("menu").show(ui, |ui| {
            self.menu(ui);
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let c = if self.status.contains("nicht möglich") || self.status.contains("Fehler") { BAD } else { MUTED };
                ui.label(RichText::new(&self.status).small().color(c));
            });
        });
        egui::Panel::left("side").resizable(true).default_size(320.0).show(ui, |ui| self.side(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (t, l) in [(Tab::Netz, "🗺  Netzbild"), (Tab::Beob, "📏  Beobachtungen"), (Tab::Koord, "📍  Koordinaten"), (Tab::Protokoll, "📄  Protokoll"), (Tab::Daten, "🗂  Daten")] {
                    let sel = self.tab == t;
                    let txt = RichText::new(l).size(14.0).color(if sel { ACCENT } else { Color32::from_rgb(51, 65, 85) });
                    if ui.add(egui::Button::selectable(sel, txt)).clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.add_space(4.0);
            match self.tab {
                Tab::Netz => self.net_view(ui),
                Tab::Beob => self.obs_table(ui),
                Tab::Koord => self.coord_table(ui),
                Tab::Protokoll => self.protocol(ui),
                Tab::Daten => self.data_view(ui),
            }
        });
        self.dialogs(&ctx);
        if self.rerun {
            self.rerun = false;
            self.run();
        }

        // Screenshot-Modus
        self.frame += 1;
        if self.screenshot.is_some() {
            ctx.request_repaint();
            if self.frame == 120 && !self.shot_requested {
                self.shot_requested = true;
                let shot = self.shot.clone();
                ctx.request_screenshot(move |img| *shot.lock().unwrap() = Some(img));
            }
            if self.frame > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let img = self.shot.lock().unwrap().take();
            if let Some(img) = img {
                if let Some(path) = &self.screenshot {
                    save_png(&img, path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self) {
        self.save_sys();
    }
}

fn save_png(img: &egui::ColorImage, path: &Path) -> bool {
    let [w, h] = img.size;
    let mut buf = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        buf.extend_from_slice(&[c.r(), c.g(), c.b(), 255]);
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    image::save_buffer(path, &buf, w as u32, h as u32, image::ColorType::Rgba8).is_ok()
}
