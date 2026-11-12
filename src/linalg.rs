//! Dichte Matrizen und die wählbaren Lösungsverfahren der Ausgleichung.
//!
//! Alle Verfahren lösen dasselbe Problem  min |A·dx − l|²  (A, l bereits mit √p gewichtet)
//! und liefern dx sowie die Kofaktormatrix Qxx = (AᵀA)⁻¹:
//! * Gauß-Jordan: Normalgleichungen N = AᵀA, Inversion durch Austauschschritte (wie das Original
//!   ULAGE.PAS/gauss, das die gepackte Normalgleichungsmatrix in sich invertiert)
//! * Cholesky: N = L·Lᵀ (schnell, numerisch gut bei regulären Netzen)
//! * QR (Householder): A = Q·R direkt, ohne Normalgleichungen (halbe Konditionszahl)
//! * SVD (Jacobi): A = U·Σ·Vᵀ; zeigt Rangdefekte und Konditionszahl

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Mat {
    pub r: usize,
    pub c: usize,
    pub d: Vec<f64>,
}

impl Mat {
    pub fn zeros(r: usize, c: usize) -> Self {
        Mat { r, c, d: vec![0.0; r * c] }
    }
    pub fn eye(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }
    pub fn t(&self) -> Mat {
        let mut m = Mat::zeros(self.c, self.r);
        for i in 0..self.r {
            for j in 0..self.c {
                m[(j, i)] = self[(i, j)];
            }
        }
        m
    }
    pub fn mul(&self, o: &Mat) -> Mat {
        assert_eq!(self.c, o.r);
        let mut m = Mat::zeros(self.r, o.c);
        for i in 0..self.r {
            for k in 0..self.c {
                let a = self[(i, k)];
                if a == 0.0 {
                    continue;
                }
                for j in 0..o.c {
                    m.d[i * o.c + j] += a * o.d[k * o.c + j];
                }
            }
        }
        m
    }
    pub fn mulv(&self, v: &[f64]) -> Vec<f64> {
        (0..self.r).map(|i| (0..self.c).map(|j| self[(i, j)] * v[j]).sum()).collect()
    }
    /// AᵀA
    pub fn ata(&self) -> Mat {
        let mut n = Mat::zeros(self.c, self.c);
        for row in 0..self.r {
            let a = &self.d[row * self.c..(row + 1) * self.c];
            for i in 0..self.c {
                if a[i] == 0.0 {
                    continue;
                }
                for j in i..self.c {
                    n.d[i * self.c + j] += a[i] * a[j];
                }
            }
        }
        for i in 0..self.c {
            for j in 0..i {
                n.d[i * self.c + j] = n.d[j * self.c + i];
            }
        }
        n
    }
    /// Aᵀl
    pub fn atl(&self, l: &[f64]) -> Vec<f64> {
        let mut v = vec![0.0; self.c];
        for row in 0..self.r {
            let lr = l[row];
            if lr == 0.0 {
                continue;
            }
            for j in 0..self.c {
                v[j] += self.d[row * self.c + j] * lr;
            }
        }
        v
    }
    pub fn row(&self, i: usize) -> &[f64] {
        &self.d[i * self.c..(i + 1) * self.c]
    }
}

impl std::ops::Index<(usize, usize)> for Mat {
    type Output = f64;
    fn index(&self, (i, j): (usize, usize)) -> &f64 {
        &self.d[i * self.c + j]
    }
}
impl std::ops::IndexMut<(usize, usize)> for Mat {
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut f64 {
        &mut self.d[i * self.c + j]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Solver {
    /// wie im Original: Normalgleichungen, Inversion durch Austauschverfahren
    #[default]
    GaussJordan,
    Cholesky,
    QrHouseholder,
    Svd,
}

impl Solver {
    pub const ALL: [Solver; 4] = [Solver::GaussJordan, Solver::Cholesky, Solver::QrHouseholder, Solver::Svd];
    pub fn label(&self) -> &'static str {
        match self {
            Solver::GaussJordan => "Gauß-Jordan (Normalgleichungen, wie Original)",
            Solver::Cholesky => "Cholesky-Zerlegung (Normalgleichungen)",
            Solver::QrHouseholder => "QR-Zerlegung (Householder, ohne Normalgl.)",
            Solver::Svd => "Singulärwertzerlegung (SVD, Jacobi)",
        }
    }
    pub fn short(&self) -> &'static str {
        match self {
            Solver::GaussJordan => "Gauß-Jordan",
            Solver::Cholesky => "Cholesky",
            Solver::QrHouseholder => "QR (Householder)",
            Solver::Svd => "SVD",
        }
    }
}

