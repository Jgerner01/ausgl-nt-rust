# AUSGL-NT

Geodätische Netzausgleichung nach vermittelnden Beobachtungen – Nachfolger von WAUSGL/AUSGL (Delphi, 1996–1998), neu in Rust mit eframe/egui.

* Lagenetz (5 Varianten wie im Original), Höhennetz (Nivellement / trig.), 2D-Lage + Höhe, volle 3D-Ausgleichung, räumliche Vorwärtsschnitte
* Lösungsverfahren: Gauß-Jordan (wie Original), Cholesky, QR (Householder), SVD
* robuste Schätzer zur Ausreißersuche: Huber, Dänische Methode, IGG-III, Tukey, L1
* Ausgaben: Protokoll als RTF, Netzbild als PDF und DXF, Neupunkte als *.KON
* Messdaten-Editor: Linie oder Punkt im Netzbild anklicken → rechts öffnet sich die Tabelle, die zugehörigen Zeilen sind markiert. Werte editieren, Beobachtungen einzeln (Richtung / Strecke / Höhe) ein- und ausschalten, Punkte als Fest- oder Neupunkt festlegen, als Netzdatei speichern
* Werkzeuge: Satzmittelbildung, räumliche Helmert-Transformation
* Import von CAPLAN-Datensätzen (*.N, Cremer): Punkte, Festpunkte, Richtungssätze, Strecken und Höhenunterschiede mit ihren Standardabweichungen (GNSS-Basislinien werden nicht ausgewertet)

Rechenregeln und Konstanten aus dem Original: siehe `MECHANICS.md`.

Anleitungen: [Kurzanleitung](docs/Kurzanleitung.md) und [Handbuch](docs/Handbuch.md) mit allen Formeln, Zahlenbeispielen und Professor-Modus.
Die HTML-Fassungen fürs Hilfe-Menü erzeugt `python tools/Doku-HTML.py` (benötigt `markdown` und `latex2mathml`).

## Netzdatei (*.NET)

Punkte und Messungen in einer ASCII-Datei, Felder durch Leerzeichen getrennt, `#` = Kommentar, `-` = kein Wert
bzw. Voreinstellung. Jede Zeile beginnt mit einem Kennbuchstaben, die Reihenfolge der Punkte ist beliebig.
Fehlerhafte Zeilen werden beim Einlesen übergangen und gemeldet, der Rest bleibt gültig.

```
# P  Punkt        Rechtswert       Hochwert      Höhe   Art  Code
P    G1        32689789.5810   5330413.9290   522.7310  F    GP
P    SP1       32689783.0570   5330550.6890          -  N    -
#    Art: F = Lage und Höhe fest, L = nur Lage fest, H = nur Höhe fest, N = Neupunkt (Näherung)

# S  Standpunkt  Instr.höhe  Bemerkung
S    SP1         1.520       Satz 1
# Z  Ziel   Hz[gon]     Strecke[m]  V[gon]/dH[m]  Zielh.  Code  Status  σHz[mgon]  σS[mm]  σV
Z    G1     321.94583   136.9148*   0.06042       0.000   GP    RSH     0.820      1.230   -
Z    G2     284.73635   173.4850*   0.17838       0.000   GP    -SH     -          -       -
#    Strecke mit * = Horizontalstrecke, dann steht unter V der Höhenunterschied
#    Status: R / S / H = Richtung / Strecke / Höhe verwendet, '-' an der Stelle = gestrichen

# N  Von   Nach   dH[m]     Strecke[m]  Status  σdH[mm]
N    SP1   G1     0.06042   136.92      H       2.306
```

```
cargo build --release
cargo test --release
```
