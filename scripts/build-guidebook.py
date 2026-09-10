from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.platypus import (
    BaseDocTemplate, Flowable, Frame, HRFlowable, KeepTogether, PageBreak,
    PageTemplate, Paragraph, Spacer, Table, TableStyle,
)


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "pdf" / "Fabric-OS-Guidebook.pdf"
PUBLIC_OUTPUT = ROOT / "website" / "public" / "docs" / "fabric-os-guidebook.pdf"

INK = colors.HexColor("#17191E")
MUTED = colors.HexColor("#626875")
LINE = colors.HexColor("#DDE1EA")
BLUE = colors.HexColor("#526CF2")
LIME = colors.HexColor("#DFFF62")
PALE_BLUE = colors.HexColor("#EEF1FF")
PALE_LIME = colors.HexColor("#F5F9D9")
AMBER = colors.HexColor("#FFF4DC")


styles = getSampleStyleSheet()
styles.add(ParagraphStyle(name="CoverBrand", fontName="Helvetica-Bold", fontSize=16, leading=19, textColor=INK, spaceAfter=18))
styles.add(ParagraphStyle(name="CoverTitle", fontName="Helvetica-Bold", fontSize=42, leading=43, textColor=INK, spaceAfter=13))
styles.add(ParagraphStyle(name="CoverSub", fontName="Helvetica", fontSize=16, leading=22, textColor=MUTED, spaceAfter=20))
styles.add(ParagraphStyle(name="Kicker", fontName="Helvetica-Bold", fontSize=8, leading=10, textColor=BLUE, tracking=1.2, spaceAfter=10))
styles.add(ParagraphStyle(name="H1x", fontName="Helvetica-Bold", fontSize=27, leading=30, textColor=INK, spaceAfter=12))
styles.add(ParagraphStyle(name="H2x", fontName="Helvetica-Bold", fontSize=15, leading=18, textColor=INK, spaceBefore=12, spaceAfter=6))
styles.add(ParagraphStyle(name="Bodyx", fontName="Helvetica", fontSize=9.5, leading=14, textColor=INK, spaceAfter=8))
styles.add(ParagraphStyle(name="Smallx", fontName="Helvetica", fontSize=8, leading=11, textColor=MUTED, spaceAfter=5))
styles.add(ParagraphStyle(name="SmallBold", fontName="Helvetica-Bold", fontSize=8, leading=11, textColor=INK, spaceAfter=4))
styles.add(ParagraphStyle(name="Quote", fontName="Helvetica-BoldOblique", fontSize=19, leading=23, textColor=INK, spaceBefore=8, spaceAfter=10))
styles.add(ParagraphStyle(name="TableHead", fontName="Helvetica-Bold", fontSize=8, leading=10, textColor=colors.white))
styles.add(ParagraphStyle(name="TableCell", fontName="Helvetica", fontSize=7.5, leading=10, textColor=INK))
styles.add(ParagraphStyle(name="TableCellMuted", fontName="Helvetica", fontSize=7.5, leading=10, textColor=MUTED))


def P(text, style="Bodyx"):
    return Paragraph(text, styles[style])


class FlowDiagram(Flowable):
    def __init__(self):
        super().__init__()
        self.width = 170 * mm
        self.height = 45 * mm

    def draw(self):
        c = self.canv
        y = self.height / 2
        centers = [28 * mm, 85 * mm, 142 * mm]
        labels = [("Intent", "what you want"), ("Boundaries", "what is allowed"), ("Outcome", "what remains")]
        fills = [PALE_BLUE, colors.HexColor("#F0ECFF"), PALE_LIME]
        for index, x in enumerate(centers):
            c.setFillColor(fills[index])
            c.setStrokeColor(LINE)
            c.circle(x, y, 16 * mm, stroke=1, fill=1)
            c.setFillColor(INK)
            c.setFont("Helvetica-Bold", 10)
            c.drawCentredString(x, y + 1 * mm, labels[index][0])
            c.setFillColor(MUTED)
            c.setFont("Helvetica", 7)
            c.drawCentredString(x, y - 5 * mm, labels[index][1])
            if index < 2:
                c.setStrokeColor(BLUE)
                c.setLineWidth(1.2)
                c.line(x + 17 * mm, y, centers[index + 1] - 17 * mm, y)
                c.line(centers[index + 1] - 19 * mm, y + 1.5 * mm, centers[index + 1] - 17 * mm, y)
                c.line(centers[index + 1] - 19 * mm, y - 1.5 * mm, centers[index + 1] - 17 * mm, y)


