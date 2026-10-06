#!/usr/bin/env python3
"""Static site generator for the Wahoo portal. Python standard library only.

    python site/build.py              build dist/
    python site/build.py --release    strict: any warning (broken link, missing string, missing compiler) fails
    python site/build.py --serve      build, then serve dist/ on http://127.0.0.1:8000

Content lives in site/content/ (one HTML fragment per chapter and language), presentation in site/src/. The
playground runs the real compiler: build it first with
    cargo build -p wahoo-web --release --target wasm32-unknown-unknown
and this script copies target/.../wahoo_web.wasm into the site. Wahoo examples come from examples/ (English) and
examples/es/ (Spanish).
"""
from __future__ import annotations

import argparse
import functools
import hashlib
import html
import http.server
import json
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent
CONTENT = ROOT / "content"
SRC = ROOT / "src"
DIST = REPO / "dist"
COMPILER_WASM = REPO / "target" / "wasm32-unknown-unknown" / "release" / "wahoo_web.wasm"
EXAMPLES = REPO / "examples"

OG_LOCALE = {"en": "en_GB", "es": "es_ES"}
PAGES = ("playground", "reference", "timeline")

WARNINGS: list[str] = []


def warn(msg: str) -> None:
    if msg not in WARNINGS:
        WARNINGS.append(msg)
        print(f"  ! {msg}", file=sys.stderr)


def load_json(path: Path):
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def esc(s: str) -> str:
    return html.escape(s, quote=True)


def slugify(text: str) -> str:
    import unicodedata
    text = unicodedata.normalize("NFKD", re.sub(r"<[^>]+>", "", text)).encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


# --------------------------------------------------------------------------- content


META_RE = re.compile(r"^\s*<!--meta\s*(\{.*?\})\s*-->\s*", re.S)


def read_fragment(path: Path) -> tuple[dict, str]:
    """An HTML fragment with an optional leading <!--meta {json} --> block."""
    text = path.read_text(encoding="utf-8")
    m = META_RE.match(text)
    if not m:
        return {}, text
    try:
        meta = json.loads(m.group(1))
    except json.JSONDecodeError as e:
        sys.exit(f"{path}: invalid meta block: {e}")
    return meta, text[m.end():]


def key_paths(obj, prefix=""):
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield from key_paths(v, f"{prefix}.{k}" if prefix else k)
    else:
        yield prefix


def load_site() -> dict:
    site = load_json(CONTENT / "site.json")
    ui = {lang: load_json(CONTENT / "i18n" / f"{lang}.json") for lang in site["langs"]}
    ref = set(key_paths(ui[site["default_lang"]]))
    for lang, strings in ui.items():
        keys = set(key_paths(strings))
        for k in sorted(ref - keys):
            warn(f"i18n/{lang}.json: missing {k}")
        for k in sorted(keys - ref):
            warn(f"i18n/{lang}.json: extra key {k}")

    chapters: dict[str, list[dict]] = {}
    for lang in site["langs"]:
        chapters[lang] = []
        for n, cid in enumerate(site["chapters"]):
            path = CONTENT / lang / "chapters" / f"{cid}.html"
            if not path.exists():
                warn(f"missing chapter {lang}/{cid}")
                continue
            meta, body = read_fragment(path)
            for field in ("slug", "title", "headline", "dek", "era", "concepts", "people", "chain"):
                if field not in meta:
                    warn(f"{lang}/chapters/{cid}.html: meta lacks '{field}'")
            meta.update(id=cid, number=n, body=body, lang=lang)
            chapters[lang].append(meta)
    site.update(ui=ui, chapter_list=chapters)
    return site


# --------------------------------------------------------------------------- urls


def page_path(site: dict, lang: str, kind: str, chapter: dict | None = None) -> str:
    """Output path (relative to dist/) of a page, always a directory index."""
    prefix = "" if lang == site["default_lang"] else f"{lang}/"
    if kind == "home":
        return f"{prefix}index.html"
    if kind in PAGES:
        return f"{prefix}{site['ui'][lang][kind]['slug']}/index.html"
    return f"{prefix}{chapter['slug']}/index.html"


def rel_url(from_path: str, to_path: str) -> str:
    """Relative link between two dist/ paths; directory indexes become their folder."""
    depth = from_path.count("/")
    target = to_path[: -len("index.html")] if to_path.endswith("index.html") else to_path
    url = "../" * depth + target
    return url or "./"