pub struct Solution {
    pub dx: Vec<f64>,
    pub qxx: Mat,
    /// Konditionszahl (nur SVD, sonst Schätzung aus der Diagonale)
    pub cond: Option<f64>,
    /// numerischer Rang (SVD)
    pub rank: Option<usize>,
}

#[derive(Debug)]
pub struct SolveError(pub String);

/// Inverse einer symmetrischen positiv definiten Matrix durch Gauß-Jordan mit Pivotsuche.
pub fn invert_gauss_jordan(m: &Mat) -> Result<Mat, SolveError> {
    let n = m.r;
    let mut a = m.clone();
    let mut inv = Mat::eye(n);
    let scale = (0..n).map(|i| a[(i, i)].abs()).fold(0.0, f64::max).max(1e-300);
    for col in 0..n {
        let mut piv = col;
        let mut best = a[(col, col)].abs();
        for r in col + 1..n {
            if a[(r, col)].abs() > best {
                best = a[(r, col)].abs();
                piv = r;
            }
        }
        if best <= 1e-13 * scale {
            return Err(SolveError(format!("Normalgleichungsmatrix singulär (Unbekannte {}): Netz nicht ausreichend bestimmt / Datumsdefekt", col + 1)));
        }
        if piv != col {
            for j in 0..n {
                a.d.swap(col * n + j, piv * n + j);
                inv.d.swap(col * n + j, piv * n + j);
            }
        }
        let p = a[(col, col)];
        for j in 0..n {
            a[(col, j)] /= p;
            inv[(col, j)] /= p;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let fct = a[(r, col)];
            if fct == 0.0 {
                continue;
            }
            for j in 0..n {
                a.d[r * n + j] -= fct * a.d[col * n + j];
                inv.d[r * n + j] -= fct * inv.d[col * n + j];
            }
        }
    }
    Ok(inv)
}

/// Cholesky N = L·Lᵀ, liefert L (untere Dreiecksmatrix)
pub fn cholesky(m: &Mat) -> Result<Mat, SolveError> {
    let n = m.r;
    let mut l = Mat::zeros(n, n);
    let scale = (0..n).map(|i| m[(i, i)].abs()).fold(0.0, f64::max).max(1e-300);
    for j in 0..n {
        let mut s = m[(j, j)];
        for k in 0..j {
            s -= l[(j, k)] * l[(j, k)];
        }
        if s <= 1e-13 * scale {
            return Err(SolveError(format!("Cholesky: Matrix nicht positiv definit (Unbekannte {}): Netz nicht ausreichend bestimmt", j + 1)));
        }
        let d = s.sqrt();
        l[(j, j)] = d;
        for i in j + 1..n {
            let mut s = m[(i, j)];
            for k in 0..j {
                s -= l[(i, k)] * l[(j, k)];
            }
            l[(i, j)] = s / d;
        }
    }
    Ok(l)
}

/// Inverse einer unteren Dreiecksmatrix
fn inv_lower(l: &Mat) -> Mat {
    let n = l.r;
    let mut x = Mat::zeros(n, n);
    for c in 0..n {
        for i in c..n {
            let mut s = if i == c { 1.0 } else { 0.0 };
            for k in c..i {
                s -= l[(i, k)] * x[(k, c)];
            }
            x[(i, c)] = s / l[(i, i)];
        }
    }
    x
}

