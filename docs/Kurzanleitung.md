# AUSGL-NT – Kurzanleitung

AUSGL-NT gleicht geodätische Netze nach der Methode der kleinsten Quadrate aus: Lagenetze, Höhennetze,
Lage + Höhe, volle 3D-Netze und räumliche Vorwärtsschnitte.

> **Wichtig:** AUSGL-NT ist für die Ausbildung gedacht. Die Algorithmen sind nicht vollständig getestet.
> Die Nutzung geschieht auf eigene Gefahr. Professionelle Projekte dürfen mit AUSGL-NT nur **nachgerechnet**
> werden – maßgebend ist die Berechnung mit einer geprüften Software.

Die ausführliche Beschreibung mit allen Formeln steht im **Handbuch** (Hilfe ▸ Handbuch).

---

## 1. In fünf Schritten zum Ergebnis

1. **Daten bereitstellen** – entweder eine *Netzdatei* (`*.NET`, empfohlen), einen *CAPLAN-Datensatz* (`*.N`)
   oder die klassischen Dateien *Punktdatei* (`*.KOR`) + *Messdatei* (`*.DAT`) + ggf. *Nivellement*.
2. **Projekt beginnen**
    * Netzdatei: *Datei ▸ Netzdatei (\*.NET) öffnen…*
    * CAPLAN: *Datei ▸ CAPLAN-Datensatz (\*.N) importieren…*
    * KOR/DAT: *Datei ▸ Projekt neu anlegen…*, dann im Projektbeschrieb die Dateien wählen.
    * Zum Ausprobieren: *Datei ▸ Beispielprojekt laden* (enthält absichtlich einen groben Fehler).
3. **Einstellungen prüfen** (Seitenleiste links): Berechnungsart, Variante (z. B. *1. Normale Ausgleichung*
   mit festen Altpunkten oder *5. Freie Netzausgleichung*), Genauigkeiten der Messungen.
4. **Ausgleichen** – Knopf ▶ *Ausgleichen* oder Taste **F5**.
5. **Ergebnis beurteilen und ausgeben**
    * *s0* sollte nahe 1 liegen, der *Globaltest* bestanden sein.
    * Rot markierte Beobachtungen sind *Ausreißer* (normierte Verbesserung > 3,29).
    * Ausgabe: *Protokoll RTF* (A4 quer), Netzbild als *PDF* / *DXF*, *Neupunkte* als `*.KON`.

Projekt am Ende mit *Datei ▸ Projekt speichern* als `*.ausgl` sichern – darin stehen die Einstellungen und der
Verweis auf die Datendateien.

---

## 2. Wie die Daten aussehen sollen

Alle Dateien sind einfache Textdateien, die Felder sind durch **Leerzeichen** getrennt. Winkel in **gon**,
Strecken und Koordinaten in **Metern**. Y = Rechtswert, X = Hochwert.

### Netzdatei `*.NET` (empfohlen)

Punkte und Messungen in **einer** Datei. Jede Zeile beginnt mit einem Kennbuchstaben, `#` leitet einen Kommentar
ein, `-` bedeutet „kein Wert“.

```text
# P  Punkt   Rechtswert      Hochwert       Höhe     Art  Code
P    G1      32689789.5810   5330413.9290   522.7310  F    GP
P    SP1     32689783.0570   5330550.6890   -         N    -
#    Art: F = Lage+Höhe fest, L = nur Lage fest, H = nur Höhe fest, N = Neupunkt

# S  Standpunkt  Instrumentenhöhe  Bemerkung
S    SP1         1.520             Satz 1
# Z  Ziel  Hz[gon]     Strecke[m]  V[gon]   Zielhöhe  Code  Status
Z    G1    321.94583   136.9148    99.9718  1.300     GP    RSH
Z    G2    284.73635   173.4850    99.9345  1.300     GP    -SH

# N  Von  Nach  dH[m]     Strecke[m]  Status
N    SP1  G1    0.06042   136.92      H
```