# --------------------------------------------------------------------------- syntax highlighting

# Wahoo: the same token classes as the compiler's lexer, plus comments (which the lexer skips).
WAHOO_KEYWORDS = {"world", "pipe", "coin", "power_up", "damage", "by", "question", "else", "bounce", "run", "from",
                  "to", "flag", "and", "or", "not"}
WAHOO_LITERALS = {"star", "goomba"}
WAHOO_TYPES = {"coins", "switch", "text"}
WAHOO_RE = re.compile(r'(?P<com>//[^\n]*)|(?P<str>"(?:\\.|[^"\\\n])*"?)|(?P<num>\b\d[\d_]*\b)|(?P<word>[A-Za-z_]\w*)'
                      r'|(?P<op>->|==|!=|<=|>=|[-+*/%<>=])|(?P<other>.)', re.S)

WAT_RE = re.compile(r'(?P<com>;;[^\n]*|\(;[^;]*;\))|(?P<str>"(?:\\.|[^"\\])*")|(?P<id>\$[\w.]+)'
                    r'|(?P<num>-?\b\d+\b)|(?P<word>[a-z_][\w.]*)|(?P<other>.)', re.S)
WAT_KEYWORDS = {"module", "func", "param", "result", "local", "import", "export", "memory", "data", "type", "block",
                "loop", "if", "else", "end", "i32"}


def highlight(code: str, lang: str) -> str:
    out = []
    regex = WAHOO_RE if lang == "wahoo" else WAT_RE
    for m in regex.finditer(code):
        kind, text = m.lastgroup, m.group()
        cls = None
        if kind == "com":
            cls = "c"
        elif kind == "str":
            cls = "s"
        elif kind == "num":
            cls = "n"
        elif kind == "id":
            cls = "v"
        elif kind == "op" and lang == "wahoo":
            cls = "o"
        elif kind == "word":
            if lang == "wahoo":
                cls = ("k" if text in WAHOO_KEYWORDS else "b" if text in WAHOO_LITERALS else
                       "t" if text in WAHOO_TYPES else "f" if text == "wahoo" else None)
            else:
                cls = "k" if text in WAT_KEYWORDS else "i"
        out.append(f'<span class="{cls}">{esc(text)}</span>' if cls else esc(text))
    return "".join(out)


CODE_RE = re.compile(r'<pre class="(wahoo|wat)">(.*?)</pre>', re.S)


def render_code(text: str) -> str:
    def repl(m: re.Match) -> str:
        lang, code = m.group(1), html.unescape(m.group(2)).strip("\n")
        return f'<pre class="code-block lang-{lang}"><code>{highlight(code, lang)}</code></pre>'
    return CODE_RE.sub(repl, text)


# --------------------------------------------------------------------------- transforms


def resolve_links(text: str, site: dict, lang: str, here: str, where: str) -> str:
    """href="@ch:<id>[#frag]", "@home", "@playground", "@reference", "@timeline" -> relative URLs (validated)."""
    by_id = {c["id"]: c for c in site["chapter_list"][lang]}

    def repl(m: re.Match) -> str:
        target, frag = m.group(1), m.group(2) or ""
        if target == "home":
            path = page_path(site, lang, "home")
        elif target in PAGES:
            path = page_path(site, lang, target)
        elif target.startswith("ch:") and target[3:] in by_id:
            path = page_path(site, lang, "chapter", by_id[target[3:]])
        else:
            warn(f"{where}: unknown link target @{target}")
            return 'href="#"'
        return f'href="{rel_url(here, path)}{frag}"'

    return re.sub(r'href="@([a-z:\-]+)(#[^"]*)?"', repl, text)


def add_heading_ids(text: str) -> tuple[str, list[tuple[str, str]]]:
    toc: list[tuple[str, str]] = []
    seen: set[str] = set()

    def repl(m: re.Match) -> str:
        attrs, inner = m.group(1), m.group(2)
        idm = re.search(r'id="([^"]+)"', attrs)
        hid = idm.group(1) if idm else slugify(inner)
        while hid in seen:
            hid += "-2"
        seen.add(hid)
        toc.append((hid, re.sub(r"</?(?:a|em|strong)\b[^>]*>", "", inner).strip()))
        if not idm:
            attrs += f' id="{hid}"'
        return f'<h2{attrs}><a class="anchor" href="#{hid}" aria-hidden="true">§</a>{inner}</h2>'

    return re.sub(r"<h2([^>]*)>(.*?)</h2>", repl, text, flags=re.S), toc