class StatusBand(Flowable):
    def __init__(self, title, text, fill=AMBER):
        super().__init__()
        self.title = title
        self.text = text
        self.fill = fill
        self.width = 170 * mm
        self.height = 19 * mm

    def draw(self):
        c = self.canv
        c.setFillColor(self.fill)
        c.roundRect(0, 0, self.width, self.height, 4 * mm, stroke=0, fill=1)
        c.setFillColor(INK)
        c.setFont("Helvetica-Bold", 8)
        c.drawString(7 * mm, self.height - 7 * mm, self.title)
        c.setFillColor(MUTED)
        c.setFont("Helvetica", 7.4)
        c.drawString(7 * mm, 5 * mm, self.text)


def table(rows, widths, header=True):
    converted = []
    for row_index, row in enumerate(rows):
        converted.append([P(str(value), "TableHead" if header and row_index == 0 else "TableCell") for value in row])
    t = Table(converted, colWidths=widths, repeatRows=1 if header else 0, hAlign="LEFT")
    commands = [
        ("GRID", (0, 0), (-1, -1), 0.35, LINE),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 6),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
    ]
    if header:
        commands += [("BACKGROUND", (0, 0), (-1, 0), INK), ("TEXTCOLOR", (0, 0), (-1, 0), colors.white)]
        for index in range(1, len(rows)):
            if index % 2 == 0:
                commands.append(("BACKGROUND", (0, index), (-1, index), colors.HexColor("#F8F9FC")))
    t.setStyle(TableStyle(commands))
    return t


def footer(canvas, doc):
    canvas.saveState()
    canvas.setStrokeColor(LINE)
    canvas.setLineWidth(.5)
    canvas.line(18 * mm, 14 * mm, 192 * mm, 14 * mm)
    canvas.setFillColor(MUTED)
    canvas.setFont("Helvetica", 7)
    canvas.drawString(18 * mm, 8 * mm, "Fabric OS  |  Patience AI  |  Public guidebook")
    canvas.drawRightString(192 * mm, 8 * mm, f"{doc.page}")
    canvas.restoreState()


