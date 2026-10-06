// Home — a first taste: a tiny editor, the real compiler and a Run button.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  WH.register("home/try", function (stage) {
    var ed = WHC.editor(stage, { value: t("home.code"), small: true, label: t("pg.editor"), onSubmit: run, onChange: function () {} });
    var out = h("div");
    var btn = WH.button(t("pg.run"), run, "primary");
    stage.appendChild(WH.controls(btn, h("span", { class: "pg-note" }, t("home.hint"))));
    stage.appendChild(out);

    function run() {
      btn.disabled = true;
      WHC.compiler().then(function (c) {
        var r = c.wasm(ed.get());
        if (!r.ok) {
          btn.disabled = false;
          var list = h("ol", { class: "diags" });
          WHC.diagnosticsView(list, r.diagnostics, ed);
          WH.clear(out).appendChild(list);
          return null;
        }
        return WHC.run(r.bytes).then(function (res) {
          btn.disabled = false;
          WHC.outputView(out, res);
        });
      }, function () {
        btn.disabled = false;
        WH.clear(out).appendChild(h("p", { class: "run-trap" }, t("pg.loadError")));
      });
    }
  });
})();