DEMO_RE = re.compile(r'data-demo="([a-z0-9\-]+)/([a-z0-9\-]+)"')


def demo_scripts(text: str, where: str) -> list[str]:
    files: list[str] = []
    for group, name in DEMO_RE.findall(text):
        js = SRC / "js" / "demos" / f"{group}.js"
        if not js.exists():
            warn(f"{where}: no script for demo {group}/{name}")
            continue
        if f'"{group}/{name}"' not in js.read_text(encoding="utf-8"):
            warn(f"{where}: {js.name} does not register {group}/{name}")
        if group not in files:
            files.append(group)
    return files


def check_demo_strings(site: dict) -> None:
    """Every t("demo.key") used by a script must exist in each language's demos section."""
    used: set[str] = set()
    for js in (SRC / "js").rglob("*.js"):
        used |= set(re.findall(r'\bt\("([a-zA-Z0-9_.\-]+)"', js.read_text(encoding="utf-8")))
    for lang, strings in site["ui"].items():
        have = set(key_paths(strings.get("demos", {})))
        for k in sorted(used - have):
            if k.endswith(".") and any(h.startswith(k) for h in have):
                continue
            warn(f"i18n/{lang}.json: demos.{k} is used by a script but missing")


def transform(text: str, site: dict, lang: str, here: str, where: str) -> str:
    return resolve_links(render_code(text), site, lang, here, where)


# --------------------------------------------------------------------------- assets


def copy_assets(site: dict) -> dict[str, str]:
    """Copy src/ assets (plus the compiler and the examples) to dist/assets/; return {name: versioned url-path}."""
    out: dict[str, str] = {}

    def add(rel: str, data: bytes) -> None:
        dest = DIST / "assets" / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        out[rel] = f"assets/{rel}?v={hashlib.sha256(data).hexdigest()[:10]}"

    for f in SRC.rglob("*"):
        if f.is_file() and f.name != "template.html":
            add(f.relative_to(SRC).as_posix(), f.read_bytes())

    if COMPILER_WASM.exists():
        add("wahoo.wasm", COMPILER_WASM.read_bytes())
    else:
        warn(f"compiler not built ({COMPILER_WASM.relative_to(REPO)}): "
             "cargo build -p wahoo-web --release --target wasm32-unknown-unknown")

    for lang in site["langs"]:
        examples = []
        for ex in site["examples"]:
            path = EXAMPLES / lang / f"{ex}.wahoo" if lang != "en" else EXAMPLES / f"{ex}.wahoo"
            if not path.exists():
                warn(f"missing example {path.relative_to(REPO)}")
                continue
            title = site["ui"][lang]["examples"].get(ex)
            if not title:
                warn(f"i18n/{lang}.json: examples.{ex} missing")
            examples.append({"id": ex, "title": title or ex, "code": path.read_text(encoding="utf-8")})
        js = "window.WAHOO_EXAMPLES = " + json.dumps(examples, ensure_ascii=False, indent=1) + ";\n"
        add(f"js/data/examples-{lang}.js", js.encode("utf-8"))
    return out


# --------------------------------------------------------------------------- page parts


def same_page(site: dict, lang: str, current: str) -> str:
    """dist/ path of the page `current` ("home", a page id or a chapter id) in another language."""
    if current in PAGES:
        return page_path(site, lang, current)
    match = [c for c in site["chapter_list"][lang] if c["id"] == current]
    return page_path(site, lang, "chapter", match[0]) if match else page_path(site, lang, "home")


