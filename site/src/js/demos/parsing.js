// Parsing — an expression goes in, the real parser's tree comes out, with how it groups the operators.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  var PRESETS = ["1 + 2 * 3", "(1 + 2) * 3", "10 - 4 - 3", "-a * b", "a < b + 1 and not c or d", "fib(n - 1) + fib(n - 2)", "1 < 2 < 3"];

  // The expression sits on line 2 of a tiny program, as the argument of wahoo(...).
  var PREFIX = "world 1-1 {\n  wahoo(";
  var SUFFIX = ")\n}\n";

  function parenthesize(n) {
    if (!n.children.length) return n.label;
    if (n.class === "call") return n.label.slice(0, -2) + "(" + n.children.map(parenthesize).join(", ") + ")";
    if (n.children.length === 1) return "(" + n.label + " " + parenthesize(n.children[0]) + ")";
    return "(" + parenthesize(n.children[0]) + " " + n.label + " " + parenthesize(n.children[1]) + ")";
  }

  WH.register("parsing/tree", function (stage) {
    var input = h("input", { type: "text", class: "expr-input", value: PRESETS[0], spellcheck: "false",
      "aria-label": t("parse.input"), autocomplete: "off" });
    var chips = h("div", { class: "lang-picker" }, PRESETS.map(function (p) {
      return h("button", { type: "button", class: "chip", onclick: function () { input.value = p; update(); } }, p);
    }));
    var reading = h("p", { class: "demo-hint", role: "status" });
    var tree = h("div");
    var errs = h("ol", { class: "diags" });
    stage.appendChild(input);
    stage.appendChild(chips);
    stage.appendChild(reading);
    stage.appendChild(tree);
    stage.appendChild(errs);
    var compiler = null;

    function update() {
      if (!compiler) return;
      var r = compiler.ast(PREFIX + input.value + SUFFIX);
      var errors = r.diagnostics.filter(function (d) { return d.severity === "error"; });
      input.classList.toggle("bad", errors.length > 0);
      WH.clear(errs);
      errors.forEach(function (d) { errs.appendChild(h("li", { class: "diag" }, h("span", { class: "diag-code" }, d.code), h("span"), h("span", { class: "diag-title" }, d.title))); });
      var world = r.ast.children[0];
      var call = world && world.children[0];
      var expr = call && call.children.length === 1 ? call.children[0] : null;
      if (!expr || errors.length) {
        WH.clear(tree);
        reading.textContent = "";
        return;
      }
      reading.textContent = t("parse.reads", { expr: parenthesize(expr) });
      WHC.astView(tree, expr);
    }

    input.addEventListener("input", update);
    WHC.compiler().then(function (c) { compiler = c; update(); }, function () { reading.textContent = t("pg.loadError"); });
  });
})();
