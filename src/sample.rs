//! Beispielnetz (frei erfunden): 4 Altpunkte, 5 Neupunkte, Tachymeter- und Nivellementsmessungen
//! mit normalverteiltem Rauschen und – auf Wunsch – einem groben Fehler in einer Richtung.

use crate::adjust::Input;
use crate::geo::{norm400, riwi, RHO, RM};
use crate::io::{KorPoint, NetKind, NivLine, Setup, Shot};

pub struct Sample {
    pub kor: Vec<KorPoint>,
    pub setups: Vec<Setup>,
    pub niv: Vec<NivLine>,
    pub truth: Vec<(String, f64, f64, f64)>,
    pub gross_line: usize,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn gauss(&mut self) -> f64 {
        let u1 = self.next().max(1e-12);
        let u2 = self.next();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

const PTS: [(&str, f64, f64, f64, bool); 9] = [
    ("1001", 4_468_000.000, 5_333_000.000, 512.300, true),
    ("1002", 4_468_620.000, 5_333_080.000, 518.700, true),
    ("1003", 4_468_540.000, 5_333_610.000, 530.150, true),
    ("1004", 4_467_980.000, 5_333_560.000, 524.900, true),
    ("10", 4_468_180.000, 5_333_190.000, 515.400, false),
    ("11", 4_468_400.000, 5_333_230.000, 519.800, false),
    ("12", 4_468_330.000, 5_333_430.000, 524.200, false),
    ("13", 4_468_120.000, 5_333_400.000, 521.000, false),
    ("14", 4_468_480.000, 5_333_380.000, 523.300, false),
];

const SETUPS: [(&str, &[&str]); 8] = [
    ("1001", &["1004", "13", "10", "11", "1002"]),
    ("1002", &["1001", "10", "11", "14", "1003"]),
    ("1003", &["1002", "14", "12", "13", "1004"]),
    ("1004", &["1003", "12", "13", "10", "1001"]),
    ("10", &["1001", "1002", "11", "12", "13", "1004"]),
    ("11", &["10", "1001", "1002", "14", "12", "13"]),
    ("12", &["13", "10", "11", "14", "1003", "1004"]),
    ("14", &["11", "1002", "1003", "12"]),
];

impl Sample {
    pub fn generate(gross: bool) -> Sample {
        let mut rng = Rng(20_241_112);
        let p = |n: &str| PTS.iter().find(|p| p.0 == n).unwrap();
        let kor = PTS.iter().filter(|p| p.4).map(|p| KorPoint { name: p.0.into(), y: p.1, x: p.2, h: Some(p.3), code: "015".into() }).collect();
        let mut setups = Vec::new();
        let mut line = 0usize;
        let mut gross_line = 0;
        let k_r = 1.0 - crate::geo::K_REFR;
        for (si, (st, targets)) in SETUPS.iter().enumerate() {
            let s = p(st);
            let o = norm400(37.0 + 53.7 * si as f64);
            let ih = 1.45 + 0.05 * (si % 3) as f64;
            line += 1;
            let st_line = line;
            let mut shots = Vec::new();
            for (ti, t) in targets.iter().enumerate() {
                let q = p(t);
                line += 1;
                let th = 1.30 + 0.1 * (ti % 3) as f64;
                let (tt, sh) = riwi(s.1, s.2, q.1, q.2);
                let dz_geo = (q.3 + th) - (s.3 + ih) - sh * sh / (2.0 * RM);
                let dz_app = (q.3 + th) - (s.3 + ih) - k_r * sh * sh / (2.0 * RM);
                let slope = (sh * sh + dz_geo * dz_geo).sqrt();
                let z = sh.atan2(dz_app) * RHO;
                let mut hz = norm400(tt - o + rng.gauss() * 0.0005);
                if gross && *st == "11" && *t == "13" {
                    hz = norm400(hz + 0.030);
                    gross_line = line;
                }
                let d = slope + rng.gauss() * 0.003;
                let zz = z + rng.gauss() * 0.001;
                shots.push(Shot { target: t.to_string(), hz: Some(hz), dist: Some(d), dist_horizontal: false, zenith: Some(zz), dh: None, th, code: "00".into(), line, sigma_hz: None, sigma_dist: None, sigma_v: None });
            }
            setups.push(Setup { station: st.to_string(), ih, remark: String::new(), shots, line: st_line });
        }
        let mut niv = Vec::new();
        let mut nline = 0;
        for (a, b) in [("1001", "10"), ("10", "11"), ("11", "1002"), ("1004", "13"), ("13", "12"), ("12", "1003"), ("11", "14"), ("14", "1003"), ("10", "13")] {
            let (pa, pb) = (p(a), p(b));
            let s = riwi(pa.1, pa.2, pb.1, pb.2).1 * 1.1;
            nline += 2;
            niv.push(NivLine { from: a.into(), to: b.into(), dh: pb.3 - pa.3 + rng.gauss() * 0.002 * (s / 1000.0).sqrt(), s: (s * 10.0).round() / 10.0, line: nline, sigma: None });
        }
        let truth = PTS.iter().map(|p| (p.0.to_string(), p.1, p.2, p.3)).collect();
        Sample { kor, setups, niv, truth, gross_line }
    }

    pub fn input(&self, kind: NetKind) -> Input<'_> {
        Input { kind, kor: &self.kor, lagekor: &[], kon: &[], approx: &[], hfix: &[], setups: &self.setups, niv: &self.niv, disabled: &[] }
    }

    pub fn truth(&self, name: &str) -> (f64, f64, f64) {
        self.truth.iter().find(|t| t.0 == name).map(|t| (t.1, t.2, t.3)).unwrap_or((f64::NAN, f64::NAN, f64::NAN))
    }

    pub fn kor_text(&self) -> String {
        format!("# Beispielnetz – Altpunkte (Nummer  Rechtswert  Hochwert  Höhe  Code)\r\n{}", crate::io::write_kor(&self.kor, 3))
    }

    pub fn dat_text(&self) -> String {
        format!("# Beispielnetz – Tachymetermessungen (CADdy-Format)\r\n{}", crate::io::write_tachy(&self.setups))
    }

    pub fn niv_text(&self) -> String {
        let mut s = String::from("# Beispielnetz – Nivellement (Standpunkt / Ziel dH Strecke)\r\n");
        for n in &self.niv {
            s += &format!("-{:<14} 0\r\n{:<15}{:>12.4}{:>10.1}\r\n", n.from, n.to, n.dh, n.s);
        }
        s
    }
}