def build_story():
    s = []
    s += [Spacer(1, 24 * mm), P("PATIENCE AI", "CoverBrand"), P("Fabric OS", "CoverTitle"), P("A public guide to a Linux-compatible foundation for AI-assisted work.", "CoverSub")]
    s += [Spacer(1, 11 * mm), FlowDiagram(), Spacer(1, 12 * mm), StatusBand("PUBLIC RELEASE GUIDE", "Product basics, device validation, security, privacy, and open-source use.", PALE_LIME), Spacer(1, 20 * mm)]
    s += [P("Version 0.1.0  |  10 September 2026", "Smallx"), P("Prepared for evaluation and early device testing. This guide intentionally stays at the public product level.", "Smallx"), PageBreak()]

    s += [P("01 / PRODUCT", "Kicker"), P("A calmer operating system", "H1x"), P("Fabric OS is a Linux-compatible, local-first foundation for intelligent work. It is built around a simple idea: AI can help, while people keep the final say.", "Bodyx"), P("The public promise", "H2x"), P("The product is designed to keep intent clear, access limited, and important work understandable. It is an early research release, not a claim of universal hardware support or production certification.", "Bodyx"), P("AI helps. People decide.", "Quote"), Spacer(1, 5 * mm), FlowDiagram(), Spacer(1, 10 * mm), P("What makes it different", "H2x"), table([
        ["Traditional software", "Fabric OS public model"],
        ["Actions can be opaque", "Intent and outcome stay visible"],
        ["Permissions are often broad", "Access is meant to be limited"],
        ["Failures can be hard to unwind", "Important work is designed for review and recovery"],
    ], [78 * mm, 92 * mm]), PageBreak()]

    s += [P("02 / HOW IT WORKS", "Kicker"), P("Simple on the surface", "H1x"), P("The public flow is intentionally easy to explain: a person sets an intent, reviews the next step, and decides what happens.", "Bodyx")]
    steps = [
        ("01", "Say what you need", "Start with a goal in your own words."),
        ("02", "See the next step", "Fabric OS presents a clear, bounded suggestion."),
        ("03", "Stay in control", "You can accept, change, or stop before important work begins."),
        ("04", "Keep the result", "Important changes remain reviewable and recoverable."),
    ]
    s += [table([["Step", "Public experience", "What it means"]] + list(steps), [18 * mm, 47 * mm, 105 * mm]), Spacer(1, 10 * mm), StatusBand("PUBLIC BOUNDARY", "The guide explains the experience, not private implementation or business logic.", PALE_BLUE), PageBreak()]

    s += [P("03 / LINUX FOUNDATION", "Kicker"), P("Built for real machines", "H1x"), P("Fabric OS follows upstream Linux wherever practical and keeps the base system useful without cloud AI. The first target is x86_64 with UEFI and a GNU/Linux-compatible userspace.", "Bodyx"), table([
        ["Area", "Public release position"],
        ["Architecture", "x86_64 initial target; additional architectures require separate validation."],
        ["Boot", "UEFI device testing is required before claiming hardware support."],
        ["Userspace", "GNU/Linux-compatible userspace with system services and a familiar shell."],
        ["AI", "Local-first; cloud providers are optional and must be user-controlled."],
        ["Storage", "ext4 research image for the current early build; installer and recovery status must be stated per release."],
    ], [42 * mm, 128 * mm]), Spacer(1, 10 * mm), P("Device validation", "H2x"), P("Use a spare machine or a separately replaceable test disk. Record the exact model, firmware, CPU, memory, graphics, storage, network, audio, sleep, and recovery results. Do not install an early research image on your only computer.", "Bodyx"), P("See the public device guide at /device and the repository validation record before testing.", "Smallx"), PageBreak()]

    s += [P("04 / SECURITY AND PRIVACY", "Kicker"), P("Useful, with clear limits", "H1x"), P("Fabric OS treats AI output as untrusted and keeps authority separate from suggestions. The public security posture is about boundaries, not about asking people to trust a model.", "Bodyx"), table([
        ["Principle", "Public meaning"],
        ["Fail closed", "When a sensitive decision is unclear, access should stop."],
        ["Least privilege", "A task should receive only the access it needs."],
        ["Local-first", "Private work should be able to stay on the device."],
        ["Credential protection", "Provider keys belong in secure storage, not ordinary settings or logs."],
        ["Recovery", "Important changes should be reviewable and, where possible, reversible."],
    ], [43 * mm, 127 * mm]), Spacer(1, 10 * mm), P("Privacy basics", "H2x"), P("The website uses necessary preference storage and optional analytics only after a visitor chooses it. The OS must apply the same public principles: clear notice, purpose-specific choices, easy withdrawal, minimal collection, retention limits, and a clear local/cloud indicator.", "Bodyx"), StatusBand("LEGAL STATUS", "This guide is not a GDPR or DPDP certification and requires jurisdiction-specific review.", AMBER), PageBreak()]

    s += [P("05 / OPEN SOURCE AND LICENSES", "Kicker"), P("Use it responsibly", "H1x"), P("Fabric OS project code is intended to be distributed under the Apache License 2.0 unless a file or dependency says otherwise. Keep the license and notices with redistributed portions.", "Bodyx"), P("What the project license covers", "H2x"), P("The project license governs project code and documentation where the repository states that license. It does not automatically relicense the Linux kernel, firmware, model weights, datasets, fonts, icons, or third-party packages.", "Bodyx"), table([
        ["Asset type", "Required release treatment"],
        ["Fabric OS code", "Apache License 2.0 notice and conditions."],
        ["Rust and Node dependencies", "Keep each dependency's own license and attribution; archive a complete SBOM."],
        ["Linux and system components", "Follow each upstream component's license and source/notice requirements."],
        ["Firmware and drivers", "Record source, version, redistribution terms, and device limitations."],
        ["Models and datasets", "Record exact version/hash, source, license, use restrictions, and provenance."],
        ["Fonts, icons, and media", "Record origin and permission; do not assume the project license covers them."],
    ], [47 * mm, 123 * mm]), Spacer(1, 8 * mm), P("Release rule", "H2x"), P("If the terms or provenance are unclear, keep the asset out of the release until reviewed.", "Bodyx"), PageBreak()]

    s += [P("06 / RELEASE AND SUPPORT", "Kicker"), P("Evidence before confidence", "H1x"), P("A launch candidate is more than a website and an image. Each release needs a named owner, a reproducible record, and evidence for the exact artifacts and devices being supported.", "Bodyx"), table([
        ["Gate", "Evidence"],
        ["Source", "Reviewed tag, lockfiles, tests, and clean release record."],
        ["Artifacts", "SHA-256 manifest, signatures, provenance, and SBOM."],
        ["Device", "Exact hardware matrix, install results, recovery path, and known limitations."],
        ["Security", "Threat tests, credential-redaction checks, incident process, and external review."],
        ["Privacy", "Data map, retention, vendors, consent, rights process, and transfer review."],
        ["Support", "Contact, vulnerability reporting, release notes, and end-of-life policy."],
    ], [43 * mm, 127 * mm]), Spacer(1, 12 * mm), P("Support contact", "H2x"), P("Patience AI: info@patienceai.in", "Bodyx"), P("The contact and legal entity details must be finalized before production launch.", "Smallx"), StatusBand("CURRENT STATUS", "Fabric OS remains an early research release; do not use it as the sole system for safety-critical work.", AMBER), PageBreak()]

    s += [P("07 / LEGAL AND PUBLICATION NOTES", "Kicker"), P("Read this before publishing", "H1x"), P("This is a public product guide. It covers the user-facing product, supported release principles, privacy expectations, and open-source responsibilities.", "Bodyx"), P("Before launch", "H2x"), P("Finalize the legal entity, launch countries, governing law, privacy contact, terms of use, accessibility review, release process, support contact, and independent security review.", "Bodyx"), P("This guide is informational and is not a legal certification. Compliance depends on the organization, markets, processing, vendors, and evidence behind the release.", "Quote"), Spacer(1, 8 * mm), P("Public references", "H2x"), P("License, security, compliance, device, and release materials are maintained with the project and should be reviewed for the specific release.", "Smallx"), P("Website: patienceai.in  |  Product: Fabric OS  |  Copyright 2026 Patience AI. All copyright reserved.", "Smallx")]
    return s


def main():
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    PUBLIC_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    frame = Frame(18 * mm, 20 * mm, 174 * mm, 257 * mm, id="normal")
    doc = BaseDocTemplate(str(OUTPUT), pagesize=A4, leftMargin=18 * mm, rightMargin=18 * mm, topMargin=20 * mm, bottomMargin=20 * mm, title="Fabric OS Guidebook", author="Patience AI")
    doc.addPageTemplates([PageTemplate(id="all", frames=frame, onPage=footer)])
    doc.build(build_story())
    PUBLIC_OUTPUT.write_bytes(OUTPUT.read_bytes())
    print(OUTPUT)
    print(PUBLIC_OUTPUT)


if __name__ == "__main__":
    main()
