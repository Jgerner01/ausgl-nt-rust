# AUSGL-NT – Handbuch

**Geodätische Netzausgleichung nach vermittelnden Beobachtungen**

Autor: Johann Gerner

> **Wichtig:** AUSGL-NT ist für die Ausbildung gedacht. Die verwendeten Algorithmen sind nicht vollständig
> getestet. Die Nutzung geschieht auf eigene Gefahr. Professionelle Projekte dürfen mit AUSGL-NT nur
> **nachgerechnet** werden – maßgebend ist die Berechnung mit einer geprüften Software.

Dieses Handbuch erklärt Schritt für Schritt, was das Programm rechnet und warum. Es ist für Studierende
geschrieben: Jede Formel wird in Worten erklärt und mit Zahlen aus einem echten Beispiel belegt.

Die grün-kursiv gesetzten Abschnitte **Professor-Modus** richten sich an Fortgeschrittene. Sie verwenden die
Fachbegriffe besonders genau und weisen auf Fallstricke der Verfahren hin. In der HTML-Fassung lassen sie sich
oben auf der Seite ein- und ausblenden.

Das durchgehende Zahlenbeispiel ist das **hybride Brückennetz** aus dem Seminar „Qualität von geodätischen
Netzen“ (Datei `Bsp3 - Brückennetz mit 6 SP.N`, 22 Punkte, 66 Richtungen, 66 Strecken, 66 Höhenunterschiede),
gerechnet als freies Netz.

---

## Inhalt

1. Grundbegriffe
2. Ablauf einer Netzausgleichung
3. Daten und Dateiformate
4. Das stochastische Modell: Genauigkeiten und Gewichte
5. Das funktionale Modell: Beobachtungsgleichungen
6. Näherungskoordinaten
7. Die Ausgleichung nach vermittelnden Beobachtungen
8. Varianten und geodätisches Datum
9. Beurteilung des Ergebnisses
10. Robuste Schätzung
11. Grenzwertprüfung vor der Ausgleichung
12. Werkzeuge: Satzmittel und Helmert-Transformation
13. Bedienung: Messdaten-Editor und Ausgaben
14. Quellen

---

## 1. Grundbegriffe

* **Beobachtung** – ein gemessener Wert, z. B. eine Richtung, eine Strecke oder ein Höhenunterschied.
  Bezeichnung $L_i$, Anzahl $n$.
* **Unbekannte** – die gesuchten Größen: Koordinaten der Neupunkte, Orientierungen der Standpunkte, evtl. ein
  Maßstab. Bezeichnung $X_j$, Anzahl $u$.
* **Redundanz** (Freiheitsgrad) – so viele Beobachtungen hat man „zu viel“: $f = n - u + d$
  ($d$ = Datumsdefekt, siehe Abschnitt 8). Nur mit $f > 0$ kann man ausgleichen und Fehler erkennen.
* **Festpunkt (Altpunkt)** – Koordinaten sind gegeben und werden nicht verändert.
  **Neupunkt** – Koordinaten werden bestimmt.
* **Winkelmaß gon** – der Vollkreis hat 400 gon, ein rechter Winkel 100 gon.
  1 mgon = 0,001 gon. Umrechnung: $\rho = \frac{200}{\pi}$ gon je Radiant.
* **Koordinatensystem** – Y = Rechtswert (Ost), X = Hochwert (Nord). Der **Richtungswinkel** $t$ wird von
  Nord im Uhrzeigersinn gezählt.

> **Professor-Modus.** Die Ausgleichung nach vermittelnden Beobachtungen ist das Gauß-Markov-Modell
> $E(\mathbf{L}) = f(\mathbf{X})$, $D(\mathbf{L}) = \sigma_0^2 \mathbf{Q}_{LL}$. AUSGL-NT setzt $\mathbf{Q}_{LL}$
> stets diagonal an, also unkorrelierte Beobachtungen. Das ist für Satzmittel aus mehreren Sätzen nur
> näherungsweise richtig: Richtungen eines Satzes sind über die gemeinsame Orientierung korreliert, und die
> Satzausgleichung erzeugt weitere Korrelationen. Wer Satzmittel einführt, unterschätzt daher tendenziell die
> Varianzen der Unbekannten.

---

## 2. Ablauf einer Netzausgleichung

1. **Daten einlesen** – Festpunkte und Messungen (Abschnitt 3).
2. **Grenzwertprüfung** – grobe Fehler vorab aussortieren (Abschnitt 11).
3. **Näherungskoordinaten** für alle Neupunkte bestimmen (Abschnitt 6).
4. **Ausgleichung** iterativ rechnen, bis sich die Koordinaten nicht mehr ändern (Abschnitt 7).
5. **Beurteilen** – Standardabweichung $s_0$, Globaltest, Ausreißertest, Fehlerellipsen (Abschnitt 9).
6. Bei Bedarf **Ausreißer streichen** oder **robust schätzen** (Abschnitt 10) und erneut rechnen.
7. **Ausgeben** – Protokoll (RTF), Netzbild (PDF/DXF), Neupunkte (KON).

Bei der Berechnungsart *2D-Lage + Höhenausgleichung* rechnet AUSGL-NT zwei getrennte Ausgleichungen
nacheinander: zuerst die Lage, dann mit den ausgeglichenen Lagekoordinaten die Höhen.

> **Professor-Modus.** Die getrennte Lage- und Höhenausgleichung ist ein Zweischrittverfahren: Die
> Kovarianzen zwischen Lage und Höhe werden vernachlässigt, und die Strecken für die Gewichte der
> Höhenunterschiede stammen aus der Lageausgleichung. Bei steilen Visuren (große Höhenunterschiede,
> kurze Strecken) ist die *volle 3D-Ausgleichung* mit Schrägstrecken und Zenitwinkeln das strengere Modell.

---

## 3. Daten und Dateiformate

Alle Dateien sind Textdateien. Felder werden durch Leerzeichen getrennt. Winkel stehen in gon, Strecken und
Koordinaten in Metern.

### 3.1 Netzdatei `*.NET` (empfohlen)

Punkte und Messungen stehen in einer Datei. Jede Zeile beginnt mit einem Kennbuchstaben:

