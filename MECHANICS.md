# AUSGL-NT – Rechenregeln aus dem Original (AUSGL.EXE, dekompiliert)

Alle Werte unten wurden in AUSGL.EXE (Delphi, 1998) mit Ghidra dekompiliert bzw. als Konstanten im
Maschinencode gelesen und mit dem mitgelieferten Pascal-Quelltext abgeglichen. Adressen = Funktion bzw.
Konstante im Programm.

## Funktionen
| Adresse | Inhalt | Pascal |
|---|---|---|
| `0x4519d8` | Lageausgleichung Teil 1: Variante, Startgewichte, Normalgleichungen, freies Netz | ULAGE `tlage.Ausgleichung1` |
| `0x44f9ac` | Verbesserungsgleichungen Lage, Gewichte je Strecke, Redundanz, Streichen | ULAGE `tlage.lauf` |
| `0x45e73c` | Verbesserungsgleichungen Höhe, Streichen > 0,5 m | UNIT7 `thoehe.lauf` |
| `0x45e158` | Höhenunterschied und Gewicht je Beobachtung (Niv / trig.) | UNIT7 `up1` |
| `0x45710c` | Netzbild mit Fehlerellipsen | UNIT6 `elli` |

## Lagenetz – Startgewichte (`0x4519d8`)
| Größe | Wert | Beleg |
|---|---|---|
| σ Richtung fr | 0,0005 gon = 0,5 mgon | double 0x3f40624dd2f1a9fc |
| σ Strecke konstant fs | 0,005 m | double 0x3f747ae147ae147b |
| σ Strecke proportional fs1 | 0,003 (Eingabe mm/km) → ×0,001 | double 0x3f689374bc6a7efa, Faktor `_DAT_00452638` |
| σ Altpunkt fk | 0,01 m | double 0x3f847ae147ae147b |
| Richtungsgewicht ger | 1,0 | double 0x3ff0000000000000 |
| Streckengewicht ges | fr² / fs² | Ausdruck in 0x4519d8 |
| Altpunktgewicht gk (Variante 3/4) | fr² / fk² | Ausdruck in 0x4519d8 |
| Unbekannte je Variante | 1: 2·NP · 2: 2·NP+1 · 3: 2·(NP+AP) · 4: 2·(NP+AP)+1 · 5: 2·(NP+AP) | `switch` in 0x4519d8 |
| freies Netz | Schwerpunkt aller Punkte, Bedingungen Verschiebung Y/X mit 1/n, Drehung mit 1/Σ(dy²+dx²); Freiheitsgrade + 3 | 0x4519d8 |

## Lagenetz – je Beobachtung (`0x44f9ac`)
| Größe | Wert | Beleg |
|---|---|---|
| Streckengewicht | fr² / (fs + 0,001·fs1·s)² | `_DAT_00451734` = 0,001 (extended) |
| Richtungsgewicht entfernungsabhängig | √(s / gew_dist) | Ausdruck in 0x44f9ac |
| Strecke vorhanden, wenn | s > 0,00001 m | `_DAT_00451728` = 1e-5 |
| Redundanz „##“ | r ≤ 0,03 (und r < 0,0002) | `_DAT_00451748` = 0,0002, `_DAT_00451758` = 0,03 |
| Redundanz „#“ | 0,03 < r ≤ 0,1 | `_DAT_00451764` = 0,1 |
| Anzeige vr | mgon (×1000), vs cm (×100) | float 1000.0 @0x451774, 100.0 @0x45177c |
| Strecke gestrichen | \|g0\| > fl1 (WAUSGL.SYS 0,300 m) | Text @0x451784 |
| Richtung gestrichen | \|f0\| > maxv = 10·fr1 (WAUSGL.SYS 0,0200 gon → 0,2 gon) | ULAGE `laden` |