def render_header(site: dict, lang: str, here: str, current: str) -> str:
    ui = site["ui"][lang]
    home = rel_url(here, page_path(site, lang, "home"))
    items = []
    for c in site["chapter_list"][lang]:
        url = rel_url(here, page_path(site, lang, "chapter", c))
        cur = ' aria-current="page"' if current == c["id"] else ""
        items.append(f'<li class="ch-{c["id"]}"><a href="{url}"{cur}><span class="num">{c["number"]:02d}</span>'
                     f'{esc(c["title"])}</a></li>')
    pages = []
    items.append('<li class="menu-sep" aria-hidden="true"></li>')
    for p in ("playground", "reference", "timeline"):
        url = rel_url(here, page_path(site, lang, p))
        cur = ' aria-current="page"' if current == p else ""
        pages.append(f'<a class="nav-{p}" href="{url}"{cur}>{esc(ui["nav"][p])}</a>')
        # On small screens the header hides these links and the menu shows them instead.
        items.append(f'<li class="menu-extra"><a href="{url}"{cur}><span class="num">·</span>{esc(ui["nav"][p])}</a></li>')
    links = []
    for other in site["langs"]:
        target = same_page(site, other, current)
        cur = ' aria-current="true"' if other == lang else ""
        links.append(f'<a href="{rel_url(here, target)}" hreflang="{other}" lang="{other}"{cur}>{other.upper()}</a>')
    langs = f'<nav class="langs" aria-label="{esc(ui["nav"]["lang"])}">{"".join(links)}</nav>'
    return f"""<header class="site-header">
  <a class="skip" href="#main">{esc(ui["nav"]["skip"])}</a>
  <a class="brand" href="{home}"><span class="brand-mark" aria-hidden="true">?</span>{esc(ui["site_short"])}</a>
  <nav class="site-nav" aria-label="{esc(ui["nav"]["main"])}">
    <details class="chapters-menu">
      <summary>{esc(ui["nav"]["chapters"])}</summary>
      <ol>{"".join(items)}</ol>
    </details>
    {"".join(pages)}
    {langs}
    <button class="theme-toggle" type="button" aria-label="{esc(ui["nav"]["theme"])}" title="{esc(ui["nav"]["theme"])}">
      <span class="theme-icon" aria-hidden="true"></span>
    </button>
  </nav>
</header>"""


def render_footer(site: dict, lang: str) -> str:
    ui = site["ui"][lang]
    return f"""<footer class="site-footer">
  <p>{ui["footer"]["text"]}</p>
  <p>{ui["footer"]["siblings"]}</p>
  <p><a href="{esc(site["repo"])}">{esc(ui["footer"]["source"])}</a> · <a href="{esc(site["author_url"])}">{esc(site["author"])}</a></p>
</footer>"""


def render_chain(site: dict, lang: str, here: str, current: str | None = None, compact: bool = False) -> str:
    """The source -> ... -> running program chain: the site's table of contents and its mental model."""
    ui = site["ui"][lang]
    rows = []
    for c in site["chapter_list"][lang]:
        url = rel_url(here, page_path(site, lang, "chapter", c))
        cur = ' aria-current="step"' if c["id"] == current else ""
        if compact:
            rows.append(f'<li class="ch-{c["id"]}"><a href="{url}"{cur} title="{esc(c["title"])}">'
                        f'<span class="dot" aria-hidden="true"></span><span class="lbl">{esc(c["chain"])}</span></a></li>')
        else:
            concepts = "".join(f"<li>{transform(t, site, lang, here, 'chain')}</li>" for t in c["concepts"])
            rows.append(f"""<li class="ch-{c["id"]}">
  <a class="chain-card" href="{url}">
    <span class="chain-num">{c["number"]:02d}</span>
    <span class="chain-body">
      <span class="chain-tag">{esc(c["chain"])} <span class="chain-era">{esc(c["era"])}</span></span>
      <span class="chain-title">{esc(c["headline"])}</span>
      <span class="chain-dek">{transform(c["dek"], site, lang, here, "chain")}</span>
      <ul class="chain-theorems" aria-label="{esc(ui["chapter"]["concepts"])}">{concepts}</ul>
    </span>
  </a>
</li>""")
    cls = "chain chain-compact" if compact else "chain"
    return f'<ol class="{cls}" aria-label="{esc(ui["nav"]["chapters"])}">{"".join(rows)}</ol>'


