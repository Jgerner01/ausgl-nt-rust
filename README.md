# AUSGL-NT

Geodätische Netzausgleichung nach vermittelnden Beobachtungen – Nachfolger von WAUSGL/AUSGL (Delphi, 1996–1998), neu in Rust mit eframe/egui.

* Lagenetz (5 Varianten wie im Original), Höhennetz (Nivellement / trig.), 2D-Lage + Höhe, volle 3D-Ausgleichung, räumliche Vorwärtsschnitte
* Lösungsverfahren: Gauß-Jordan (wie Original), Cholesky, QR (Householder), SVD
* robuste Schätzer zur Ausreißersuche: Huber, Dänische Methode, IGG-III, Tukey, L1
* Ausgaben: Protokoll als RTF, Netzbild als PDF und DXF, Neupunkte als *.KON
* Werkzeuge: Satzmittelbildung, räumliche Helmert-Transformation

Rechenregeln und Konstanten aus dem Original: siehe `MECHANICS.md`.

```
cargo build --release
cargo test --release
```