| Kennung | Bedeutung | Felder |
|---|---|---|
| `P` | Punkt | Punkt, Rechtswert, Hochwert, Höhe, Art, Code |
| `S` | Standpunkt | Standpunkt, Instrumentenhöhe, Bemerkung |
| `Z` | Zielung (gehört zur letzten `S`-Zeile) | Ziel, Hz, Strecke, V bzw. dH, Zielhöhe, Code, Status, σHz, σS, σV |
| `N` | Höhenunterschied / Nivellement | Von, Nach, dH, Strecke, Status, σdH |

**Punktart:** `F` = Lage und Höhe fest, `L` = nur Lage fest, `H` = nur Höhe fest, `N` = Neupunkt.
Bei Neupunkten sind die Koordinaten **Näherungswerte**; `-` bedeutet „berechnen lassen“.

**Status:** drei Zeichen für Richtung, Strecke und Höhe: `RSH` = alles verwenden, `-SH` = Richtung
gestrichen, `---` = ganze Zielung gestrichen. So steht in der Datei selbst, was gestrichen ist; das bleibt
auch richtig, wenn man Zeilen einfügt oder löscht.

Auszug aus der Netzdatei des Brückennetzes (vom Programm aus dem CAPLAN-Datensatz erzeugt):

```text
# P Punkt             Rechtswert        Hochwert        Höhe  Art  Code
P G1             32689789.5810    5330413.9290    522.7310  N    -
P SP1            32689783.0570    5330550.6890    522.6650  N    -
#
# Z Ziel                 Hz      Strecke        V/dH  Zielh.  Code   St.      σHz      σS      σV
S SP1              0.000  Richtungssatz (CAPLAN R)
Z G2             284.73635           -            -   0.000  00     RSH    0.759       -       -
Z ZP1            123.33216           -            -   0.000  00     RSH    0.949       -       -
S SP1              0.000  Strecken (CAPLAN E)
Z G1                     -    136.9148*           -   0.000  00     RSH        -   1.230       -
#
# N Von          Nach                 dH    Strecke  St.      σdH
N SP1          G1               0.06042     136.92  H      2.306
```

### 3.2 Klassische Dateien: KOR, DAT, Nivellement

Die Formate des Vorgängers WAUSGL werden weiter gelesen.

```text
# Punktdatei *.KOR: Nummer  Rechtswert  Hochwert  [Höhe]  [Code]   – alle Punkte sind Festpunkte
1234   4565310.18   5312520.13   513.16   015

# Messdatei *.DAT (CADdy): -Standpunkt  i  [Bemerkung]   /   Ziel  Hz  Strecke  Zenit  Zielhöhe  Code
-125      1.520    PP
1251     125.1234    230.333     98.4567   1.600    GP5
1252     126.4567    190.444*    10.123    1.600    GP5

# Nivellement: -Standpunkt 0   /   Ziel  dH  Strecke
-2400   0
2456     -13.124     2350
```

Eine Strecke mit `*` ist eine Horizontalstrecke; dahinter steht dann der Höhenunterschied statt eines
Zenitwinkels.

### 3.3 CAPLAN-Datensätze `*.N`

Datensätze aus CAPLAN (Cremer Programmentwicklung) liest AUSGL-NT direkt ein
(*Datei ▸ CAPLAN-Datensatz importieren*). Ein Abschnitt beginnt mit einer Zeile, die nur den Kennbuchstaben in
Spalte 1 enthält:

| Abschnitt | Inhalt |
|---|---|
| Punktblock | Name, W, Rechtswert, Hochwert, Höhe |
| `T` | Titel (nächste Zeile) |
| `F` | Festpunkte: Name, Komponente (1 = Y, 2 = X, 3 = H) |
| `E` | Horizontalstrecken: Von, Nach, Strecke, σ [m] |
| `R` | Richtungssätze: Standpunktzeile, darunter Ziel, Richtung, σ [gon] |
| `H` | Höhenunterschiede: Von, Nach, dH, σ [m] |
| `B` | GNSS-Basislinien – werden **nicht** ausgewertet (Hinweis im Reiter *Daten*) |

Die Standardabweichungen aus der Datei werden je Beobachtung übernommen. Fehlt der Abschnitt `F`, rechnet
AUSGL-NT ein freies Netz.

> **Professor-Modus.** Groß- und Kleinbuchstaben der Abschnittskennung (`F`/`f`, `E`/`e`, …) behandelt
> AUSGL-NT gleich. Die genaue Bedeutung der Kleinschreibung in CAPLAN ist nicht dokumentiert; wer Datensätze
> mit gezielt deaktivierten Abschnitten austauscht, sollte das Ergebnis mit CAPLAN abgleichen. Beim
> Brückennetz fehlen ohne die Basislinien die Anschlüsse an G1–G4. Das Netz ist dann nur über ein freies
> Datum gelagert, und die Koordinaten sind mit denen einer Rechnung mit GNSS-Anschluss **nicht** vergleichbar
> (andere Lagerung). Vergleichbar bleiben datumsinvariante Größen: $s_0$, Redundanzanteile, Verbesserungen.

---

## 4. Das stochastische Modell: Genauigkeiten und Gewichte

Nicht jede Messung ist gleich genau. Genauere Messungen sollen das Ergebnis stärker bestimmen. Dazu bekommt
jede Beobachtung eine **Standardabweichung** $\sigma_i$ und daraus ein **Gewicht**

$$p_i = \frac{\sigma_0^2}{\sigma_i^2}, \qquad \sigma_0 = 1 .$$

$\sigma_0$ ist die *Standardabweichung der Gewichtseinheit a priori*; AUSGL-NT setzt sie auf 1. Hat eine
Beobachtung in der Netzdatei oder im CAPLAN-Datensatz eine eigene Standardabweichung, wird diese verwendet.
Sonst gelten die Voreinstellungen des Projekts:

| Beobachtung | Standardabweichung | Voreinstellung |
|---|---|---|
| Richtung | $\sigma_r$ konstant | 0,5 mgon |
| Strecke | $\sigma_s = a + b \cdot s_{\mathrm{km}}$ | $a$ = 5 mm, $b$ = 3 mm/km |
| Zenitwinkel | $\sigma_z$ konstant | 1,0 mgon |
| Nivellement | $\sigma_{\Delta h} = \sigma_{\mathrm{km}} \sqrt{s_{\mathrm{km}}}$ | 5 mm/√km |
| trig. Höhenunterschied | $\sigma_{\Delta h} = \sigma_{\mathrm{km}} \cdot s_{\mathrm{km}}$ | 10 mm/km |
| Festpunkt als Beobachtung | $\sigma_{\mathrm{fix}}$ | Lage 10 mm, Höhe 5 mm |

