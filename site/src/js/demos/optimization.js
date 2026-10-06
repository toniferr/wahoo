// Optimization — the same program compiled twice, without and with the optimizer, side by side.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  // Instructions in a WAT listing: indented lines that start with an opcode (not "(", ";;" or ")").
  function countInstructions(wat) {
    return wat.split("\n").filter(function (l) { return /^\s{4,}[a-z]/.test(l); }).length;
  }

  WH.register("optimization/diff", function (stage) {
    var ed = WHC.editor(stage, { value: t("opt.code"), small: true, label: t("pg.editor"), delay: 150, onChange: update });
    var stats = h("div", { class: "stats" });
    var before = h("div"), after = h("div");
    var sBefore = WH.stat(t("opt.before")), sAfter = WH.stat(t("opt.after"), "accent"), sWork = WH.stat(t("opt.work"));
    stats.appendChild(sBefore.el);
    stats.appendChild(sAfter.el);
    stats.appendChild(sWork.el);
    stage.appendChild(stats);
    stage.appendChild(h("div", { class: "code-pair" },
      h("div", {}, h("p", { class: "panel-label" }, t("opt.off")), before),
      h("div", {}, h("p", { class: "panel-label" }, t("opt.on")), after)));
    var diags = h("ol", { class: "diags" });
    stage.appendChild(diags);
    var compiler = null;

    function update() {
      if (!compiler) return;
      var src = ed.get();
      var off = compiler.check(src, { optimize: false });
      var on = compiler.check(src, { optimize: true });
      WHC.diagnosticsView(diags, on.diagnostics.filter(function (d) { return d.severity === "error"; }), ed);
      WH.clear(before);
      WH.clear(after);
      if (!on.ok) {
        sBefore.set("—"); sAfter.set("—"); sWork.set("—");
        return;
      }
      before.appendChild(WHC.codeBlock(off.wat, "wat"));
      after.appendChild(WHC.codeBlock(on.wat, "wat"));
      sBefore.set(t("opt.size", { n: countInstructions(off.wat), b: off.size }));
      sAfter.set(t("opt.size", { n: countInstructions(on.wat), b: on.size }));
      sWork.set(t("pg.stats", on.stats));
    }

    WHC.compiler().then(function (c) { compiler = c; update(); }, function () { sWork.set(t("pg.loadError")); });
  });
})();
