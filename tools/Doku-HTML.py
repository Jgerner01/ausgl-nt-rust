# Zweck:    Erzeugt aus docs/*.md die HTML-Fassungen fürs Hilfe-Menü (Formeln als MathML, offline lesbar,
#           Professor-Modus dunkelgrün kursiv und ein-/ausblendbar). Die MD-Dateien sind die Quelle.
# Hardware: keine
# Autor:    Johann Gerner
#
# Aufruf:   python tools/Doku-HTML.py
# Benötigt: pip install markdown latex2mathml   (oder PYTHONPATH auf einen Ordner mit beiden Paketen)

import html
import re
import sys
from pathlib import Path

import markdown
from latex2mathml.converter import convert as tex_to_mathml

DOCS = Path(__file__).parent.parent / "docs"
DATEIEN = ["Kurzanleitung", "Handbuch"]
PROF_MARKE = "**Professor-Modus.**"

CSS = """
:root { --ink:#0f172a; --muted:#475569; --accent:#1d4ed8; --prof:#14532d; --line:#e2e8f0; }
* { box-sizing: border-box; }
body { margin: 0; font-family: "Segoe UI", system-ui, sans-serif; color: var(--ink); background: #f4f6fa; line-height: 1.55; }
header { position: sticky; top: 0; z-index: 2; background: #ffffff; border-bottom: 1px solid var(--line);
         display: flex; gap: 16px; align-items: center; justify-content: space-between; padding: 10px 24px; }
header .titel { font-weight: 600; }
header label { display: flex; gap: 8px; align-items: center; cursor: pointer; color: var(--prof); font-style: italic; }
header input { width: 18px; height: 18px; accent-color: #15803d; }
main { max-width: 920px; margin: 0 auto; padding: 24px 32px 64px; background: #ffffff; }
h1 { font-size: 30px; margin: 8px 0 12px; }
h2 { font-size: 22px; margin: 36px 0 10px; padding-top: 8px; border-top: 1px solid var(--line); color: #1e3a8a; }
h3 { font-size: 17px; margin: 24px 0 8px; }
a { color: var(--accent); }
code, pre { font-family: Consolas, "Cascadia Mono", monospace; font-size: 13px; }
pre { background: #f1f5f9; border: 1px solid var(--line); border-radius: 8px; padding: 12px 14px; overflow-x: auto; }
table { border-collapse: collapse; margin: 12px 0; width: 100%; font-size: 14px; }
th, td { border: 1px solid var(--line); padding: 6px 10px; text-align: left; vertical-align: top; }
th { background: #eef2ff; }
blockquote { margin: 16px 0; padding: 10px 16px; background: #fef3c7; border-left: 4px solid #d97706; border-radius: 6px; }
.prof { margin: 16px 0; padding: 12px 16px; background: #f0fdf4; border-left: 4px solid #15803d; border-radius: 6px;
        color: var(--prof); font-style: italic; }
.prof p { margin: 6px 0; }
.ohne-prof .prof { display: none; }
math[display="block"] { margin: 10px 0; padding: 4px 0; overflow-x: auto; overflow-y: hidden; }
math { font-size: 1.08em; }
@media print { header { display: none; } body { background: #fff; } main { max-width: none; padding: 0; } }
"""

SKRIPT = """
const k = document.getElementById('prof');
const setze = () => document.body.classList.toggle('ohne-prof', !k.checked);
k.addEventListener('change', setze); setze();
"""


def mathe(tex: str, block: bool) -> str:
    try:
        m = tex_to_mathml(tex, display="block" if block else "inline")
        # Dezimalkomma ({,}) ohne Operatorabstand: 66{,}1669 → eine Zahl
        return re.sub(r"<mn>(\d+)</mn><mrow><mo>&#x0002C;</mo></mrow><mn>(\d+)</mn>", r"<mn>\1,\2</mn>", m)
    except Exception as e:  # Formel nicht umwandelbar: als Text zeigen und melden
        print(f"  Warnung: Formel nicht umgewandelt ({e}): {tex}", file=sys.stderr)
        return f"<code>{html.escape(tex)}</code>"


def vorbereiten(text: str) -> tuple[str, list[str]]:
    """Formeln durch Platzhalter ersetzen (außerhalb von Codeblöcken) und Professor-Blöcke markieren."""
    formeln: list[str] = []

    def platzhalter(m: re.Match, block: bool, tabelle: bool) -> str:
        tex = m.group(1)
        if tabelle:
            # in Markdown-Tabellen ist \| das maskierte Betragszeichen
            tex = tex.replace("\\|", "|")
        formeln.append(mathe(tex, block))
        return f"@@FORMEL{len(formeln) - 1}@@"

    aus: list[str] = []
    im_code = False
    prof = False
    for zeile in text.split("\n"):
        if zeile.strip().startswith("```"):
            im_code = not im_code
            aus.append(zeile)
            continue
        if im_code:
            aus.append(zeile)
            continue
        # Professor-Modus: Blockzitat, das mit der Marke beginnt → <div class="prof">
        if zeile.startswith("> " + PROF_MARKE):
            prof = True
            aus.append('<div class="prof" markdown="1">')
            aus.append("")
            zeile = "> " + zeile[len("> " + PROF_MARKE):].lstrip()
        if prof:
            if zeile.startswith(">"):
                zeile = zeile[1:].lstrip()
            else:
                aus.append("")
                aus.append("</div>")
                prof = False
        tabelle = zeile.lstrip().startswith("|")
        zeile = re.sub(r"\$\$(.+?)\$\$", lambda m: platzhalter(m, True, tabelle), zeile)
        zeile = re.sub(r"\$(.+?)\$", lambda m: platzhalter(m, False, tabelle), zeile)
        aus.append(zeile)
    if prof:
        aus += ["", "</div>"]
    return "\n".join(aus), formeln


def seite(titel: str, inhalt: str, mit_prof: bool) -> str:
    schalter = (
        '<label><input type="checkbox" id="prof" checked> Professor-Modus anzeigen</label>' if mit_prof else ""
    )
    return f"""<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(titel)}</title>
<style>{CSS}</style>
</head>
<body>
<header><span class="titel">{html.escape(titel)}</span>{schalter}</header>
<main>
{inhalt}
</main>
{"<script>" + SKRIPT + "</script>" if mit_prof else ""}
</body>
</html>
"""


def main() -> None:
    for name in DATEIEN:
        quelle = DOCS / f"{name}.md"
        text = quelle.read_text(encoding="utf-8")
        titel = text.split("\n", 1)[0].lstrip("# ").strip()
        vor, formeln = vorbereiten(text)
        inhalt = markdown.markdown(vor, extensions=["tables", "fenced_code", "md_in_html", "sane_lists"])
        for i, f in enumerate(formeln):
            inhalt = inhalt.replace(f"@@FORMEL{i}@@", f)
        ziel = DOCS / f"{name}.html"
        ziel.write_text(seite(titel, inhalt, 'class="prof"' in inhalt), encoding="utf-8")
        print(f"{ziel.name}: {len(formeln)} Formeln")


if __name__ == "__main__":
    main()