Beim Nivellement wächst die Standardabweichung mit der Wurzel der Strecke (Gewicht $\propto 1/s$), beim
trigonometrischen Höhenunterschied linear mit der Strecke (Gewicht $\propto 1/s^2$) – so wie im Original WAUSGL.

**Zahlenbeispiele**

* Strecke $s$ = 250 m: $\sigma_s = 5 + 3 \cdot 0{,}25 = 5{,}75$ mm.
* Nivellementslinie über 400 m: $\sigma_{\Delta h} = 5 \cdot \sqrt{0{,}4} = 3{,}16$ mm.
* Trigonometrischer Höhenunterschied über 300 m: $\sigma_{\Delta h} = 10 \cdot 0{,}3 = 3{,}0$ mm.

Optional kann das Richtungsgewicht von der Zielweite abhängen (Einstellung *Richtungsgewicht entfernungs­abhängig*,
Bezugsstrecke $s_g$): $p_r \propto \sqrt{s / s_g}$, also $\sigma_r' = \sigma_r \,(s_g/s)^{1/4}$.

> **Professor-Modus.** Die Gewichte sind nur *relativ* bedeutsam; über $\sigma_0^2$ a priori wird lediglich der
> Maßstab der Kofaktormatrix festgelegt. Erst der Vergleich mit $s_0$ a posteriori (Abschnitt 9) prüft, ob das
> stochastische Modell stimmt. Ein Fallstrick ist das Mischen unterschiedlicher Beobachtungsarten: Sind etwa
> die Richtungen zu optimistisch und die Strecken zu pessimistisch angesetzt, kann $s_0 \approx 1$ trotzdem
> herauskommen, obwohl beide Gruppen falsch gewichtet sind. Abhilfe ist eine Varianzkomponentenschätzung je
> Beobachtungsgruppe (nicht in AUSGL-NT enthalten) oder zumindest der Blick auf die normierten Verbesserungen
> getrennt nach Beobachtungsart. Die Wurzelabhängigkeit $\sigma_r \propto s^{-1/4}$ ist eine pragmatische Regel
> des Originals, kein strenges Modell der Zentrierfehler; streng wäre $\sigma_r^2 = \sigma_{r,0}^2 + (e/s)^2\rho^2$
> mit der Zentrierunsicherheit $e$.

---

## 5. Das funktionale Modell: Beobachtungsgleichungen

Für jede Beobachtung braucht man eine Formel, die sie aus den Unbekannten berechnet. Mit den Koordinaten
von Standpunkt $i$ und Zielpunkt $j$ gilt

$$\Delta y = y_j - y_i, \qquad \Delta x = x_j - x_i, \qquad s_{ij} = \sqrt{\Delta y^2 + \Delta x^2}, \qquad t_{ij} = \arctan\frac{\Delta y}{\Delta x} .$$

Der Richtungswinkel $t_{ij}$ wird quadrantenrichtig über die zweiargumentige Arkustangensfunktion bestimmt
und auf 0 … 400 gon gebracht.

### 5.1 Richtung

Ein Tachymeter misst keine Richtungswinkel, sondern Richtungen $r$ gegen seine willkürliche Nullrichtung.
Der Unterschied ist die **Orientierung** $o_i$ des Standpunkts, eine zusätzliche Unbekannte je Satz:

$$r_{ij} = t_{ij} - o_i .$$

### 5.2 Horizontalstrecke

$$s_{ij} = \sqrt{\Delta y^2 + \Delta x^2}\,(1 + \mu \cdot 10^{-6}) ,$$

$\mu$ ist ein Maßstabsfaktor in ppm. Er wird nur in den Varianten mit Maßstab (2 und 4) mitgeschätzt, sonst ist
$\mu = 0$.

### 5.3 Schrägstrecke und Zenitwinkel (3D)

Mit Instrumentenhöhe $i$, Zielhöhe $t$, Erdradius $R$ = 6 380 000 m und Refraktionskoeffizient $k$:

$$\Delta z = (H_j + t) - (H_i + i) - (1-k)\,\frac{s^2}{2R}$$

$$z_{ij} = \arctan\frac{s}{\Delta z}, \qquad S_{ij} = \sqrt{s^2 + \Delta z_0^2}\,(1+\mu\cdot 10^{-6}) .$$

Für die Schrägstrecke verwendet AUSGL-NT $\Delta z_0$ mit der Erdkrümmung $s^2/(2R)$ allein, ohne Refraktion.
Der Refraktionskoeffizient ist fest $k$ = 0,1324. Er steckt im Original WAUSGL in der Konstanten
$68\cdot 10^{-9} = (1-k)/(2R)$.

### 5.4 Höhenunterschied

Nivellement und gegebene Höhenunterschiede: $\Delta h_{ij} = H_j - H_i$.

Aus einer Tachymetermessung mit Schrägstrecke $S$ und Zenitwinkel $z$ entsteht der Höhenunterschied
Boden–Boden so:

$$s = |S \sin z|, \qquad \Delta h = \frac{s}{\tan z} + 68\cdot 10^{-9}\, s^2 + i - t .$$

**Zahlenbeispiel:** $S$ = 150,000 m, $z$ = 98,0000 gon, $i$ = 1,550 m, $t$ = 1,300 m.
$\sin z = 0{,}999507$, also $s = 149{,}926$ m; $\tan z = 31{,}8205$, also $s/\tan z = 4{,}7116$ m;
Erdkrümmung und Refraktion $68\cdot10^{-9}\cdot 149{,}926^2 = 0{,}0015$ m.
Ergebnis: $\Delta h = 4{,}7116 + 0{,}0015 + 1{,}550 - 1{,}300 = 4{,}963$ m.

### 5.5 Streckenreduktion (optional)

Mit der Einstellung *Strecken reduzieren* werden Strecken auf die Abbildungsebene reduziert, mit mittlerer
Höhe $H_m$ und Abstand vom Mittelmeridian $y_m$:

$$s_{\mathrm{red}} = s - \frac{H_m}{R}\, s + \frac{y_m^2}{2R^2}\, s .$$

