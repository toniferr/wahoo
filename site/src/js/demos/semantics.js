// Semantics — a gallery of programs that parse fine but mean nothing (or something suspicious).
// The snippets are editable; the checker runs on every keystroke.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  var CASES = {
    en: [
      ["Unknown name", "world 1-1 {\n  coin lives = 3\n  wahoo(lifes)\n}\n"],
      ["Wrong type", "world 1-1 {\n  coin lives = 3\n  question lives {\n    wahoo(\"alive\")\n  }\n}\n"],
      ["Text plus coins", "world 1-1 {\n  coin name = \"Mario\"\n  wahoo(name + 1)\n}\n"],
      ["A type from elsewhere", "pipe double(n: int) -> coins {\n  flag n * 2\n}\n\nworld 1-1 {\n  wahoo(double(21))\n}\n"],
      ["Missing flag", "pipe sign(n: coins) -> coins {\n  question n > 0 {\n    flag 1\n  } else question n < 0 {\n    flag -1\n  }\n}\n\nworld 1-1 {\n  wahoo(sign(5))\n}\n"],
      ["Wrong arguments", "pipe add(a: coins, b: coins) -> coins {\n  flag a + b\n}\n\nworld 1-1 {\n  wahoo(add(1))\n  wahoo(add(1, star))\n}\n"],
      ["Warnings only", "world 1-1 {\n  coin unused = 10\n  coin zero = 0\n  wahoo(100 / 0)\n}\n"],
    ],
    es: [
      ["Nombre desconocido", "world 1-1 {\n  coin vidas = 3\n  wahoo(vida)\n}\n"],
      ["Tipo equivocado", "world 1-1 {\n  coin vidas = 3\n  question vidas {\n    wahoo(\"vivo\")\n  }\n}\n"],
      ["Texto más monedas", "world 1-1 {\n  coin nombre = \"Mario\"\n  wahoo(nombre + 1)\n}\n"],
      ["Un tipo de otro lenguaje", "pipe doble(n: int) -> coins {\n  flag n * 2\n}\n\nworld 1-1 {\n  wahoo(doble(21))\n}\n"],
      ["Falta un flag", "pipe signo(n: coins) -> coins {\n  question n > 0 {\n    flag 1\n  } else question n < 0 {\n    flag -1\n  }\n}\n\nworld 1-1 {\n  wahoo(signo(5))\n}\n"],
      ["Argumentos erróneos", "pipe suma(a: coins, b: coins) -> coins {\n  flag a + b\n}\n\nworld 1-1 {\n  wahoo(suma(1))\n  wahoo(suma(1, star))\n}\n"],
      ["Solo avisos", "world 1-1 {\n  coin sin_usar = 10\n  coin cero = 0\n  wahoo(100 / 0)\n}\n"],
    ],
  };

  WH.register("semantics/errors", function (stage) {
    var cases = CASES[WH.lang()] || CASES.en;
    var picker = h("div", { class: "lang-picker", role: "group", "aria-label": t("sem.pick") });
    stage.appendChild(picker);
    var ed = WHC.editor(stage, { value: cases[0][1], small: true, label: t("pg.editor"), delay: 120, onChange: update });
    var summary = h("p", { class: "pg-summary", role: "status" });
    var list = h("ol", { class: "diags" });
    stage.appendChild(summary);
    stage.appendChild(list);
    var buttons = cases.map(function (c, i) {
      var b = h("button", { type: "button", class: "chip", "aria-pressed": i === 0 ? "true" : "false", onclick: function () {
        buttons.forEach(function (x) { x.setAttribute("aria-pressed", x === b ? "true" : "false"); });
        ed.set(c[1]);
      } }, c[0]);
      picker.appendChild(b);
      return b;
    });
    var compiler = null;

    function update() {
      if (!compiler) return;
      var r = compiler.check(ed.get());
      summary.textContent = r.summary;
      summary.className = "pg-summary " + (r.ok ? "ok" : "bad");
      WHC.diagnosticsView(list, r.diagnostics, ed);
      ed.mark(r.diagnostics);
    }

    WHC.compiler().then(function (c) { compiler = c; update(); }, function () { summary.textContent = t("pg.loadError"); });
  });
})();