def render_chapter(site: dict, lang: str, ch: dict, here: str) -> tuple[str, list[str]]:
    ui = site["ui"][lang]
    where = f"{lang}/chapters/{ch['id']}.html"
    body = transform(ch["body"], site, lang, here, where)
    body, toc = add_heading_ids(body)
    demos = demo_scripts(body, where)
    chs = site["chapter_list"][lang]
    i = ch["number"]

    def nav_card(other: dict | None, label: str, cls: str) -> str:
        if not other:
            url = rel_url(here, page_path(site, lang, "playground"))
            return (f'<a class="pn-card {cls}" href="{url}"><span class="pn-label">{esc(label)}</span>'
                    f'<span class="pn-title">{esc(ui["nav"]["playground"])}</span>'
                    f'<span class="pn-dek">{esc(ui["playground"]["dek_short"])}</span></a>') if cls == "next" else \
                f'<span class="pn-card {cls} empty"></span>'
        url = rel_url(here, page_path(site, lang, "chapter", other))
        return (f'<a class="pn-card {cls} ch-{other["id"]}" href="{url}"><span class="pn-label">{esc(label)}</span>'
                f'<span class="pn-title">{other["number"]:02d} · {esc(other["title"])}</span>'
                f'<span class="pn-dek">{esc(other["headline"])}</span></a>')

    prev_ = chs[i - 1] if i > 0 else None
    next_ = chs[i + 1] if i + 1 < len(chs) else None
    toc_html = "".join(f'<li><a href="#{hid}">{label}</a></li>' for hid, label in toc)
    concepts = "".join(f"<li>{transform(t, site, lang, here, where)}</li>" for t in ch["concepts"])
    people = ", ".join(esc(p) for p in ch["people"])
    main = f"""<article class="chapter">
<header class="chapter-head">
  {render_chain(site, lang, here, ch["id"], compact=True)}
  <p class="kicker">{esc(ui["chapter"]["label"])} {ch["number"]:02d} · {esc(ch["title"])}</p>
  <h1>{esc(ch["headline"])}</h1>
  <p class="dek">{transform(ch["dek"], site, lang, here, where)}</p>
  <dl class="byline">
    <div><dt>{esc(ui["chapter"]["era"])}</dt><dd>{esc(ch["era"])}</dd></div>
    <div><dt>{esc(ui["chapter"]["people"])}</dt><dd>{people}</dd></div>
    <div class="wide"><dt>{esc(ui["chapter"]["concepts"])}</dt><dd><ul>{concepts}</ul></dd></div>
  </dl>
  <nav class="toc" aria-label="{esc(ui["chapter"]["toc"])}"><p class="toc-title">{esc(ui["chapter"]["toc"])}</p><ol>{toc_html}</ol></nav>
</header>
<div class="prose">
{body}
</div>
<nav class="prev-next" aria-label="{esc(ui["chapter"]["prevnext"])}">
  {nav_card(prev_, ui["chapter"]["prev"], "prev")}
  {nav_card(next_, ui["chapter"]["next"], "next")}
</nav>
</article>"""
    return main, demos


def render_home(site: dict, lang: str, here: str) -> tuple[str, list[str]]:
    _, body = read_fragment(CONTENT / lang / "home.html")
    where = f"{lang}/home.html"
    body = body.replace("<!--chain-->", render_chain(site, lang, here))
    body = transform(body, site, lang, here, where)
    return f'<article class="home">{body}</article>', demo_scripts(body, where)


def render_page(site: dict, lang: str, page: str, here: str) -> tuple[str, dict, list[str]]:
    """playground and reference: a meta block (title, dek) and an HTML body; h2s make the table of contents."""
    meta, body = read_fragment(CONTENT / lang / f"{page}.html")
    where = f"{lang}/{page}.html"
    ui = site["ui"][lang]
    body = transform(body, site, lang, here, where)
    body, toc = add_heading_ids(body)
    toc_html = ""
    if toc and meta.get("toc", True):
        items = "".join(f'<li><a href="#{hid}">{label}</a></li>' for hid, label in toc)
        toc_html = f'<nav class="toc" aria-label="{esc(ui["chapter"]["toc"])}"><p class="toc-title">{esc(ui["chapter"]["toc"])}</p><ol>{items}</ol></nav>'
    main = f"""<article class="page page-{page}">
<header class="page-head">
  <p class="kicker">{esc(meta.get("kicker", ""))}</p>
  <h1>{esc(meta["title"])}</h1>
  <p class="dek">{transform(meta["dek"], site, lang, here, where)}</p>
  {toc_html}
</header>
<div class="prose">
{body}
</div>
</article>"""
    return main, meta, demo_scripts(body, where)