> **Professor-Modus.** Die Ableitungen der Beobachtungsgleichungen nach den Unbekannten (Designmatrix
> $\mathbf{A}$) bildet AUSGL-NT *numerisch* mit zentralen Differenzen und der Schrittweite $h = 10^{-4}$ m:
> $\partial f/\partial X \approx [f(X+h) - f(X-h)]/(2h)$. Der Abbruchfehler ist von der Ordnung $h^2 f'''$
> und bei geodätischen Netzen vernachlässigbar. Bei sehr kurzen Visuren (wenige Dezimeter, etwa
> Exzentrizitäten) wird die Krümmung der Richtungsfunktion aber spürbar; dann ist die analytische Ableitung
> $\partial t/\partial y_j = \rho\,\Delta x/s^2$, $\partial t/\partial x_j = -\rho\,\Delta y/s^2$ vorzuziehen.
> Weitere Fallstricke: Der Refraktionskoeffizient ist fest, wird also nicht geschätzt; bei bodennahen Visuren
> schwankt $k$ real zwischen etwa −1 und +1, die Zenitwinkel sind dann systematisch verfälscht. Bei der
> Schrägstrecke wird die Refraktion ganz vernachlässigt, beim Zenitwinkel nicht – das ist konsistent nur,
> solange die Bahnkrümmung des Strahls die Länge praktisch nicht ändert, was bei geodätischen Weiten zutrifft.

---

## 6. Näherungskoordinaten

Die Beobachtungsgleichungen sind nicht linear. Die Ausgleichung braucht deshalb für jeden Neupunkt
**Näherungskoordinaten**, von denen aus sie iterativ verbessert. AUSGL-NT bestimmt sie automatisch, in dieser
Reihenfolge und so oft hintereinander, bis kein weiterer Punkt dazukommt:

1. **Orientierung** eines Standpunkts aus bekannten Zielen: $o = t - r$, gemittelt; Werte, die mehr als
   0,1 gon vom Median abweichen, werden verworfen.
2. **Polarpunkt** von einem orientierten Standpunkt: $y = y_S + s\,\sin(r+o)$, $x = x_S + s\,\cos(r+o)$.
3. **Freie Stationierung**: unbekannter Standpunkt mit mindestens zwei bekannten Zielen (Richtung und
   Strecke) – ebene Ähnlichkeitstransformation vom örtlichen in das Landessystem.
4. **Vorwärtseinschnitt** aus zwei orientierten Strahlen.
5. **Bogenschnitt** aus zwei Strecken von bekannten Standpunkten.
6. **Rückwärtseinschnitt** aus mindestens drei Richtungen zu bekannten Punkten.

Höhen werden über Höhenunterschiede (trigonometrisch oder Nivellement) von bekannten Punkten aus fortgepflanzt.
Welche Bestimmung für welchen Punkt verwendet wurde, steht im Reiter *Daten ▸ Näherungswerte*.

In einer Netzdatei oder einem CAPLAN-Datensatz stehen die Näherungskoordinaten meist schon in der Datei; sie
werden dann direkt verwendet.

> **Professor-Modus.** Die Konvergenz des Gauß-Newton-Verfahrens ist nur lokal garantiert. Schlechte
> Näherungen bei schleifenden Schnitten (Vorwärtseinschnitt mit Schnittwinkeln unter etwa 20 gon,
> Rückwärtseinschnitt nahe dem gefährlichen Kreis) können zu Divergenz oder zur Konvergenz gegen eine falsche
> Lösung führen – beim Bogenschnitt etwa gegen den gespiegelten Punkt. AUSGL-NT löst die Zweideutigkeit des
> Bogenschnitts über eine dritte Strecke oder den Drehsinn der Richtungen; fehlt beides, ist das Ergebnis zu
> prüfen. Der Rückwärtseinschnitt wird intern von mehreren Startwerten aus gerechnet, um dem gefährlichen
> Kreis auszuweichen; liegen alle Anschlusspunkte nahezu auf einem Kreis mit dem Standpunkt, bleibt die Lösung
> trotzdem schlecht bestimmt (große Ellipse, kleine Redundanzanteile).

---

## 7. Die Ausgleichung nach vermittelnden Beobachtungen

### 7.1 Linearisierung

Man setzt die Unbekannten als Näherung plus kleinen Zuschlag an: $\mathbf{X} = \mathbf{X}_0 + \delta\mathbf{x}$.
Die Taylor-Entwicklung bis zum linearen Glied ergibt die **Verbesserungsgleichungen**

$$\mathbf{v} = \mathbf{A}\,\delta\mathbf{x} - \mathbf{l}, \qquad \mathbf{l} = \mathbf{L} - f(\mathbf{X}_0) .$$

* $\mathbf{A}$ ($n \times u$) ist die **Designmatrix**: die Ableitungen der Beobachtungen nach den Unbekannten.
* $\mathbf{l}$ ist der **gekürzte Beobachtungsvektor** („gemessen minus gerechnet“).
* $\mathbf{v}$ sind die **Verbesserungen**: so viel muss man an jeder Messung ändern, damit alles widerspruchsfrei passt.

Bei Winkeln wird die Differenz in $\mathbf{l}$ auf −200 … +200 gon gebracht, damit z. B. 399,9 gon und 0,1 gon
als 0,2 gon Unterschied erkannt werden.

### 7.2 Methode der kleinsten Quadrate

Gesucht ist die Lösung, bei der die gewichtete Quadratsumme der Verbesserungen am kleinsten ist:

$$\Omega = \mathbf{v}^T \mathbf{P}\, \mathbf{v} = \sum_i p_i\, v_i^2 \;\rightarrow\; \min .$$

Daraus folgen die **Normalgleichungen**

$$\mathbf{N}\,\delta\mathbf{x} = \mathbf{n}, \qquad \mathbf{N} = \mathbf{A}^T\mathbf{P}\mathbf{A}, \qquad \mathbf{n} = \mathbf{A}^T\mathbf{P}\,\mathbf{l}$$

mit der Lösung und der **Kofaktormatrix der Unbekannten**

$$\delta\hat{\mathbf{x}} = \mathbf{N}^{-1}\mathbf{n}, \qquad \mathbf{Q}_{xx} = \mathbf{N}^{-1} .$$

### 7.3 Iteration (Gauß-Newton)

Weil nur linearisiert wurde, rechnet AUSGL-NT mehrmals: Zuschläge anbringen, $\mathbf{A}$ und $\mathbf{l}$ neu
bilden, wieder lösen. Abgebrochen wird, wenn der größte Zuschlag kleiner als $10^{-5}$ m ist (Orientierungen
umgerechnet) oder nach höchstens 15 Iterationen (einstellbar). Beim Brückennetz genügten **2 Iterationen**.

### 7.4 Lösungsverfahren

Alle vier Verfahren lösen dieselbe Aufgabe $\min \|\mathbf{P}^{1/2}(\mathbf{A}\,\delta\mathbf{x} - \mathbf{l})\|^2$
und liefern dasselbe Ergebnis; sie unterscheiden sich in Rechenweg und numerischer Stabilität.

| Verfahren | Prinzip |
|---|---|
| Gauß-Jordan (wie Original) | $\mathbf{N}$ bilden und durch Austauschschritte invertieren |
| Cholesky | $\mathbf{N} = \mathbf{L}\mathbf{L}^T$, Vorwärts- und Rückwärtseinsetzen |
| QR (Householder) | $\mathbf{P}^{1/2}\mathbf{A} = \mathbf{Q}\mathbf{R}$ direkt, ohne Normalgleichungen |
| SVD (Jacobi) | $\mathbf{P}^{1/2}\mathbf{A} = \mathbf{U}\boldsymbol{\Sigma}\mathbf{V}^T$; zeigt Rangdefekte und die Konditionszahl |

> **Professor-Modus.** Die Konditionszahl der Normalgleichungsmatrix ist das Quadrat der Konditionszahl von
> $\mathbf{P}^{1/2}\mathbf{A}$: $\kappa(\mathbf{N}) = \kappa(\mathbf{A})^2$. Bei Großkoordinaten (UTM-Rechtswerte
> mit 32 689 789 m) und gemischten Einheiten (gon, m, ppm) kann $\kappa(\mathbf{N})$ in die Größenordnung
> $10^{12}$ und mehr kommen, sodass Gauß-Jordan und Cholesky in doppelter Genauigkeit spürbar Stellen verlieren.
> QR und SVD arbeiten mit $\kappa(\mathbf{A})$ und sind hier robuster; die SVD zeigt zudem verschwindende
> Singulärwerte, also Datums- oder Konfigurationsdefekte. Ein Fallstrick der Konvergenzschranke ist die
> Umrechnung der Orientierungszuschläge: Sie werden über ein Ersatzmaß mit Strecken verglichen. Bei sehr
> großen Netzen kann eine Orientierung daher noch messbar im mgon-Bereich wandern, während das
> Abbruchkriterium schon erfüllt ist. Das Gauß-Newton-Verfahren konvergiert bei kleinen Residuen quadratisch;
> bei groben Fehlern kann es oszillieren – deshalb vorher die Grenzwertprüfung.

---

## 8. Varianten und geodätisches Datum

Ein Netz aus Richtungen und Strecken legt nur die **Form** fest, nicht die **Lage** im Koordinatensystem.
Wo das Netz liegt, bestimmt das **Datum**. AUSGL-NT bietet fünf Varianten (wie WAUSGL):

| Variante | Datum | Unbekannte |
|---|---|---|
| 1. Normale Ausgleichung | Festpunkte fest | $2\,n_{NP}$ (+ Orientierungen) |
| 2. mit Maßstab | wie 1, zusätzlich Maßstab $\mu$ | $2\,n_{NP} + 1$ |
| 3. Festpunkte als Beobachtungen | Festpunkte mit $\sigma_{\mathrm{fix}}$ beweglich | $2\,(n_{NP}+n_{AP})$ |
| 4. wie 3, mit Maßstab | | $2\,(n_{NP}+n_{AP}) + 1$ |
| 5. Freie Netzausgleichung | Schwerpunkt-Lagerung über alle Punkte | $2\,(n_{NP}+n_{AP})$ |

### Freies Netz

Ohne Festpunkte ist $\mathbf{N}$ singulär. Der **Datumsdefekt** $d$ ist die Zahl der fehlenden Festlegungen:

* Lagenetz mit Strecken: $d = 3$ (zwei Verschiebungen, eine Drehung),
* Lagenetz nur mit Richtungen: $d = 4$ (zusätzlich der Maßstab),
* Höhennetz: $d = 1$ (eine Höhenverschiebung).

AUSGL-NT stellt die Matrix $\mathbf{G}$ der Datumsparameter auf, z. B. für einen Lagepunkt $k$ mit
Schwerpunkt $(\bar y, \bar x)$:

$$\mathbf{G}_k^T = \begin{pmatrix} 1 & 0 \\ 0 & 1 \\ x_k - \bar x & -(y_k - \bar y) \end{pmatrix}$$

(Zeilen: Verschiebung Y, Verschiebung X, Drehung). Gelöst wird mit den Bedingungen
$\mathbf{G}^T\delta\mathbf{x} = \mathbf{0}$: Die Koordinatenänderungen haben im Schwerpunkt keine Verschiebung
und keine Drehung. Die Kofaktormatrix der freien Lösung ist

$$\mathbf{Q}_{xx} = \left(\mathbf{N} + \mathbf{B}\mathbf{B}^T\right)^{-1} - \mathbf{G}\left(\mathbf{G}^T\mathbf{B}\mathbf{B}^T\mathbf{G}\right)^{-1}\mathbf{G}^T ,$$

wobei $\mathbf{B}$ die auf die Koordinaten beschränkte, normierte Form von $\mathbf{G}$ ist. Die Redundanz wird
um den Defekt erhöht: $f = n - u + d$.

**Brückennetz:** Lage: 132 Beobachtungen, 50 Unbekannte (22 Punkte × 2 + 6 Orientierungen), Datumsdefekt 3,
Redundanz $f = 132 - 50 + 3 = 85$.

> **Professor-Modus.** Die verwendeten Bedingungen entsprechen der Gesamtspurminimierung
> $\mathrm{sp}(\mathbf{Q}_{xx}) \rightarrow \min$ über alle Koordinaten (Pseudoinverse bezüglich der
> Koordinaten, die Orientierungen tragen nur die Drehung mit). Alle datumsabhängigen Größen –
> Koordinaten, Standardabweichungen, Fehlerellipsen – gelten nur für **diese** Lagerung; eine andere Wahl der
> Datumspunkte (Teilspurminimierung) liefert andere, gleich richtige Werte, die über eine S-Transformation
> ineinander überführbar sind. Datumsinvariant sind dagegen $\mathbf{v}$, $s_0$, die Redundanzanteile und
> geschätzte Strecken und Winkel. Ein häufiger Fehler ist es, die Ellipsen eines freien Netzes als „absolute“
> Punktgenauigkeit zu interpretieren. Fehlen Strecken ganz, nimmt AUSGL-NT den Maßstab automatisch ins Datum
> auf ($d = 4$); dann ist auch der Maßstab nur durch die Lagerung festgelegt. Bei Variante 3 und 4 hängen die
> Ergebnisse empfindlich von $\sigma_{\mathrm{fix}}$ ab: Ist es zu klein, erzwingt man Spannungen aus den
> Festpunkten ins Netz.

---

## 9. Beurteilung des Ergebnisses

### 9.1 Standardabweichung der Gewichtseinheit $s_0$

$$s_0 = \sqrt{\frac{\mathbf{v}^T\mathbf{P}\mathbf{v}}{f}}$$

Ist das stochastische Modell richtig, liegt $s_0$ nahe 1. Deutlich größer heißt: Die Messungen sind
ungenauer als angenommen oder es gibt grobe Fehler. Deutlich kleiner heißt: Die Genauigkeiten sind zu
pessimistisch angesetzt.

**Brückennetz (Lage):** $\mathbf{v}^T\mathbf{P}\mathbf{v}$ = 66,1669, $f$ = 85, also
$s_0 = \sqrt{66{,}1669/85} = 0{,}882$.

### 9.2 Globaltest

Der Globaltest prüft, ob $\mathbf{v}^T\mathbf{P}\mathbf{v}$ zu einer $\chi^2$-Verteilung mit $f$
Freiheitsgraden passt (zweiseitig, Irrtumswahrscheinlichkeit 5 %). Die Grenzen berechnet AUSGL-NT mit der
Näherung von Wilson und Hilferty:

$$\chi^2_{f,\,q} \approx f \left(1 - \frac{2}{9f} + z_q\sqrt{\frac{2}{9f}}\right)^3, \qquad z_{0{,}975} = 1{,}96 .$$

**Brückennetz:** $f$ = 85 ergibt die Grenzen 61,4 und 112,4. Weil 66,17 dazwischen liegt, ist der Globaltest
**bestanden**.

### 9.3 Redundanzanteile – wie gut ist eine Messung kontrolliert?

$$r_i = \left(\mathbf{Q}_{vv}\mathbf{P}\right)_{ii} = 1 - p_i\,\mathbf{a}_i\,\mathbf{Q}_{xx}\,\mathbf{a}_i^T, \qquad \sum_i r_i = f .$$

$r_i$ liegt zwischen 0 und 1. Bei $r_i$ = 0 wird ein Fehler dieser Messung gar nicht sichtbar (nicht
kontrolliert), bei $r_i$ = 1 vollständig. Das Protokoll markiert schlecht kontrollierte Beobachtungen:
`#` bei $r_i \le 0{,}1$, `##` bei $r_i \le 0{,}03$ (Grenzen wie im Original).

### 9.4 Normierte Verbesserung – Ausreißertest nach Baarda

$$NV_i = \frac{|v_i|}{\sigma_i\sqrt{r_i}}$$

Ist $NV_i > 3{,}29$, gilt die Beobachtung als **Ausreißer** (Irrtumswahrscheinlichkeit 0,1 %). Der
**geschätzte grobe Fehler** ist

$$GF_i = -\frac{v_i}{r_i} .$$

**Zahlenbeispiel Brückennetz:** Richtung SP5 → ZP2, $v$ = 2,69 mgon, $\sigma$ = 0,954 mgon, $r$ = 0,65:

$$NV = \frac{2{,}69}{0{,}954\cdot\sqrt{0{,}65}} = \frac{2{,}69}{0{,}769} = 3{,}50 > 3{,}29 \;\Rightarrow\; \text{Ausreißer}, \qquad GF = -\frac{2{,}69}{0{,}65} \approx -4{,}1\ \mathrm{mgon} .$$

Ebenso die Strecke SP1 → ZP2: $v$ = −3,49 mm, $\sigma$ = 1,23 mm, $r$ = 0,62 ergibt $NV$ = 3,60 und
$GF$ = +5,6 mm.

> **Professor-Modus.** Der Test verwendet $\sigma_0$ **a priori** ($w$-Test nach Baarda). Ist $s_0$ deutlich
> von 1 verschieden, wäre der $\tau$-Test nach Pope mit $s_0$ a posteriori angemessener. Der Test ist streng nur
> für **einen** groben Fehler gültig (Einzelfehlerhypothese); bei mehreren Fehlern verschmiert die
> Ausgleichung sie über benachbarte Beobachtungen (*Verschmierungseffekt*), sodass auch korrekte Messungen
> auffällig werden können. Man streicht deshalb immer nur die **größte** normierte Verbesserung und rechnet
> neu (*Data Snooping*). Der kritische Wert 3,29 gehört zu $\alpha_0$ = 0,1 % zweiseitig; im
> B-Methode-Konzept nach Baarda wird er zusammen mit der Testgüte $\beta_0$ = 80 % gewählt, woraus der
> Nichtzentralitätsparameter $\delta_0 = 4{,}13$ und der kleinste aufdeckbare Fehler
> $\nabla_0 L_i = \delta_0\,\sigma_i/\sqrt{r_i}$ folgen (innere Zuverlässigkeit; von AUSGL-NT nicht ausgegeben).
> Bei $r_i < 0{,}1$ ist $GF_i$ unzuverlässig, weil kleine Redundanzanteile im Nenner stehen.

### 9.5 Fehlerellipsen

Aus der $2\times 2$-Untermatrix der Kofaktoren eines Punktes ($q_{yy}$, $q_{xx}$, $q_{xy}$) folgen

$$s_y = s_0\sqrt{q_{yy}}, \qquad s_x = s_0\sqrt{q_{xx}}$$

$$w = \sqrt{(q_{xx}-q_{yy})^2 + 4q_{xy}^2}, \qquad A = s_0\sqrt{\frac{q_{yy}+q_{xx}+w}{2}}, \qquad B = s_0\sqrt{\frac{q_{yy}+q_{xx}-w}{2}}$$

$$\theta = \frac{1}{2}\arctan\frac{2q_{xy}}{q_{xx}-q_{yy}}$$

$A$ und $B$ sind die große und kleine Halbachse der **Standardellipse**, $\theta$ der Richtungswinkel der großen
Halbachse. Mit der Einstellung *Fehlerellipsen mit s0 a posteriori* (Voreinstellung) wird $s_0$ aus der
Ausgleichung verwendet, sonst 1.

**Brückennetz, Punkt G2:** $s_y$ = 1,4 mm, $s_x$ = 0,9 mm, $A$ = 1,5 mm, $B$ = 0,7 mm, $\theta$ = 73,5 gon.
Probe: $A^2 + B^2 = s_y^2 + s_x^2$ (2,74 ≈ 2,77, Rundung).

> **Professor-Modus.** Die Standardellipse enthält die wahre Punktlage nur mit etwa 39 % Wahrscheinlichkeit
> (zweidimensionale Normalverteilung, $\chi^2_2$). Die Option *95 % Konfidenz* im Netzbild vergrößert die
> Achsen mit dem Faktor $\sqrt{\chi^2_{2;0{,}95}} = 2{,}4477$. Streng genommen ist bei $s_0$ a posteriori die
> $F$-Verteilung $\sqrt{2F_{2,f;0{,}95}}$ statt $\chi^2$ zu verwenden; bei großer Redundanz ist der Unterschied
> gering. Fehlerellipsen sind datumsabhängig (Abschnitt 8); für Aussagen über die Netzgeometrie eignen sich
> relative Ellipsen zwischen Punktpaaren besser (nicht in AUSGL-NT enthalten).

---

## 10. Robuste Schätzung

Die Methode der kleinsten Quadrate reagiert empfindlich auf grobe Fehler. **Robuste Schätzer** geben
verdächtigen Beobachtungen schrittweise weniger Gewicht. AUSGL-NT rechnet dazu eine **iterativ neu gewichtete
Ausgleichung** (IRLS):

1. normal ausgleichen,
2. für jede Beobachtung die standardisierte Verbesserung $u_i = v_i / (\sigma_i\sqrt{r_i})$ bilden,
3. die Streuung robust schätzen: $\hat\sigma_u = \max\bigl(1{,}4826 \cdot \mathrm{median}|u_i|,\; 1\bigr)$,
4. neue Gewichtsfaktoren $w_i = w(u_i/\hat\sigma_u)$ berechnen: $p_i' = w_i\, p_i$,
5. erneut ausgleichen, bis sich kein $w_i$ mehr als 0,002 ändert (höchstens 40 Durchläufe).

| Schätzer | Gewichtsfunktion $w(u)$ für $\|u\| > k$ (sonst 1) | Konstante $k$ |
|---|---|---|
| Huber | $k / \|u\|$ | 1,5 |
| Dänische Methode (Krarup) | $\exp\bigl(1 - (u/k)^2\bigr)$ | 2,0 |
| IGG-III (Yang) | $\dfrac{k}{\|u\|}\left(\dfrac{3k - \|u\|}{3k - k}\right)^2$ für $\|u\| \le 3k$, sonst 0 | 1,5 |
| Tukey (Biweight) | 0 (für $\|u\| < k$: $(1-(u/k)^2)^2$) | 4,685 |
| L1-Norm | $1 / \max(\|u\|, 0{,}05)$ (für alle $u$) | – |

Im Beispielprojekt (*Datei ▸ Beispielprojekt laden*) ist ein grober Richtungsfehler eingebaut. Alle fünf
Schätzer finden ihn und geben ihm das kleinste Gewicht.

> **Professor-Modus.** Huber und L1 sind konvexe M-Schätzer mit eindeutiger Lösung; die Dänische Methode,
> IGG-III und Tukey sind *redescending* – sie können grobe Fehler ganz ausschalten, haben aber mehrere lokale
> Minima. Ihr Ergebnis hängt vom Startwert ab, hier der L2-Lösung, die ein großer Fehler bereits verzerrt hat.
> Die Standardisierung mit $\sqrt{r_i}$ ist wichtig: Ohne sie würden Beobachtungen mit kleinem
> Redundanzanteil, deren Fehler sich kaum in $v_i$ zeigt, nie abgewertet. Gegen schlecht kontrollierte
> Beobachtungen ($r_i \to 0$, Hebelpunkte) ist aber **kein** robuster Schätzer wirksam – der Bruchpunkt
> von M-Schätzern in der Regression ist in Bezug auf Hebelbeobachtungen null. Die Untergrenze 1 für
> $\hat\sigma_u$ verhindert, dass in kleinen, nahezu fehlerfreien Netzen reguläre Messungen abgewertet werden;
> sie bewirkt aber auch, dass bei insgesamt zu optimistischem stochastischem Modell ($s_0 \gg 1$) eher zu viele
> Beobachtungen abgewertet werden. Robuste Gewichte sind kein Ersatz für die Ursachenforschung: Abgewertete
> Messungen sollten im Feldbuch geprüft und dann bewusst gestrichen werden.

---

## 11. Grenzwertprüfung vor der Ausgleichung

Vor der ersten Iteration vergleicht AUSGL-NT jede Beobachtung mit dem aus den Näherungskoordinaten
gerechneten Wert. Ist der Widerspruch größer als der Grenzwert, wird die Beobachtung gestrichen und im
Protokoll genannt (wie im Original WAUSGL):

| Beobachtung | Grenzwert (Voreinstellung) |
|---|---|
| Richtung, Zenitwinkel | 0,2 gon |
| Strecke | 0,3 m |
| Höhenunterschied | 0,5 m |

> **Professor-Modus.** Die Prüfung hängt von der Güte der Näherungskoordinaten ab. Wurde ein Neupunkt
> selbst aus einer fehlerhaften Messung bestimmt (z. B. polar mit falscher Strecke), sind die übrigen
> Messungen zu diesem Punkt „widersprüchlich“ und werden zu Unrecht gestrichen, während die eigentlich
> falsche Messung bleibt. Bei unerwartet vielen Streichungen daher die Näherungsbestimmung im Reiter *Daten*
> prüfen oder die Grenzwertprüfung abschalten und robust schätzen.

---

## 12. Werkzeuge

### 12.1 Satzmittel

*Tools ▸ Satzmittelbildung* mittelt mehrere Sätze desselben Standpunkts. Innerhalb eines Satzes werden die
beiden Fernrohrlagen gemittelt (Lage II erkennt das Programm an $z > 200$ gon):

$$Hz = Hz_I + \frac{(Hz_{II} - 200) - Hz_I}{2}, \qquad z = \frac{z_I + 400 - z_{II}}{2}, \qquad \text{Indexfehler} = \frac{z_I + z_{II} - 400}{2} .$$

Danach wird jeder Satz um seine mittlere Abweichung gegenüber dem ersten Satz verschoben und das Mittel
gebildet (Satzausgleichung).

### 12.2 Räumliche Helmert-Transformation

*Tools ▸ Transformation* bestimmt aus mindestens drei identischen Punkten die Transformation

$$\mathbf{x}_B = \mathbf{c} + m\,\mathbf{R}\,\mathbf{x}_A$$

mit Verschiebung $\mathbf{c}$, Drehmatrix $\mathbf{R}$ und – wahlweise – Maßstab $m$ (7 bzw. 6 Parameter).
Die Drehung wird geschlossen über Quaternionen nach Horn berechnet: Man reduziert beide Punktgruppen auf ihre
Schwerpunkte, bildet die Kreuzkovarianzmatrix und nimmt den Eigenvektor zum größten Eigenwert einer daraus
gebildeten symmetrischen $4\times4$-Matrix als Drehquaternion.

> **Professor-Modus.** Die geschlossene Lösung minimiert die ungewichtete Quadratsumme der Restklaffen;
> unterschiedliche Genauigkeiten der identischen Punkte (etwa Lage besser als Höhe) werden nicht
> berücksichtigt. Bei nahezu kollinearen identischen Punkten ist die Drehung um die Verbindungsachse
> unbestimmt – der zweitgrößte Eigenwert liegt dann nahe am größten. Fehlt die Höhe in den Punktdateien, setzt
> das Werkzeug sie auf 0; eine 3D-Transformation ebener Koordinaten ist dann formal möglich, aber sinnlos.

---

## 13. Bedienung: Messdaten-Editor und Ausgaben

* **Anklicken im Netzbild:** Eine Linie öffnet rechts die Tabelle *Messungen* und markiert alle zugehörigen
  Zeilen (beide Richtungen, Strecken, Höhenunterschiede); ein Punkt öffnet die Tabelle *Punkte*.
* **Bearbeiten:** Werte direkt ändern; Schalter **R S H** streichen einzelne Beobachtungen; Punktart mit
  **F L H N** festlegen. Liegt schon ein Ergebnis vor, wird sofort neu gerechnet.
* **Speichern:** als Netzdatei; die vorige Fassung bleibt als `*.net.bak` erhalten.
* **Protokoll:** RTF im Format A4 quer (in *Datei ▸ Voreinstellungen* auf Hochformat umstellbar).
  Am besten mit Word oder LibreOffice öffnen; WordPad ignoriert das Papierformat.
* **Netzbild:** PDF und DXF mit Fehlerellipsen; Ellipsenmaßstab automatisch oder fest.

---

## 14. Quellen

**Lehrbücher**

* Niemeier, W. (2008): *Ausgleichungsrechnung – Statistische Auswertemethoden*. 2. Auflage, de Gruyter, Berlin.
* Jäger, R.; Müller, T.; Saler, H.; Schwäble, R. (2005): *Klassische und robuste Ausgleichungsverfahren*.
  Wichmann, Heidelberg.
* Ghilani, C. D. (2017): *Adjustment Computations – Spatial Data Analysis*. 6. Auflage, Wiley, Hoboken.

**Originalarbeiten**

* Baarda, W. (1968): *A Testing Procedure for Use in Geodetic Networks*. Netherlands Geodetic Commission,
  Publications on Geodesy, New Series, Vol. 2, No. 5, Delft.
* Huber, P. J. (1964): Robust Estimation of a Location Parameter. *Annals of Mathematical Statistics* 35(1),
  73–101. <https://doi.org/10.1214/aoms/1177703732>
* Krarup, T.; Juhl, J.; Kubik, K. (1980): Götterdämmerung over Least Squares Adjustment. *Proceedings of the
  14th Congress of the International Society of Photogrammetry*, Hamburg.
* Horn, B. K. P. (1987): Closed-form solution of absolute orientation using unit quaternions. *Journal of the
  Optical Society of America A* 4(4), 629–642. <https://doi.org/10.1364/JOSAA.4.000629>
* Wilson, E. B.; Hilferty, M. M. (1931): The Distribution of Chi-Square. *Proceedings of the National Academy
  of Sciences* 17(12), 684–688. <https://doi.org/10.1073/pnas.17.12.684>
* Yang, Y. (1994): Robust estimation for dependent observations. *manuscripta geodaetica* 19, 10–17
  (Grundlage des IGG-III-Schemas).

**Nachschlagen im Netz**

* Methode der kleinsten Quadrate: <https://de.wikipedia.org/wiki/Methode_der_kleinsten_Quadrate>
* Ausgleichungsrechnung: <https://de.wikipedia.org/wiki/Ausgleichungsrechnung>
* Gauß-Newton-Verfahren: <https://de.wikipedia.org/wiki/Gau%C3%9F-Newton-Verfahren>
* Cholesky-Zerlegung: <https://de.wikipedia.org/wiki/Cholesky-Zerlegung>
* QR-Zerlegung: <https://de.wikipedia.org/wiki/QR-Zerlegung>
* Singulärwertzerlegung: <https://de.wikipedia.org/wiki/Singul%C3%A4rwertzerlegung>
* Chi-Quadrat-Verteilung: <https://de.wikipedia.org/wiki/Chi-Quadrat-Verteilung>
* Helmert-Transformation: <https://de.wikipedia.org/wiki/Helmert-Transformation>
* Freie Stationierung: <https://de.wikipedia.org/wiki/Freie_Stationierung>
* M-Schätzer: <https://en.wikipedia.org/wiki/M-estimator>
* Iterativ neu gewichtete kleinste Quadrate: <https://en.wikipedia.org/wiki/Iteratively_reweighted_least_squares>
* Median der absoluten Abweichungen (MAD): <https://en.wikipedia.org/wiki/Median_absolute_deviation>

**AUSGL-NT selbst**

* Rechenregeln und Konstanten aus dem Original WAUSGL/AUSGL (dekompiliert und mit dem Pascal-Quelltext
  abgeglichen): <https://github.com/Jgerner01/ausgl-nt-rust/blob/main/MECHANICS.md>
* Quelltext: <https://github.com/Jgerner01/ausgl-nt-rust>

---

*Dank an **Prof. Fritz Wimbauer** und Prof. Dr.-Ing. Peter Wasmeier.*