/// Inverse einer oberen Dreiecksmatrix
fn inv_upper(r: &Mat) -> Mat {
    inv_lower(&r.t()).t()
}

/// Householder-QR von [A | l]: liefert R (n×n) und Qᵀl (erste n Komponenten)
fn householder(a: &Mat, l: &[f64]) -> Result<(Mat, Vec<f64>), SolveError> {
    let (m, n) = (a.r, a.c);
    if m < n {
        return Err(SolveError("weniger Beobachtungen als Unbekannte".into()));
    }
    let mut w = a.clone();
    let mut b = l.to_vec();
    let scale = (0..n).map(|j| (0..m).map(|i| w[(i, j)].abs()).fold(0.0, f64::max)).fold(0.0, f64::max).max(1e-300);
    for k in 0..n {
        let mut norm = 0.0;
        for i in k..m {
            norm += w[(i, k)] * w[(i, k)];
        }
        let norm = norm.sqrt();
        if norm <= 1e-13 * scale {
            return Err(SolveError(format!("QR: Spalte {} linear abhängig: Netz nicht ausreichend bestimmt / Datumsdefekt", k + 1)));
        }
        let alpha = if w[(k, k)] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = (k..m).map(|i| w[(i, k)]).collect();
        v[0] -= alpha;
        let vn: f64 = v.iter().map(|x| x * x).sum();
        if vn == 0.0 {
            continue;
        }
        for j in k..n {
            let s: f64 = (k..m).map(|i| v[i - k] * w[(i, j)]).sum::<f64>() * 2.0 / vn;
            for i in k..m {
                w[(i, j)] -= s * v[i - k];
            }
        }
        let s: f64 = (k..m).map(|i| v[i - k] * b[i]).sum::<f64>() * 2.0 / vn;
        for i in k..m {
            b[i] -= s * v[i - k];
        }
    }
    let mut r = Mat::zeros(n, n);
    for i in 0..n {
        for j in i..n {
            r[(i, j)] = w[(i, j)];
        }
    }
    b.truncate(n);
    Ok((r, b))
}

/// Einseitige Jacobi-SVD: A (m×n, m ≥ n) = U Σ Vᵀ; liefert (U·Σ als Spalten, Σ, V)
fn jacobi_svd(a: &Mat) -> (Mat, Vec<f64>, Mat) {
    let (m, n) = (a.r, a.c);
    let mut u = a.clone();
    let mut v = Mat::eye(n);
    for _sweep in 0..80 {
        let mut off = 0.0;
        for p in 0..n {
            for q in p + 1..n {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for i in 0..m {
                    let (up, uq) = (u[(i, p)], u[(i, q)]);
                    alpha += up * up;
                    beta += uq * uq;
                    gamma += up * uq;
                }
                if gamma == 0.0 || alpha == 0.0 || beta == 0.0 {
                    continue;
                }
                let c0 = gamma.abs() / (alpha * beta).sqrt();
                off = f64::max(off, c0);
                if c0 < 1e-15 {
                    continue;
                }
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let t = if zeta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for i in 0..m {
                    let (up, uq) = (u[(i, p)], u[(i, q)]);
                    u[(i, p)] = c * up - s * uq;
                    u[(i, q)] = s * up + c * uq;
                }
                for i in 0..n {
                    let (vp, vq) = (v[(i, p)], v[(i, q)]);
                    v[(i, p)] = c * vp - s * vq;
                    v[(i, q)] = s * vp + c * vq;
                }
            }
        }
        if off < 1e-15 {
            break;
        }
    }
    let sig: Vec<f64> = (0..n).map(|j| (0..m).map(|i| u[(i, j)] * u[(i, j)]).sum::<f64>().sqrt()).collect();
    (u, sig, v)
}

