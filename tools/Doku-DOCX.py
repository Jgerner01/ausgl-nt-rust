# Zweck:    Erzeugt aus docs/*.md Word-Dateien (docs/*.docx) mit echten Word-Formeln (OMML).
#           Formeln: LaTeX → MathML (latex2mathml) → OMML (MML2OMML.XSL aus Microsoft Office).
#           Professor-Modus als eigene Absatzvorlage: dunkelgrün, kursiv, grüner Randstrich.
# Hardware: keine
# Autor:    Johann Gerner
#
# Aufruf:   python tools/Doku-DOCX.py
# Benötigt: pip install markdown latex2mathml python-docx lxml; Microsoft Office (MML2OMML.XSL)

import importlib.util
import re
from pathlib import Path

import markdown
from docx import Document
from docx.enum.style import WD_STYLE_TYPE
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH, WD_BREAK
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt, RGBColor
from lxml import etree, html as lhtml

HIER = Path(__file__).parent
DOCS = HIER.parent / "docs"
DATEIEN = ["Kurzanleitung", "Handbuch"]

# Aufbereitung (Formel-Platzhalter, Professor-Blöcke) aus dem HTML-Werkzeug übernehmen
_spec = importlib.util.spec_from_file_location("doku_html", HIER / "Doku-HTML.py")
doku_html = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(doku_html)

XSL_KANDIDATEN = [
    Path(r"C:\Program Files\Microsoft Office\root\Office16\MML2OMML.XSL"),
    Path(r"C:\Program Files (x86)\Microsoft Office\root\Office16\MML2OMML.XSL"),
    Path(r"C:\Program Files\Microsoft Office\Office16\MML2OMML.XSL"),
]
MML_NS = "http://www.w3.org/1998/Math/MathML"
OMML_NS = "http://schemas.openxmlformats.org/officeDocument/2006/math"

# Farben und Maße
FARBE_TEXT = RGBColor(0x0F, 0x17, 0x2A)
FARBE_TITEL = RGBColor(0x1E, 0x3A, 0x8A)
FARBE_PROF = RGBColor(0x14, 0x53, 0x2D)
FARBE_LINK = RGBColor(0x1D, 0x4E, 0xD8)
HG_PROF = "F0FDF4"
RAND_PROF = "15803D"
HG_HINWEIS = "FEF3C7"
RAND_HINWEIS = "D97706"
HG_CODE = "F1F5F9"
HG_KOPF = "EEF2FF"
SCHRIFT = "Segoe UI"
SCHRIFT_CODE = "Consolas"


def xslt():
    for p in XSL_KANDIDATEN:
        if p.exists():
            return etree.XSLT(etree.parse(str(p)))
    raise SystemExit("MML2OMML.XSL nicht gefunden – Microsoft Office wird für die Formeln benötigt.")


MML2OMML = xslt()


# ---------------------------------------------------------------- Formatvorlagen

def schattierung(pPr, farbe: str):
    shd = OxmlElement("w:shd")
    shd.set(qn("w:val"), "clear")
    shd.set(qn("w:color"), "auto")
    shd.set(qn("w:fill"), farbe)
    pPr.append(shd)


def randstrich(pPr, farbe: str):
    bdr = OxmlElement("w:pBdr")
    links = OxmlElement("w:left")
    links.set(qn("w:val"), "single")
    links.set(qn("w:sz"), "24")
    links.set(qn("w:space"), "8")
    links.set(qn("w:color"), farbe)
    bdr.append(links)
    pPr.append(bdr)


def vorlagen(doc: Document):
    st = doc.styles
    normal = st["Normal"]
    normal.font.name = SCHRIFT
    normal.font.size = Pt(11)
    normal.font.color.rgb = FARBE_TEXT
    normal.paragraph_format.space_after = Pt(6)
    normal.paragraph_format.line_spacing = 1.15
    for name, groesse, farbe, vor in [("Title", 24, FARBE_TEXT, 0), ("Heading 1", 16, FARBE_TITEL, 18), ("Heading 2", 13, FARBE_TEXT, 12)]:
        s = st[name]
        s.font.name = SCHRIFT
        s.font.size = Pt(groesse)
        s.font.bold = True
        s.font.color.rgb = farbe
        s.paragraph_format.space_before = Pt(vor)
        s.paragraph_format.space_after = Pt(6)
        s.paragraph_format.keep_with_next = True
    for name, hg, rand, farbe, kursiv in [("Professor", HG_PROF, RAND_PROF, FARBE_PROF, True), ("Hinweis", HG_HINWEIS, RAND_HINWEIS, FARBE_TEXT, False)]:
        s = st.add_style(name, WD_STYLE_TYPE.PARAGRAPH)
        s.base_style = normal
        s.font.color.rgb = farbe
        s.font.italic = kursiv
        s.paragraph_format.left_indent = Cm(0.4)
        s.paragraph_format.right_indent = Cm(0.2)
        s.paragraph_format.space_before = Pt(2)
        s.paragraph_format.space_after = Pt(2)
        pPr = s.element.get_or_add_pPr()
        randstrich(pPr, rand)
        schattierung(pPr, hg)
    code = st.add_style("Code", WD_STYLE_TYPE.PARAGRAPH)
    code.base_style = normal
    code.font.name = SCHRIFT_CODE
    code.font.size = Pt(9)
    code.paragraph_format.space_after = Pt(0)
    code.paragraph_format.line_spacing = 1.0
    schattierung(code.element.get_or_add_pPr(), HG_CODE)


