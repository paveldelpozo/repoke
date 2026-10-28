"""Renders the Markdown subset docs/plugins.md uses (headings, paragraphs, lists, tables, fenced
code, inline code, bold, links) into HTML, with only the standard library, so the site builds
without downloading anything.

Usage: python3 render.py in.md > out.html
"""

import html
import re
import sys

REPO = "https://github.com/paveldelpozo/repoke/blob/main/"


def spans(text):
    """Code spans and bold; code spans first, so nothing inside them is touched."""
    out = []
    for part in re.split(r"(`[^`]+`)", text):
        if len(part) > 1 and part.startswith("`") and part.endswith("`"):
            out.append(f"<code>{html.escape(part[1:-1])}</code>")
        else:
            part = html.escape(part, quote=False)
            out.append(re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", part))
    return "".join(out)


def inline(text):
    # Links first: their label may itself be code, as in [`examples/plugins`](../examples/plugins).
    out = []
    pieces = re.split(r"\[([^\]]+)\]\(([^)\s]+)\)", text)
    for n, piece in enumerate(pieces):
        if n % 3 == 0:
            out.append(spans(piece))
        elif n % 3 == 1:
            url = pieces[n + 1]
            # Links in the guide are relative to docs/; on the site they point at GitHub.
            if url.startswith("../"):
                url = REPO + url[3:]
            out.append(f'<a href="{html.escape(url)}">{spans(piece)}</a>')
    return "".join(out)


def cells(row):
    return [c.strip() for c in row.strip().strip("|").split("|")]


def render(markdown):
    lines = markdown.split("\n")
    out, paragraph, i = [], [], 0

    def flush():
        if paragraph:
            out.append(f"<p>{inline(' '.join(paragraph))}</p>")
            paragraph.clear()

    while i < len(lines):
        line = lines[i]
        if line.startswith("```"):
            flush()
            lang = line[3:].strip()
            code = []
            i += 1
            while i < len(lines) and not lines[i].startswith("```"):
                code.append(lines[i])
                i += 1
            attr = f' class="language-{html.escape(lang)}"' if lang else ""
            out.append(f"<pre><code{attr}>{html.escape(chr(10).join(code))}</code></pre>")
        elif heading := re.match(r"(#{1,4}) (.+)", line):
            flush()
            level = len(heading.group(1))
            out.append(f"<h{level}>{inline(heading.group(2))}</h{level}>")
        elif line.startswith("|"):
            flush()
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(cells(lines[i]))
                i += 1
            i -= 1
            head, body = rows[0], [r for r in rows[2:]]
            out.append("<table><thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in head) + "</tr></thead><tbody>")
            out.extend("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in row) + "</tr>" for row in body)
            out.append("</tbody></table>")
        elif line.startswith("- "):
            flush()
            items = []
            while i < len(lines) and (lines[i].startswith("- ") or (lines[i].startswith("  ") and items)):
                if lines[i].startswith("- "):
                    items.append(lines[i][2:])
                else:
                    items[-1] += " " + lines[i].strip()
                i += 1
            i -= 1
            out.append("<ul>" + "".join(f"<li>{inline(item)}</li>" for item in items) + "</ul>")
        elif not line.strip():
            flush()
        else:
            paragraph.append(line.strip())
        i += 1
    flush()
    return "\n".join(out)


if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as source:
        print(render(source.read()))