/// Löst  min |A dx − l|²  mit dem gewählten Verfahren.
pub fn solve(solver: Solver, a: &Mat, l: &[f64]) -> Result<Solution, SolveError> {
    let n = a.c;
    if n == 0 {
        return Ok(Solution { dx: vec![], qxx: Mat::zeros(0, 0), cond: None, rank: None });
    }
    if a.r < n {
        return Err(SolveError(format!("keine Überbestimmung: {} Beobachtungen für {} Unbekannte", a.r, n)));
    }
    match solver {
        Solver::GaussJordan => {
            let nm = a.ata();
            let q = invert_gauss_jordan(&nm)?;
            let dx = q.mulv(&a.atl(l));
            Ok(Solution { dx, qxx: q, cond: None, rank: None })
        }
        Solver::Cholesky => {
            let nm = a.ata();
            let lo = cholesky(&nm)?;
            let li = inv_lower(&lo);
            let q = li.t().mul(&li);
            let dx = q.mulv(&a.atl(l));
            Ok(Solution { dx, qxx: q, cond: None, rank: None })
        }
        Solver::QrHouseholder => {
            let (r, qtl) = householder(a, l)?;
            let ri = inv_upper(&r);
            let dx = ri.mulv(&qtl);
            let q = ri.mul(&ri.t());
            Ok(Solution { dx, qxx: q, cond: None, rank: None })
        }
        Solver::Svd => {
            let (us, sig, v) = jacobi_svd(a);
            let smax = sig.iter().cloned().fold(0.0, f64::max);
            let tol = smax * 1e-11 * (a.r.max(n) as f64);
            let rank = sig.iter().filter(|&&s| s > tol).count();
            if rank < n {
                return Err(SolveError(format!("SVD: Rangdefekt {} (Rang {} von {}): Netz nicht ausreichend bestimmt / Datumsdefekt", n - rank, rank, n)));
            }
            let smin = sig.iter().cloned().fold(f64::MAX, f64::min);
            // dx = V Σ⁻¹ Uᵀ l, mit U = us/σ
            let mut dx = vec![0.0; n];
            for j in 0..n {
                let s = sig[j];
                let ul: f64 = (0..a.r).map(|i| us[(i, j)] * l[i]).sum::<f64>() / (s * s);
                for i in 0..n {
                    dx[i] += v[(i, j)] * ul;
                }
            }
            let mut q = Mat::zeros(n, n);
            for i in 0..n {
                for k in 0..n {
                    let mut s = 0.0;
                    for j in 0..n {
                        s += v[(i, j)] * v[(k, j)] / (sig[j] * sig[j]);
                    }
                    q[(i, k)] = s;
                }
            }
            Ok(Solution { dx, qxx: q, cond: Some(smax / smin), rank: Some(rank) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_problem() -> (Mat, Vec<f64>) {
        let mut a = Mat::zeros(6, 3);
        let rows = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, -1.0, 0.0], [0.0, 1.0, -1.0], [1.0, 1.0, 1.0]];
        for (i, r) in rows.iter().enumerate() {
            for j in 0..3 {
                a[(i, j)] = r[j];
            }
        }
        (a, vec![1.0, 2.0, 3.1, -1.05, -0.98, 6.02])
    }

    #[test]
    fn all_solvers_agree() {
        let (a, l) = test_problem();
        let refs = solve(Solver::GaussJordan, &a, &l).unwrap();
        for s in Solver::ALL {
            let x = solve(s, &a, &l).unwrap();
            for i in 0..3 {
                assert!((x.dx[i] - refs.dx[i]).abs() < 1e-10, "{:?}", s);
                for j in 0..3 {
                    assert!((x.qxx[(i, j)] - refs.qxx[(i, j)]).abs() < 1e-10, "{:?}", s);
                }
            }
        }
    }

    #[test]
    fn singular_detected() {
        let mut a = Mat::zeros(3, 2);
        for i in 0..3 {
            a[(i, 0)] = 1.0;
            a[(i, 1)] = 1.0;
        }
        for s in Solver::ALL {
            assert!(solve(s, &a, &[1.0, 2.0, 3.0]).is_err(), "{:?}", s);
        }
    }
}
