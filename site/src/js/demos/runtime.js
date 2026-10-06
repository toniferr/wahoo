// Runtime — the bytes of a compiled module, section by section, and the module running in a worker.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  WH.register("runtime/bytes", function (stage) {
    var ed = WHC.editor(stage, { value: t("rt.code"), small: true, label: t("pg.editor"), delay: 150, onChange: update, onSubmit: run });
    var runBtn = WH.button(t("pg.run"), run, "primary");
    var out = h("div");
    var dump = h("div");
    stage.appendChild(WH.controls(runBtn));
    stage.appendChild(out);
    stage.appendChild(dump);
    var compiler = null;

    function update() {
      if (!compiler) return;
      var r = compiler.wasm(ed.get());
      if (r.ok) {
        WHC.hexView(dump, r.bytes);
      } else {
        var list = h("ol", { class: "diags" });
        WHC.diagnosticsView(list, r.diagnostics, ed);
        WH.clear(dump).appendChild(list);
      }
    }

    function run() {
      if (!compiler) return;
      var r = compiler.wasm(ed.get());
      if (!r.ok) return;
      runBtn.disabled = true;
      WHC.run(r.bytes).then(function (res) {
        runBtn.disabled = false;
        WHC.outputView(out, res);
      });
    }

    WHC.compiler().then(function (c) { compiler = c; update(); }, function () { dump.textContent = t("pg.loadError"); });
  });
})();