## Höhennetz (`0x45e73c`, `0x45e158`)
| Größe | Wert | Beleg |
|---|---|---|
| Messung gestrichen | \|f0\| > 0,5 m | float 0.5 @0x45fa7c |
| Anzeige vh | mm (×1000) | float 1000.0 @0x45fa74 |
| Gewicht Nivellement | c0 / frh² / s (∝ 1/s) | 0x45e158 Zweig Modus 1 |
| Gewicht trig. Höhen | c0 / s² / frh² (∝ 1/s²) | 0x45e158 Zweig Modus 2 |
| trig. Höhenunterschied | dh = s / tan z + (i − t) + 68·s²·1e-9 | float 68.0 @0x45e72c, extended 1e-9 @0x45e730 |
| Zenit genau 100/300 gon | + 0,00001 gon | extended 1e-5 @0x45e720, float 3.0 @0x45e71c |
| Streckenreduktion (Höhe/GK) | dsh = (h1+h2)·0,5/rm·s, dsgk = ym²/(2·rm²)·s | float 0.5 @0x45e714, 2.0 @0x45e718 |
| σ Höhe frh / σ Altpunkt fk | 0,005 m / 0,005 m | UNIT7 `Ausgleichung1` |

## Netzbild / Fehlerellipsen (`0x45710c`)
| Größe | Wert | Beleg |
|---|---|---|
| Rand um das Netz | Diagonale / 8 | float 0.125 @0x458d7c |
| Abstand Beschriftung | Rand · 0,6 | extended 0.6 @0x458d80 |
| Beobachtungslinie | bis 80 % durchgezogen, Rest punktiert | extended 0.8 @0x458e08 |
| Strecke gemessen | Querstrich in der Mitte | extended 0.005 @0x458e18 (wx/200) |
| Ellipsenpunkte | 81 Schritte à 5 gon | Schleife bis 0x51 |
| Halbachsen | A,B² = s0²·(qyy+qxx ± √((qxx−qyy)²+4qxy²))·0,5 | float 0.5 @0x458e24, 2.0 @0x458e28 |
| Bezugsstrich | „10 cm Ellipsen“ wenn Maßstab > 5, sonst „1 cm“ | float 5.0 @0x458da0, 0.1 @0x458da4, 0.01 @0x458dc0 |
| Startmaßstab Ellipsen | 1:10 | Hilfe „Fehlerellipsen“ |

## Konstanten (OSTART.PAS; im EXE als Laufzeitwerte gesetzt)
rwi = 100 gon, ro = 2·rwi/π, rm = 6 380 000 m. Die Bytefolgen für 6 380 000 als double/extended/single
wurden im EXE gesucht und nicht gefunden (vermutlich zur Laufzeit aus Ganzzahl berechnet) – Wert aus dem Quelltext.

## So verwendet AUSGL-NT diese Werte
* Standard-Genauigkeiten: 0,5 mgon, 5 mm + 3 mm/km, Altpunkt 10 mm, Höhe 5 mm – wie 0x4519d8.
* Streckengewicht 1/(fs+fs1·s)², Richtungsgewicht √(s/gew_dist), Niv 1/s, trig. 1/s² – wie 0x44f9ac / 0x45e158.
* Streichen vor der Ausgleichung: Richtung 0,2 gon, Strecke 0,3 m, Höhe 0,5 m.
* Redundanzkennzeichen # / ## mit 0,1 / 0,03; Ausgabe in mgon und mm.
* Netzbild: Linien 80 % durchgezogen + Rest gestrichelt, Querstrich bei gemessener Strecke, Ellipsen mit 81×5 gon
  (auf dem Bildschirm 64 Punkte), Bezugsstrich 10 cm / 1 cm nach Maßstab.

## Neu (nicht im Original – eigene Festlegungen, „estimated“)
* volle 3D-Ausgleichung (Schrägstrecken + Zenitwinkel), Refraktion k = 0,1324 aus 68e-9 = (1−k)/(2R), σ Zenit 1 mgon
* Lösungsverfahren Cholesky, QR (Householder), SVD neben Gauß-Jordan (Original)
* robuste Schätzer: Huber 1,5 · Dänisch 2,0 · IGG-III 1,5 · Tukey 4,685 · L1; Streuung über MAD, mindestens 1
* Ausreißertest normierte Verbesserung > 3,29; Globaltest χ² 5 %
* Ellipsenmaßstab standardmäßig automatisch (Original-Start 1:10 auswählbar)
