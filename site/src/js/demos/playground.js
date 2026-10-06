// Playground — the whole compiler at work: edit a program, look at every phase, run it.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  function toBase64Url(str) {
    var bytes = new TextEncoder().encode(str);
    var bin = "";
    for (var i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  function fromBase64Url(b64) {
    try {
      var bin = atob(b64.replace(/-/g, "+").replace(/_/g, "/"));
      var bytes = new Uint8Array(bin.length);
      for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      return new TextDecoder().decode(bytes);
    } catch (e) {
      return null;
    }
  }

  function codeFromHash() {
    var m = /^#code=([A-Za-z0-9_-]+)$/.exec(location.hash);
    return m ? fromBase64Url(m[1]) : null;
  }

  WH.register("playground/main", function (root) {
    var examples = window.WAHOO_EXAMPLES || [];
    var shared = codeFromHash();
    var optimize = true;
    var compiler = null;
    var current = "output";
    var lastRun = null;

    var options = examples.map(function (e) { return { value: e.id, label: e.title }; });
    if (shared) options.unshift({ value: "shared", label: t("pg.shared") });
    var exSel = WH.select({
      label: t("pg.example"), options: options, value: shared ? "shared" : (examples[0] && examples[0].id),
      onChange: function (id) {
        var ex = examples.filter(function (e) { return e.id === id; })[0];
        if (ex) { lastRun = null; ed.set(ex.code); }
      },
    });
    var optToggle = WH.toggle({ label: t("pg.optimize"), checked: true, onChange: function (v) { optimize = v; refresh(); } });
    var shareBtn = WH.button(t("pg.share"), share);
    var runBtn = WH.button(t("pg.run"), run, "primary");
    var shareNote = h("span", { class: "pg-note", role: "status" });
    root.appendChild(h("div", { class: "pg-bar" }, exSel.el, optToggle.el, h("span", { class: "spacer" }), shareNote, shareBtn, runBtn));

    var left = h("div");
    var right = h("div");
    root.appendChild(h("div", { class: "pg-grid" }, left, right));

    var ed = WHC.editor(left, {
      value: shared || (examples[0] && examples[0].code) || "",
      label: t("pg.editor"),
      onChange: function () { lastRun = null; refresh(); },
      onSubmit: run,
    });
    var summary = h("p", { class: "pg-summary", role: "status" }, t("pg.loading"));
    var diags = h("ol", { class: "diags" });
    left.appendChild(summary);
    left.appendChild(diags);
    left.appendChild(h("p", { class: "pg-note" }, t("pg.keys")));

    var tabNames = ["output", "tokens", "ast", "wat", "bytes"];
    var tabBar = h("div", { class: "tabs", role: "tablist" });
    var panel = h("div", { class: "tab-panel", role: "tabpanel" });
    var tabs = tabNames.map(function (name) {
      var b = h("button", { type: "button", class: "tab", role: "tab", "aria-selected": name === current ? "true" : "false",
        onclick: function () { select(name); } }, t("pg.tab." + name));
      tabBar.appendChild(b);
      return b;
    });
    right.appendChild(tabBar);
    right.appendChild(panel);

    function select(name) {
      current = name;
      tabs.forEach(function (b, i) { b.setAttribute("aria-selected", tabNames[i] === name ? "true" : "false"); });
      renderTab();
    }

    var checked = null;

    function refresh() {
      if (!compiler) return;
      checked = compiler.check(ed.get(), { optimize: optimize });
      summary.textContent = checked.summary + (checked.ok ? " · " + t("pg.size", { n: WH.fmt(checked.size) }) : "");
      summary.className = "pg-summary " + (checked.ok ? "ok" : "bad");
      if (checked.ok && checked.stats && optimize) {
        var st = checked.stats;
        if (st.folded + st.branches + st.dead > 0) summary.textContent += " · " + t("pg.stats", st);
      }
      WHC.diagnosticsView(diags, checked.diagnostics, ed);
      ed.mark(checked.diagnostics);
      renderTab();
    }

    function renderTab() {
      if (!compiler) return;
      var src = ed.get();
      WH.clear(panel);
      if (current === "tokens") {
        WHC.tokensView(panel, compiler.tokens(src).tokens);
      } else if (current === "ast") {
        WHC.astView(panel, compiler.ast(src).ast);
      } else if (current === "wat") {
        if (checked && checked.wat) panel.appendChild(WHC.codeBlock(checked.wat, "wat"));
        else panel.appendChild(h("p", { class: "pg-note" }, t("pg.fixFirst")));
      } else if (current === "bytes") {
        var r = compiler.wasm(src, { optimize: optimize });
        if (r.ok) WHC.hexView(panel, r.bytes);
        else panel.appendChild(h("p", { class: "pg-note" }, t("pg.fixFirst")));
      } else if (lastRun) {
        WHC.outputView(panel, lastRun);
      } else {
        panel.appendChild(h("p", { class: "pg-note" }, t("pg.pressRun")));
      }
    }

    function run() {
      if (!compiler) return;
      var r = compiler.wasm(ed.get(), { optimize: optimize });
      if (current !== "output") select("output");
      WH.clear(panel);
      if (!r.ok) {
        panel.appendChild(h("p", { class: "run-trap" }, t("pg.cantRun")));
        return;
      }
      var pre = h("pre", { class: "run-output" });
      panel.appendChild(pre);
      panel.appendChild(h("p", { class: "pg-note" }, t("run.running")));
      runBtn.disabled = true;
      WHC.run(r.bytes, {
        timeout: 3000,
        onLines: function (lines) { pre.textContent = lines.join("\n"); },
      }).then(function (res) {
        runBtn.disabled = false;
        lastRun = res;
        if (current === "output") WHC.outputView(panel, res);
      });
    }

    function share() {
      var url = location.href.split("#")[0] + "#code=" + toBase64Url(ed.get());
      history.replaceState(null, "", url);
      var done = function () { shareNote.textContent = t("pg.copied"); };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(url).then(done, function () { shareNote.textContent = t("pg.inAddressBar"); });
      } else {
        shareNote.textContent = t("pg.inAddressBar");
      }
    }

    WHC.compiler().then(function (c) {
      compiler = c;
      refresh();
    }, function (err) {
      console.error(err);
      summary.textContent = t("pg.loadError");
      summary.className = "pg-summary bad";
    });
  });
})();