def render_timeline(site: dict, lang: str, here: str) -> tuple[str, dict, list[str]]:
    data = load_json(CONTENT / lang / "timeline.json")
    where = f"{lang}/timeline.json"
    by_id = {c["id"]: c for c in site["chapter_list"][lang]}
    ui = site["ui"][lang]

    chips = [f'<button type="button" class="chip" data-filter="all" aria-pressed="true">{esc(ui["timeline"]["all"])}</button>']
    for c in site["chapter_list"][lang]:
        chips.append(f'<button type="button" class="chip ch-{c["id"]}" data-filter="{c["id"]}" aria-pressed="false">'
                     f'<span class="dot" aria-hidden="true"></span>{esc(c["chain"])}</button>')

    events = sorted(data["events"], key=lambda e: e["year"])
    eras = data["eras"]
    out = []
    for era in eras:
        in_era = [e for e in events if era["from"] <= e["year"] < era["to"]]
        items = []
        for e in in_era:
            cid = e.get("chapter")
            if cid and cid not in by_id:
                warn(f"{where}: event {e['year']} references unknown chapter {cid}")
                cid = None
            ch = by_id.get(cid) if cid else None
            link = ""
            if ch:
                url = rel_url(here, page_path(site, lang, "chapter", ch))
                link = f'<a class="ev-link" href="{url}">{esc(ui["timeline"]["read"])} {ch["number"]:02d} · {esc(ch["title"])} →</a>'
            year = str(e["year"]) if "label" not in e else e["label"]
            kind = f' ev-{e["kind"]}' if e.get("kind") else ""
            items.append(f"""<li class="event ch-{cid or "none"}{kind}" data-chapter="{cid or ""}" id="y{e["year"]}-{slugify(e["title"])[:40]}">
  <span class="ev-year">{esc(year)}</span>
  <div class="ev-body">
    <h3>{transform(e["title"], site, lang, here, where)}</h3>
    <p class="ev-who">{esc(e.get("who", ""))}</p>
    <p>{transform(e["text"], site, lang, here, where)}</p>
    {link}
  </div>
</li>""")
        out.append(f"""<section class="era">
  <header class="era-head"><p class="era-years">{esc(era["label"])}</p><h2>{esc(era["title"])}</h2><p>{transform(era["text"], site, lang, here, where)}</p></header>
  <ol class="events">{"".join(items)}</ol>
</section>""")
    in_some = {id(e) for era in eras for e in events if era["from"] <= e["year"] < era["to"]}
    for e in events:
        if id(e) not in in_some:
            warn(f"{where}: event {e['year']} {e['title']!r} falls outside every era")

    main = f"""<article class="timeline">
<header class="page-head">
  <p class="kicker">{esc(ui["timeline"]["kicker"])}</p>
  <h1>{esc(data["title"])}</h1>
  <p class="dek">{transform(data["dek"], site, lang, here, where)}</p>
</header>
<div class="timeline-filter" data-demo="timeline/filter" role="group" aria-label="{esc(ui["timeline"]["filter"])}">{"".join(chips)}</div>
<div class="timeline-body">{"".join(out)}</div>
</article>"""
    return main, data, demo_scripts(main, where)


# --------------------------------------------------------------------------- pages

# Extra scripts a demo group needs loaded before it.
DEMO_DEPS = {
    "playground": ["js/data/examples-{lang}.js"],
}


def write_page(site: dict, lang: str, here: str, assets: dict[str, str], *, title: str, description: str,
               main: str, demos: list[str], body_class: str, current: str) -> None:
    ui = site["ui"][lang]
    template = (SRC / "template.html").read_text(encoding="utf-8")
    root = "../" * here.count("/")
    scripts = [assets["js/core.js"], assets["js/wahoo.js"]]
    for d in demos:
        for dep in DEMO_DEPS.get(d, []):
            dep = assets[dep.format(lang=lang)]
            if dep not in scripts:
                scripts.append(dep)
        scripts.append(assets[f"js/demos/{d}.js"])
    script_tags = "\n".join(f'<script src="{root}{s}" defer></script>' for s in scripts)
    strings = json.dumps({"lang": lang, **ui["demos"]}, ensure_ascii=False, separators=(",", ":"))
    canonical = site["base_url"] + here[: -len("index.html")]
    alternates = []
    for other in site["langs"]:
        alt = same_page(site, other, current)
        alternates.append(f'<link rel="alternate" hreflang="{other}" href="{esc(site["base_url"] + alt[: -len("index.html")])}">')
    default = same_page(site, site["default_lang"], current)
    alternates.append(f'<link rel="alternate" hreflang="x-default" href="{esc(site["base_url"] + default[: -len("index.html")])}">')
    page = (template
            .replace("{{lang}}", lang)
            .replace("{{title}}", esc(title))
            .replace("{{description}}", esc(re.sub(r"<[^>]+>", "", description)))
            .replace("{{site_title}}", esc(ui["site_title"]))
            .replace("{{canonical}}", esc(canonical))
            .replace("{{alternates}}", "\n".join(alternates))
            .replace("{{og_locale}}", OG_LOCALE.get(lang, lang))
            .replace("{{author}}", esc(site["author"]))
            .replace("{{favicon}}", root + assets["favicon.svg"])
            .replace("{{css}}", root + assets["css/main.css"])
            .replace("{{theme_js}}", root + assets["js/theme.js"])
            .replace("{{scripts}}", script_tags)
            .replace("{{body_class}}", body_class)
            .replace("{{strings}}", esc(strings))
            .replace("{{compiler}}", root + assets.get("wahoo.wasm", "assets/wahoo.wasm"))
            .replace("{{worker}}", root + assets["js/run-worker.js"])
            .replace("{{header}}", render_header(site, lang, here, current))
            .replace("{{main}}", main)
            .replace("{{footer}}", render_footer(site, lang)))
    dest = DIST / here
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(page, encoding="utf-8")


