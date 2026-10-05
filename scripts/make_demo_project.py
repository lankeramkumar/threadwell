"""Generates samples/demo-project: a fictional bakery chain with 10 files of each type.

Types: Markdown notes (.md), plain text notes (.txt), Word documents (.docx), PDF documents (.pdf),
WebVTT meeting transcripts (.vtt) and SubRip transcripts (.srt). The same ten topics are
written in every note format, so the notes agree with each other. Two topics disagree on
purpose (croissant price, opening date), so the assistant has a conflict to report.

Run from the repository root:  python scripts/make_demo_project.py
Requires: python-docx, reportlab.
"""

from pathlib import Path

from docx import Document
from docx.shared import Pt
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parent.parent / "samples" / "demo-project"

TOPICS = [
    {
        "slug": "ordering-app-requirements",
        "title": "Ordering app requirements",
        "summary": "Customers order ahead for pickup from any of our three shops.",
        "points": [
            "Customers choose a shop, a pickup time and a basket of items.",
            "Orders can be changed until two hours before pickup.",
            "The shop sees each order on a tablet in the kitchen.",
            "Payment is taken at checkout through the card provider.",
        ],
        "tasks": ["Write the order flow screens", "Agree the cut-off rule with the shop managers"],
        "decision": "Pickup only for version one. Delivery comes later.",
    },
    {
        "slug": "supplier-contracts",
        "title": "Supplier contracts: flour and butter",
        "summary": "Two suppliers cover most of our baking volume.",
        "points": [
            "Flour contract renews on 1 January with a 4 percent price rise.",
            "Butter supplier offers a fixed price for six months if we commit to 2 tonnes a month.",
            "Both suppliers deliver on Tuesdays and Fridays.",
        ],
        "tasks": ["Compare the two butter offers", "Confirm the Friday delivery slot"],
        "decision": "Sign the six-month butter contract if the minimum volume holds.",
    },
    {
        "slug": "seasonal-menu-pricing",
        "title": "Seasonal menu pricing",
        "summary": "The autumn menu needs prices that stay within our margin target.",
        "points": [
            "Croissant price is 3.20 pounds in the current draft.",
            "Finance asked for 3.50 pounds to protect margin on butter.",
            "The pumpkin loaf is priced at 4.10 pounds in every shop.",
        ],
        "tasks": ["Agree the croissant price with finance", "Print the autumn menu boards"],
        "decision": "Croissant price is still open: 3.20 or 3.50 pounds.",
    },
    {
        "slug": "staffing-and-rota",
        "title": "Staffing and the weekly rota",
        "summary": "Weekend mornings are short-staffed at the Riverside shop.",
        "points": [
            "Riverside needs two extra bakers on Saturday and Sunday mornings.",
            "New starters finish their food hygiene course in their first week.",
            "Rota changes must be posted 48 hours ahead.",
        ],
        "tasks": ["Advertise two weekend baker roles", "Book the hygiene course places"],
        "decision": "Trial the Saturday shift pattern for four weeks.",
    },
    {
        "slug": "riverside-opening-checklist",
        "title": "Riverside shop opening checklist",
        "summary": "The Riverside shop opens on 14 November, subject to inspection.",
        "points": [
            "Oven commissioning is booked for 10 November.",
            "Fire safety certificate must be in place before the first customer.",
            "Signage and window graphics are ordered for 1 November.",
        ],
        "tasks": ["Book the council inspection", "Confirm the oven commissioning visit"],
        "decision": "Opening date is 14 November if the inspection passes.",
    },
    {
        "slug": "food-safety-audit",
        "title": "Food safety audit results",
        "summary": "The quarterly audit found two items to fix across the three shops.",
        "points": [
            "Chiller temperature log was missing for three days at the Harbour shop.",
            "Allergen labels on the seeded loaves need updating.",
            "All other checks passed with no critical findings.",
        ],
        "tasks": ["Fix the Harbour chiller log routine", "Update the seeded loaf allergen labels"],
        "decision": "Daily chiller checks move to the opening checklist.",
    },
    {
        "slug": "autumn-marketing",
        "title": "Autumn marketing campaign",
        "summary": "The campaign focuses on the pumpkin loaf and the new ordering app.",
        "points": [
            "Launch week is 3 to 9 November.",
            "Budget is 2,400 pounds, split between local radio and social posts.",
            "Each shop gets its own sign-up code for the app.",
        ],
        "tasks": ["Brief the social media agency", "Create the sign-up codes for each shop"],
        "decision": "Radio slots are booked for the first launch week only.",
    },
    {
        "slug": "loyalty-programme",
        "title": "Loyalty programme design",
        "summary": "Customers earn a stamp for each 10 pounds spent, and a free loaf at ten stamps.",
        "points": [
            "Stamps are recorded through the app, with a paper card as a fallback.",
            "Free loaves expire after 60 days.",
            "The programme needs a privacy notice before launch.",
        ],
        "tasks": ["Draft the privacy notice", "Test the stamp redemption at the till"],
        "decision": "Paper cards stay available for at least a year.",
    },
    {
        "slug": "equipment-maintenance",
        "title": "Equipment maintenance schedule",
        "summary": "Ovens and mixers need regular servicing to avoid lost baking days.",
        "points": [
            "Each deck oven is serviced every three months.",
            "The dough mixer at Harbour has a worn belt and is due a repair.",
            "Service contracts renew in March.",
        ],
        "tasks": ["Order the Harbour mixer belt", "Diarise the March service renewals"],
        "decision": "Keep the current service provider for another year.",
    },
    {
        "slug": "q4-financial-summary",
        "title": "Q4 financial summary",
        "summary": "Revenue is ahead of plan by 6 percent, mainly from weekend trade.",
        "points": [
            "Revenue for September was 118,000 pounds against a plan of 111,000.",
            "Waste fell from 7 percent to 5 percent of bakery output.",
            "Labour cost rose 2 points because of the new weekend shifts.",
        ],
        "tasks": ["Review the weekend shift cost", "Share the waste figures with the shop managers"],
        "decision": "Keep the weekend shift pattern until the next review.",
    },
]

