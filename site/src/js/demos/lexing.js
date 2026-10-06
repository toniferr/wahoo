// Lexing — type Wahoo and watch the real lexer cut it into tokens.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  WH.register("lexing/tokens", function (stage) {
    var stats = h("p", { class: "demo-hint", role: "status" });
    var stream = h("div");
    var diags = h("ol", { class: "diags" });
    var ed = WHC.editor(stage, { value: t("lex.code"), small: true, label: t("pg.editor"), delay: 60, onChange: update });
    stage.appendChild(stats);
    stage.appendChild(stream);
    stage.appendChild(diags);
    var compiler = null;

    function update() {
      if (!compiler) return;
      var src = ed.get();
      var r = compiler.tokens(src);
      WHC.tokensView(stream, r.tokens);
      WHC.diagnosticsView(diags, r.diagnostics, ed);
      ed.mark(r.diagnostics);
      stats.textContent = t("lex.stats", { chars: WH.fmt(src.length), tokens: WH.fmt(r.tokens.length) });
    }

    WHC.compiler().then(function (c) { compiler = c; update(); }, function () { stats.textContent = t("pg.loadError"); });
  });
})();