def check_anchors() -> None:
    """Every internal link with a #fragment must land on an existing id."""
    pages = {p: p.read_text(encoding="utf-8") for p in DIST.rglob("*.html")}
    for page, text in pages.items():
        for path, frag in re.findall(r'href="([^"#:]*)#([^"]+)"', text):
            target = (page.parent / path / "index.html").resolve() if path else page
            if target not in pages and target.resolve() not in {p.resolve() for p in pages}:
                warn(f"{page.relative_to(DIST)}: link to missing page {path}")
                continue
            if f'id="{frag}"' not in target.read_text(encoding="utf-8"):
                warn(f"{page.relative_to(DIST)}: link to missing anchor {path}#{frag}")


def build() -> None:
    if DIST.exists():
        shutil.rmtree(DIST)
    DIST.mkdir()
    site = load_site()
    check_demo_strings(site)
    assets = copy_assets(site)
    count = 0
    for lang in site["langs"]:
        ui = site["ui"][lang]

        here = page_path(site, lang, "home")
        main, demos = render_home(site, lang, here)
        write_page(site, lang, here, assets, title=ui["site_title"], description=ui["site_description"],
                   main=main, demos=demos, body_class="page-home", current="home")
        count += 1

        for page in ("playground", "reference"):
            here = page_path(site, lang, page)
            main, meta, demos = render_page(site, lang, page, here)
            write_page(site, lang, here, assets, title=f'{meta["title"]} · {ui["site_short"]}', description=meta["dek"],
                       main=main, demos=demos, body_class=f"page-{page}", current=page)
            count += 1

        here = page_path(site, lang, "timeline")
        main, data, demos = render_timeline(site, lang, here)
        write_page(site, lang, here, assets, title=f'{data["title"]} · {ui["site_short"]}', description=data["dek"],
                   main=main, demos=demos, body_class="page-timeline", current="timeline")
        count += 1

        for ch in site["chapter_list"][lang]:
            here = page_path(site, lang, "chapter", ch)
            main, demos = render_chapter(site, lang, ch, here)
            write_page(site, lang, here, assets, title=f'{ch["title"]}: {ch["headline"]} · {ui["site_short"]}',
                       description=ch["dek"], main=main, demos=demos,
                       body_class=f'page-chapter ch-{ch["id"]}', current=ch["id"])
            count += 1

    check_anchors()
    (DIST / ".nojekyll").write_text("", encoding="utf-8")
    print(f"built {count} pages into {DIST.relative_to(REPO)}/" + (f" with {len(WARNINGS)} warning(s)" if WARNINGS else ""))


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm",
                      ".js": "text/javascript"}


def serve(port: int) -> None:
    handler = functools.partial(Handler, directory=str(DIST))
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as httpd:
        print(f"serving on http://127.0.0.1:{port}/  (Ctrl+C to stop)")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            pass


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--release", action="store_true", help="fail on any warning")
    ap.add_argument("--serve", action="store_true", help="serve dist/ after building")
    ap.add_argument("--port", type=int, default=8000)
    args = ap.parse_args()
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    build()
    if args.release and WARNINGS:
        sys.exit(f"--release: {len(WARNINGS)} warning(s), refusing to publish")
    if args.serve:
        serve(args.port)


if __name__ == "__main__":
    main()