MEETING_TOPICS = [
    ("Weekly operations", "ordering-app-requirements", "Ana", "Ben"),
    ("Supplier review", "supplier-contracts", "Ben", "Chris"),
    ("Menu pricing call", "seasonal-menu-pricing", "Chris", "Ana"),
    ("Rota planning", "staffing-and-rota", "Ana", "Dev"),
    ("Riverside readiness", "riverside-opening-checklist", "Dev", "Ben"),
    ("Audit follow-up", "food-safety-audit", "Ben", "Ana"),
    ("Campaign kickoff", "autumn-marketing", "Chris", "Dev"),
    ("Loyalty design review", "loyalty-programme", "Ana", "Chris"),
    ("Maintenance check-in", "equipment-maintenance", "Dev", "Ben"),
    ("Quarter close", "q4-financial-summary", "Ben", "Chris"),
]


def topic_by_slug(slug):
    return next(t for t in TOPICS if t["slug"] == slug)


def timestamp(seconds):
    return f"{seconds // 3600:02d}:{(seconds // 60) % 60:02d}:{seconds % 60:02d}"


def transcript_lines(meeting, topic):
    """Turns a topic into timed speaker lines. Returns (start_seconds, speaker, text, end_seconds)."""
    title, _slug, first, second = meeting
    lines = [
        (5, first, f"Welcome to the {title.lower()}. Our summary: {topic['summary']}"),
    ]
    t = 20
    for i, point in enumerate(topic["points"]):
        speaker = first if i % 2 == 0 else second
        lines.append((t, speaker, point))
        t += 22
    lines.append((t, second, f"Decision: {topic['decision']}"))
    t += 18
    for task in topic["tasks"]:
        lines.append((t, first, f"Action: {task}. Please confirm by Friday."))
        t += 15
    lines.append((t, second, "Thanks. Summary sent after the call."))
    return [(start, speaker, text, start + 12) for start, speaker, text in lines]