* **Status** einer Zielung: `R` = Richtung, `S` = Strecke, `H` = Höhe/Zenitwinkel wird verwendet; ein `-` an
  der Stelle streicht diesen Teil (im Beispiel ist die Richtung nach G2 gestrichen).
* Eine Strecke mit `*` (z. B. `136.9148*`) ist eine **Horizontalstrecke**; dann steht in der V-Spalte der
  Höhenunterschied statt des Zenitwinkels.
* Optional dahinter: eigene Standardabweichungen `σHz [mgon]  σS [mm]  σV [mgon bzw. mm]`.

### Punktdatei `*.KOR` und Messdatei `*.DAT` (klassisch, wie WAUSGL)

```text
# KOR: Nummer  Rechtswert  Hochwert  [Höhe]  [Code]
1234   4565310.18   5312520.13   513.16   015

# DAT (CADdy): -Standpunkt  Instrumentenhöhe  /  Ziel  Hz  Strecke  Zenit  Zielhöhe  Code
-125      1.520    PP
1251     125.1234    230.333     98.4567   1.600    GP5
1252     126.4567    190.444*    10.123    1.600    GP5
```

Die Punkte der KOR-Datei sind **Altpunkte (Festpunkte)**. Für Neupunkte rechnet AUSGL-NT die Näherungs­koordinaten
selbst.

### CAPLAN-Datensätze `*.N` können direkt eingelesen werden

Datensätze aus **CAPLAN** (Cremer Programmentwicklung) liest AUSGL-NT über
*Datei ▸ CAPLAN-Datensatz (\*.N) importieren…* ein. Übernommen werden Punkte, Festpunkte (Abschnitt `F`),
Richtungssätze (`R`), Strecken (`E`) und Höhenunterschiede (`H`) – **mit ihren eigenen Standardabweichungen**.
GNSS-Basislinien (Abschnitt `B`) werden nicht ausgewertet; das Programm meldet das im Reiter *Daten*.

```text
               N 7      100.0000      250.0000      0.00000 |
              S1 7      100.0000      100.0000      0.00000 |
T
Bsp 1 - Freie Stationierung
F
              S1         1
              S1         2
E
               N              S1    150.0000   0.00200
R
               N
                              S1   200.00000  0.000600
```

Tipp: Mit *Datei ▸ Messdaten als Netzdatei (\*.NET) speichern…* wird daraus eine bearbeitbare Netzdatei.

---

## 3. Messdaten prüfen und korrigieren

* Im **Netzbild** eine Linie oder einen Punkt anklicken: Rechts öffnet sich die **Messdaten-Tabelle**, die
  zugehörigen Zeilen sind gelb markiert.
* Werte direkt in der Tabelle ändern; mit den Schaltern **R S H** einzelne Beobachtungen ein- und ausschalten;
  bei Punkten mit **F L H N** die Punktart festlegen.
* *Speichern* schreibt alles als Netzdatei – Streichungen und Punktarten stehen dann in der Datei selbst.

---

## 4. Häufige Fragen

| Meldung / Beobachtung | Ursache und Abhilfe |
|---|---|
| „keine Näherungswerte für Punkt …“ | Punkt ist nicht genug bestimmt (z. B. nur eine Richtung). Beobachtung ergänzen oder Näherungs­koordinaten in die Netzdatei schreiben (Art `N`). |
| „Normalgleichungsmatrix singulär“ | Datum fehlt: zu wenige Festpunkte. Festpunkte ergänzen oder Variante *5. Freie Netzausgleichung* wählen. |
| s0 deutlich größer als 1 | Genauigkeiten zu optimistisch angesetzt oder grobe Fehler vorhanden – Ausreißer prüfen. |
| Protokoll passt nicht auf die Seite | RTF nicht mit WordPad öffnen (ignoriert das Querformat), sondern mit Word oder LibreOffice – siehe *Datei ▸ Voreinstellungen*. |
