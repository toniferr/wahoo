// The Wahoo compiler in the browser, and the widgets every demo shares.
//
// wahoo.wasm is the Rust compiler (crates/wahoo-web) built for wasm32: it is fetched once, on first use, and each
// phase is one call through a tiny C ABI (see crates/wahoo-web/src/lib.rs). Programs it compiles run in a Web
// Worker (js/run-worker.js) so an endless loop can be stopped without freezing the page.
(function () {
  "use strict";
  var h = WH.h, s = WH.s, t = WH.t;

  var TASK = { tokens: 0, ast: 1, check: 2, wasm: 3 };
  var loading = null;
  var utf8 = new TextEncoder();
  var utf8d = new TextDecoder();

  // ------------------------------------------------------------------ the compiler

  function instantiate(url) {
    if (WebAssembly.instantiateStreaming) {
      return WebAssembly.instantiateStreaming(fetch(url), {}).catch(function () {
        return fetch(url).then(function (r) { return r.arrayBuffer(); }).then(function (b) { return WebAssembly.instantiate(b, {}); });
      });
    }
    return fetch(url).then(function (r) { return r.arrayBuffer(); }).then(function (b) { return WebAssembly.instantiate(b, {}); });
  }

  function compiler() {
    if (!loading) {
      loading = instantiate(document.body.getAttribute("data-compiler")).then(function (res) {
        var ex = res.instance.exports;
        function call(src, task, opts) {
          var flags = ((opts && opts.optimize === false) ? 0 : 1) | (WH.lang() === "es" ? 2 : 0);
          var bytes = utf8.encode(src);
          var p = ex.alloc(bytes.length);
          new Uint8Array(ex.memory.buffer, p, bytes.length).set(bytes);
          var status = ex.compile(p, bytes.length, task, flags);
          var out = new Uint8Array(ex.memory.buffer, ex.out_ptr(), ex.out_len()).slice();
          ex.dealloc(p, bytes.length);
          return { status: status, bytes: out };
        }
        function json(src, task, opts) { return JSON.parse(utf8d.decode(call(src, task, opts).bytes)); }
        return {
          tokens: function (src) { return json(src, TASK.tokens); },
          ast: function (src) { return json(src, TASK.ast); },
          check: function (src, opts) { return json(src, TASK.check, opts); },
          wasm: function (src, opts) {
            var r = call(src, TASK.wasm, opts);
            return r.status === 0 ? { ok: true, bytes: r.bytes } : { ok: false, diagnostics: JSON.parse(utf8d.decode(r.bytes)) };
          },
        };
      });
    }
    return loading;
  }

  // ------------------------------------------------------------------ running programs

  // Runs a compiled module in a fresh worker. Resolves to { lines, error } where error is null or
  // { kind: "pit" | "overflow" | "stack" | "time" | "output" | "other", message }.
  function run(bytes, opts) {
    opts = opts || {};
    return new Promise(function (resolve) {
      var worker = new Worker(document.body.getAttribute("data-worker"));
      var lines = [];
      var done = false;
      function finish(error) {
        if (done) return;
        done = true;
        clearTimeout(timer);
        worker.terminate();
        resolve({ lines: lines, error: error });
      }
      var timer = setTimeout(function () { finish({ kind: "time", message: "" }); }, opts.timeout || 3000);
      worker.onmessage = function (e) {
        var m = e.data;
        if (m.type === "lines") {
          Array.prototype.push.apply(lines, m.lines);
          if (opts.onLines) opts.onLines(lines);
        } else if (m.type === "done") {
          finish(null);
        } else if (m.type === "trap") {
          finish({ kind: trapKind(m.message), message: m.message });
        }
      };
      worker.onerror = function (e) { finish({ kind: "other", message: e.message || "worker error" }); };
      worker.postMessage({ bytes: bytes, maxLines: opts.maxLines || 5000 });
    });
  }

  function trapKind(msg) {
    if (msg === "output") return "output";
    if (/divi\w* by zero|division by zero/i.test(msg)) return "pit";
    if (/call stack|recursion|stack overflow/i.test(msg)) return "stack";
    if (/unrepresentable|integer overflow/i.test(msg)) return "overflow";
    return "other";
  }

  function trapMessage(err) {
    var key = { pit: "run.pit", overflow: "run.overflow", stack: "run.stack", time: "run.time", output: "run.output" }[err.kind];
    return key ? t(key) : t("run.other", { msg: err.message });
  }

  // ------------------------------------------------------------------ syntax highlighting

  var KEYWORDS = /^(world|pipe|coin|power_up|damage|by|question|else|bounce|run|from|to|flag|and|or|not)$/;
  var WAHOO_RE = /(\/\/[^\n]*)|("(?:\\.|[^"\\\n])*"?)|(\b\d[\d_]*\b)|([A-Za-z_]\w*)|(->|==|!=|<=|>=|[-+*/%<>=])|([\s\S])/g;
  var WAT_RE = /(;;[^\n]*|\(;[^;]*;\))|("(?:\\.|[^"\\])*")|(\$[\w.]+)|(-?\b\d+\b)|([a-z_][\w.]*)|([\s\S])/g;
  var WAT_KEYWORDS = /^(module|func|param|result|local|import|export|memory|data|type|block|loop|if|else|end|i32)$/;

  function wahooClass(m) {
    if (m[1]) return "c";
    if (m[2]) return "s";
    if (m[3]) return "n";
    if (m[4]) {
      var w = m[4];
      if (KEYWORDS.test(w)) return "k";
      if (w === "star" || w === "goomba") return "b";
      if (w === "coins" || w === "switch" || w === "text") return "t";
      if (w === "wahoo") return "f";
      return null;
    }
    if (m[5]) return "o";
    return null;
  }

  function watClass(m) {
    if (m[1]) return "c";
    if (m[2]) return "s";
    if (m[3]) return "v";
    if (m[4]) return "n";
    if (m[5]) return WAT_KEYWORDS.test(m[5]) ? "k" : "i";
    return null;
  }

  // Appends highlighted code to `node`. Runs of plain text are merged into single text nodes.
  function highlight(node, code, lang) {
    var re = lang === "wat" ? WAT_RE : WAHOO_RE;
    var classify = lang === "wat" ? watClass : wahooClass;
    var plain = "";
    re.lastIndex = 0;
    var m;
    while ((m = re.exec(code))) {
      var cls = classify(m);
      if (!cls) { plain += m[0]; continue; }
      if (plain) { node.appendChild(document.createTextNode(plain)); plain = ""; }
      node.appendChild(h("span", { class: cls }, m[0]));
    }
    if (plain) node.appendChild(document.createTextNode(plain));
    return node;
  }

  function codeBlock(code, lang, cls) {
    return h("pre", { class: "code-block lang-" + lang + (cls ? " " + cls : "") }, highlight(h("code"), code, lang));
  }

  // ------------------------------------------------------------------ editor

  // A textarea over a highlighted <pre>: the textarea does the editing (caret, selection, undo, IME) with
  // transparent text, the <pre> underneath shows the colours. A gutter shows line numbers and error marks.
  function editor(parent, o) {
    var gutter = h("div", { class: "ed-gutter", "aria-hidden": "true" });
    var code = h("code");
    var pre = h("pre", { class: "ed-hl", "aria-hidden": "true" }, code);
    var ta = h("textarea", { class: "ed-input", spellcheck: "false", autocapitalize: "off", autocomplete: "off",
      autocorrect: "off", wrap: "off", "aria-label": o.label || "Wahoo" });
    var el = h("div", { class: "editor" + (o.small ? " editor-small" : "") }, gutter, h("div", { class: "ed-body" }, pre, ta));
    parent.appendChild(el);
    var marks = {};

    function paint() {
      WH.clear(code);
      highlight(code, ta.value + "\n", "wahoo");
      var n = ta.value.split("\n").length;
      WH.clear(gutter);
      for (var i = 1; i <= n; i++) {
        gutter.appendChild(h("span", { class: marks[i] ? "ln " + marks[i] : "ln" }, String(i)));
      }
      if (o.small) {
        // Small editors grow with their content (up to a limit) instead of scrolling.
        var lh = parseFloat(getComputedStyle(ta).lineHeight) || 21;
        el.style.height = Math.round(WH.clamp(n, 4, 18) * lh + 24) + "px";
      }
      sync();
    }
    function sync() {
      pre.scrollTop = ta.scrollTop;
      pre.scrollLeft = ta.scrollLeft;
      gutter.scrollTop = ta.scrollTop;
    }
    var timer = 0;
    ta.addEventListener("input", function () {
      paint();
      if (o.onChange) {
        clearTimeout(timer);
        timer = setTimeout(function () { o.onChange(ta.value); }, o.delay === undefined ? 200 : o.delay);
      }
    });
    ta.addEventListener("scroll", sync);
    ta.addEventListener("keydown", function (e) {
      if (e.key === "Tab" && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        var a = ta.selectionStart, b = ta.selectionEnd;
        ta.setRangeText("  ", a, b, "end");
        ta.dispatchEvent(new Event("input"));
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && o.onSubmit) {
        e.preventDefault();
        o.onSubmit();
      }
    });
    ta.value = o.value || "";
    paint();
    return {
      el: el,
      input: ta,
      get: function () { return ta.value; },
      set: function (v) { ta.value = v; marks = {}; paint(); if (o.onChange) o.onChange(v); },
      // diags: compiler diagnostics; marks their lines in the gutter.
      mark: function (diags) {
        marks = {};
        (diags || []).forEach(function (d) {
          if (marks[d.line] !== "err") marks[d.line] = d.severity === "error" ? "err" : "warn";
        });
        paint();
      },
      select: function (from, to) {
        ta.focus();
        ta.setSelectionRange(from, Math.max(to, from + 1));
      },
    };
  }

  // ------------------------------------------------------------------ views of each phase

  function diagnosticsView(parent, diags, ed) {
    WH.clear(parent);
    if (!diags.length) return;
    diags.forEach(function (d) {
      var item = h("li", { class: "diag diag-" + d.severity },
        h("span", { class: "diag-code" }, d.code),
        h("span", { class: "diag-where" }, d.line + ":" + d.col),
        h("span", { class: "diag-title" }, d.title),
        h("span", { class: "diag-label" }, d.label),
        d.hint ? h("span", { class: "diag-hint" }, d.hint) : null);
      if (ed) {
        item.tabIndex = 0;
        item.addEventListener("click", function () { ed.select(d.from, d.to); });
        item.addEventListener("keydown", function (e) { if (e.key === "Enter") ed.select(d.from, d.to); });
      }
      parent.appendChild(item);
    });
  }

  function tokensView(parent, tokens) {
    WH.clear(parent);
    var list = h("ol", { class: "tok-stream" });
    var lastLine = 1;
    tokens.forEach(function (tk) {
      if (tk.line !== lastLine) { list.appendChild(h("li", { class: "tok-break", "aria-hidden": "true" })); lastLine = tk.line; }
      var text = tk.class === "eof" ? "EOF" : tk.text;
      list.appendChild(h("li", { class: "tok tok-" + tk.class, title: tk.line + ":" + tk.col + " · " + t("tokens." + tk.class) },
        h("span", { class: "tok-text" }, text), h("span", { class: "tok-class" }, t("tokens." + tk.class))));
    });
    parent.appendChild(list);
  }

  // Tidy-ish tree layout: leaves get consecutive slots (as wide as their labels), parents sit over their children.
  function astView(parent, root, opts) {
    WH.clear(parent);
    opts = opts || {};
    var CH = 7.2, PAD = 14, GAP = 10, LEVEL = 56, BOX = 24;
    var nodes = [];
    var x = 0, maxDepth = 0;
    function widthOf(n) { return Math.max(34, n.label.length * CH + PAD); }
    function layout(n, depth) {
      var me = { n: n, depth: depth, w: widthOf(n), kids: [] };
      maxDepth = Math.max(maxDepth, depth);
      nodes.push(me);
      if (!n.children.length) {
        me.x = x + me.w / 2;
        x += me.w + GAP;
      } else {
        me.kids = n.children.map(function (c) { return layout(c, depth + 1); });
        var first = me.kids[0].x, last = me.kids[me.kids.length - 1].x;
        me.x = (first + last) / 2;
        if (me.x - me.w / 2 < 0) me.x = me.w / 2;
      }
      return me;
    }
    var top = layout(opts.skipRoot && root.children.length === 1 ? root.children[0] : root, 0);
    var W = Math.max(x, top.x + top.w / 2) + 4, H = (maxDepth + 1) * LEVEL;
    var svg = s("svg", { viewBox: "0 0 " + W + " " + H, width: W, height: H, class: "ast-svg", role: "img",
      "aria-label": t("ast.label") });
    var edges = s("g", { class: "ast-edges" });
    var boxes = s("g");
    nodes.forEach(function (m) {
      var y = m.depth * LEVEL + 4;
      m.kids.forEach(function (k) {
        edges.appendChild(s("line", { x1: m.x, y1: y + BOX, x2: k.x, y2: k.depth * LEVEL + 4 }));
      });
      var g = s("g", { class: "ast-node ast-" + m.n.class });
      g.appendChild(s("title", {}, m.n.label + " · " + m.n.line + ":" + m.n.col));
      g.appendChild(s("rect", { x: m.x - m.w / 2, y: y, width: m.w, height: BOX, rx: 6 }));
      g.appendChild(s("text", { x: m.x, y: y + 16, "text-anchor": "middle" }, m.n.label));
      if (opts.onHover) {
        g.addEventListener("mouseenter", function () { opts.onHover(m.n); });
        g.addEventListener("mouseleave", function () { opts.onHover(null); });
      }
      boxes.appendChild(g);
    });
    svg.appendChild(edges);
    svg.appendChild(boxes);
    parent.appendChild(h("div", { class: "ast-scroll" }, svg));
  }

  var SECTIONS = { 0: "custom", 1: "type", 2: "import", 3: "function", 5: "memory", 7: "export", 10: "code", 11: "data" };

  function leb(bytes, i) {
    var result = 0, shift = 0, b;
    do { b = bytes[i++]; result |= (b & 0x7f) << shift; shift += 7; } while (b & 0x80);
    return { value: result >>> 0, next: i };
  }

  // Splits a module into its header and sections: [{ name, id, start, end, payloadStart }].
  function sections(bytes) {
    var out = [{ name: "header", id: -1, start: 0, end: 8, payloadStart: 8 }];
    var i = 8;
    while (i < bytes.length) {
      var id = bytes[i];
      var size = leb(bytes, i + 1);
      out.push({ name: SECTIONS[id] || "?", id: id, start: i, end: size.next + size.value, payloadStart: size.next });
      i = size.next + size.value;
    }
    return out;
  }

  function hexView(parent, bytes) {
    WH.clear(parent);
    var secs = sections(bytes);
    var owner = new Array(bytes.length);
    secs.forEach(function (sec, k) { for (var i = sec.start; i < sec.end; i++) owner[i] = k; });
    var legend = h("table", { class: "hex-legend" },
      h("thead", {}, h("tr", {}, h("th", {}, t("bytes.section")), h("th", {}, t("bytes.offset")), h("th", {}, t("bytes.size")), h("th", {}, t("bytes.what")))),
      h("tbody", {}, secs.map(function (sec) {
        return h("tr", { class: "hx-" + sec.name },
          h("td", {}, h("span", { class: "hx-swatch" }), sec.id >= 0 ? sec.name + " (" + sec.id + ")" : t("bytes.header")),
          h("td", { class: "num" }, "0x" + sec.start.toString(16).padStart(4, "0")),
          h("td", { class: "num" }, String(sec.end - sec.start)),
          h("td", {}, t("bytes.sec." + sec.name)));
      })));
    var dump = h("div", { class: "hex-dump" });
    for (var row = 0; row < bytes.length; row += 16) {
      var line = h("div", { class: "hex-row" }, h("span", { class: "hex-off" }, row.toString(16).padStart(4, "0")));
      var ascii = "";
      for (var i = row; i < Math.min(row + 16, bytes.length); i++) {
        var sec = secs[owner[i]];
        var cls = "hx hx-" + sec.name + (i === sec.start ? " hx-start" : "");
        line.appendChild(h("span", { class: cls }, bytes[i].toString(16).padStart(2, "0")));
        ascii += bytes[i] >= 32 && bytes[i] < 127 ? String.fromCharCode(bytes[i]) : "·";
      }
      line.appendChild(h("span", { class: "hex-ascii" }, ascii));
      dump.appendChild(line);
    }
    parent.appendChild(h("p", { class: "demo-hint" }, t("bytes.total", { n: WH.fmt(bytes.length) })));
    parent.appendChild(legend);
    parent.appendChild(dump);
  }

  // Output of a run: printed lines, then the trap message if the program stopped early.
  function outputView(parent, result) {
    WH.clear(parent);
    var pre = h("pre", { class: "run-output" });
    pre.textContent = result.lines.join("\n");
    parent.appendChild(pre);
    if (result.error) parent.appendChild(h("p", { class: "run-trap" }, trapMessage(result.error)));
    else parent.appendChild(h("p", { class: "run-done" }, t("run.done")));
  }

  window.WHC = {
    compiler: compiler, run: run, trapMessage: trapMessage, highlight: highlight, codeBlock: codeBlock,
    editor: editor, diagnosticsView: diagnosticsView, tokensView: tokensView, astView: astView,
    sections: sections, hexView: hexView, outputView: outputView,
  };
})();