def write_markdown(folder):
    for topic in TOPICS:
        body = [f"# {topic['title']}", "", topic["summary"], "", "## Key points", ""]
        body += [f"- {p}" for p in topic["points"]]
        body += ["", "## Tasks", ""] + [f"- [ ] {t}" for t in topic["tasks"]]
        body += ["", "## Decision", "", topic["decision"], ""]
        (folder / f"{topic['slug']}.md").write_text("\n".join(body), encoding="utf-8")


def write_text(folder):
    for topic in TOPICS:
        body = [topic["title"].upper(), "", topic["summary"], ""]
        body += [f"* {p}" for p in topic["points"]]
        body += ["", "Tasks: " + "; ".join(topic["tasks"]) + ".", "", "Decision: " + topic["decision"], ""]
        (folder / f"{topic['slug']}.txt").write_text("\n".join(body), encoding="utf-8")


def write_docx(folder):
    for topic in TOPICS:
        doc = Document()
        doc.add_heading(topic["title"], level=1)
        doc.add_paragraph(topic["summary"])
        doc.add_heading("Key points", level=2)
        for point in topic["points"]:
            doc.add_paragraph(point, style="List Bullet")
        doc.add_heading("Tasks", level=2)
        for task in topic["tasks"]:
            doc.add_paragraph(task, style="List Number")
        doc.add_heading("Decision", level=2)
        p = doc.add_paragraph(topic["decision"])
        p.runs[0].font.size = Pt(11)
        doc.save(folder / f"{topic['slug']}.docx")


def write_pdf(folder):
    for topic in TOPICS:
        path = folder / f"{topic['slug']}.pdf"
        c = canvas.Canvas(str(path), pagesize=A4)
        width, height = A4
        y = height - 60
        c.setFont("Helvetica-Bold", 16)
        c.drawString(50, y, topic["title"])
        y -= 30
        c.setFont("Helvetica", 11)
        lines = [topic["summary"], ""] + ["Key points:"] + [f"  - {p}" for p in topic["points"]]
        lines += ["", "Tasks:"] + [f"  - {t}" for t in topic["tasks"]] + ["", f"Decision: {topic['decision']}"]
        for line in lines:
            c.drawString(50, y, line)
            y -= 18
        c.showPage()
        c.save()


def write_vtt(folder):
    for meeting in MEETING_TOPICS:
        title, slug, _a, _b = meeting
        topic = topic_by_slug(slug)
        cues = []
        for start, speaker, text, end in transcript_lines(meeting, topic):
            cues.append(f"{timestamp(start)}.000 --> {timestamp(end)}.000\n{speaker}: {text}\n")
        body = "WEBVTT\n\n" + "\n".join(cues)
        (folder / f"meeting-{slug}.vtt").write_text(body, encoding="utf-8")


def write_srt(folder):
    for index, meeting in enumerate(MEETING_TOPICS, start=1):
        _title, slug, _a, _b = meeting
        topic = topic_by_slug(slug)
        cues = []
        for n, (start, speaker, text, end) in enumerate(transcript_lines(meeting, topic), start=1):
            cues.append(f"{n}\n{timestamp(start)},000 --> {timestamp(end)},000\n{speaker}: {text}\n")
        (folder / f"call-{slug}.srt").write_text("\n".join(cues), encoding="utf-8")


def main():
    for sub in ["notes-md", "notes-txt", "documents-docx", "documents-pdf", "meetings-vtt", "meetings-srt"]:
        (ROOT / sub).mkdir(parents=True, exist_ok=True)
    write_markdown(ROOT / "notes-md")
    write_text(ROOT / "notes-txt")
    write_docx(ROOT / "documents-docx")
    write_pdf(ROOT / "documents-pdf")
    write_vtt(ROOT / "meetings-vtt")
    write_srt(ROOT / "meetings-srt")
    counts = {sub: len(list((ROOT / sub).iterdir())) for sub in ["notes-md", "notes-txt", "documents-docx", "documents-pdf", "meetings-vtt", "meetings-srt"]}
    print("generated", counts)


if __name__ == "__main__":
    main()