# ---------------------------------------------------------------- Inhalte

def omml(math_el) -> etree._Element:
    """MathML-Element → OMML (m:oMath bzw. m:oMathPara)."""
    roh = etree.tostring(math_el, with_tail=False)
    src = etree.fromstring(roh)
    # lxml.html verliert den Namensraum: MathML-Namensraum sicherstellen
    if not src.tag.startswith("{"):
        src = etree.fromstring(roh.replace(b"<math", f'<math xmlns="{MML_NS}"'.encode(), 1))
    return MML2OMML(src).getroot()


def hyperlink(par, url: str, text: str):
    rid = par.part.relate_to(url, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", is_external=True)
    h = OxmlElement("w:hyperlink")
    h.set(qn("r:id"), rid)
    r = OxmlElement("w:r")
    rPr = OxmlElement("w:rPr")
    farbe = OxmlElement("w:color")
    farbe.set(qn("w:val"), "1D4ED8")
    unter = OxmlElement("w:u")
    unter.set(qn("w:val"), "single")
    rPr.append(farbe)
    rPr.append(unter)
    r.append(rPr)
    t = OxmlElement("w:t")
    t.text = text
    t.set(qn("xml:space"), "preserve")
    r.append(t)
    h.append(r)
    par._p.append(h)


def inline(par, node, fett=False, kursiv=False, code=False):
    """Text und Kindelemente eines HTML-Knotens als Runs anhängen."""

    def lauf(text):
        if not text:
            return
        text = re.sub(r"\s+", " ", text)
        r = par.add_run(text)
        r.bold = fett or None
        r.italic = kursiv or None
        if code:
            r.font.name = SCHRIFT_CODE
            r.font.size = Pt(9.5)

    lauf(node.text)
    for k in node:
        tag = k.tag if isinstance(k.tag, str) else ""
        tag = tag.split("}")[-1]
        if tag in ("strong", "b"):
            inline(par, k, True, kursiv, code)
        elif tag in ("em", "i"):
            inline(par, k, fett, True, code)
        elif tag == "code":
            inline(par, k, fett, kursiv, True)
        elif tag == "a":
            hyperlink(par, k.get("href", ""), k.text_content())
        elif tag == "math":
            o = omml(k)
            if o.tag == f"{{{OMML_NS}}}oMathPara":
                for om in o.findall(f"{{{OMML_NS}}}oMath"):
                    par._p.append(om)
            else:
                par._p.append(o)
        elif tag == "br":
            par.add_run().add_break(WD_BREAK.LINE)
        else:
            inline(par, k, fett, kursiv, code)
        lauf(k.tail)


def nur_formel(p_el) -> bool:
    """Absatz besteht nur aus einer abgesetzten Formel."""
    kinder = [k for k in p_el if isinstance(k.tag, str)]
    return len(kinder) == 1 and kinder[0].tag.split("}")[-1] == "math" and kinder[0].get("display") == "block" and not (p_el.text or "").strip() and not (kinder[0].tail or "").strip()


def absatz(doc, el, stil=None):
    if nur_formel(el):
        par = doc.add_paragraph(style=stil)
        par.alignment = WD_ALIGN_PARAGRAPH.CENTER
        o = omml(el[0])
        if o.tag != f"{{{OMML_NS}}}oMathPara":
            para = etree.SubElement(etree.Element("dummy"), f"{{{OMML_NS}}}oMathPara")
            para.append(o)
            o = para
        par._p.append(o)
        return par
    par = doc.add_paragraph(style=stil)
    inline(par, el)
    return par


def tabelle(doc, el, breite_cm: float):
    zeilen = el.findall(".//tr")
    if not zeilen:
        return
    spalten = max(len(z.findall("th") + z.findall("td")) for z in zeilen)
    t = doc.add_table(rows=0, cols=spalten)
    t.style = "Table Grid"
    t.alignment = WD_TABLE_ALIGNMENT.CENTER
    for z in zeilen:
        zellen = z.findall("th") + z.findall("td")
        reihe = t.add_row().cells
        for i, c in enumerate(zellen):
            par = reihe[i].paragraphs[0]
            par.paragraph_format.space_after = Pt(0)
            inline(par, c, fett=c.tag == "th")
            for r in par.runs:
                r.font.size = Pt(10)
            if c.tag == "th":
                schattierung(reihe[i]._tc.get_or_add_tcPr(), HG_KOPF)
    doc.add_paragraph()


def liste(doc, el, nummeriert: bool, stil_basis=None, ebene=1):
    """Liste; Unterlisten (ul/ol im Listenpunkt) eingerückt als eigene Absätze."""
    stil = ("List Number" if nummeriert else "List Bullet") + ("" if ebene == 1 else f" {ebene}")
    for li in el.findall("li"):
        unter = [k for k in li if k.tag in ("ul", "ol")]
        for k in unter:
            li.remove(k)
        ps = li.findall("p")
        if ps:
            for p in ps:
                inline(doc.add_paragraph(style=stil), p)
        else:
            inline(doc.add_paragraph(style=stil), li)
        for k in unter:
            liste(doc, k, k.tag == "ol", stil_basis, min(ebene + 1, 3))


def block(doc, el, stil=None, breite_cm=16.0):
    tag = el.tag.split("}")[-1] if isinstance(el.tag, str) else ""
    if tag == "h1":
        doc.add_paragraph(el.text_content().strip(), style="Title")
    elif tag == "h2":
        doc.add_paragraph(el.text_content().strip(), style="Heading 1")
    elif tag == "h3":
        par = doc.add_paragraph(style="Heading 2")
        inline(par, el)
    elif tag == "p":
        absatz(doc, el, stil)
    elif tag in ("ul", "ol"):
        liste(doc, el, tag == "ol", stil)
    elif tag == "table":
        tabelle(doc, el, breite_cm)
    elif tag == "pre":
        for zeile in el.text_content().rstrip("\n").split("\n"):
            par = doc.add_paragraph(style="Code")
            r = par.add_run(zeile if zeile else " ")
            r.font.name = SCHRIFT_CODE
        doc.add_paragraph().paragraph_format.space_after = Pt(2)
    elif tag == "blockquote":
        for k in el:
            block(doc, k, "Hinweis", breite_cm)
        doc.add_paragraph().paragraph_format.space_after = Pt(0)
    elif tag == "div" and "prof" in (el.get("class") or ""):
        erste = True
        for k in el:
            if erste and k.tag == "p":
                par = absatz(doc, k, "Professor")
                marke = par.runs[0]._r if par.runs else None
                r = par.add_run("Professor-Modus. ")
                r.bold = True
                if marke is not None:
                    marke.addprevious(r._r)
                else:
                    par._p.insert(1, r._r)
                erste = False
            else:
                block(doc, k, "Professor", breite_cm)
        doc.add_paragraph().paragraph_format.space_after = Pt(0)
    elif tag == "hr":
        par = doc.add_paragraph()
        pPr = par._p.get_or_add_pPr()
        bdr = OxmlElement("w:pBdr")
        unten = OxmlElement("w:bottom")
        for k, v in (("w:val", "single"), ("w:sz", "6"), ("w:space", "1"), ("w:color", "CBD5E1")):
            unten.set(qn(k), v)
        bdr.append(unten)
        pPr.append(bdr)
    elif tag == "div":
        for k in el:
            block(doc, k, stil, breite_cm)
    elif tag:
        absatz(doc, el, stil)


def erzeugen(name: str):
    text = (DOCS / f"{name}.md").read_text(encoding="utf-8")
    vor, formeln = doku_html.vorbereiten(text)
    inhalt = markdown.markdown(vor, extensions=["tables", "fenced_code", "md_in_html", "sane_lists"])
    for i, f in enumerate(formeln):
        inhalt = inhalt.replace(f"@@FORMEL{i}@@", f)
    wurzel = lhtml.fragment_fromstring(inhalt, create_parent="div")

    doc = Document()
    sec = doc.sections[0]
    sec.page_width, sec.page_height = Cm(21.0), Cm(29.7)
    for rand in ("left_margin", "right_margin"):
        setattr(sec, rand, Cm(2.2))
    sec.top_margin = sec.bottom_margin = Cm(2.0)
    vorlagen(doc)
    # Fußzeile mit Seitenzahl
    fp = sec.footer.paragraphs[0]
    fp.alignment = WD_ALIGN_PARAGRAPH.RIGHT
    fp.add_run(f"AUSGL-NT – {name}   ·   Seite ")
    for teil, wert in (("begin", None), ("instr", "PAGE"), ("end", None)):
        r = fp.add_run()
        if teil == "instr":
            it = OxmlElement("w:instrText")
            it.set(qn("xml:space"), "preserve")
            it.text = wert
            r._r.append(it)
        else:
            fc = OxmlElement("w:fldChar")
            fc.set(qn("w:fldCharType"), teil)
            r._r.append(fc)
    for r in fp.runs:
        r.font.size = Pt(8)
        r.font.color.rgb = RGBColor(0x64, 0x74, 0x8B)

    breite = (sec.page_width - sec.left_margin - sec.right_margin) / 360000
    for el in wurzel:
        block(doc, el, None, breite)
    ziel = DOCS / f"{name}.docx"
    doc.save(ziel)
    print(f"{ziel.name}: {len(formeln)} Formeln")


if __name__ == "__main__":
    for n in DATEIEN:
        erzeugen(n)
