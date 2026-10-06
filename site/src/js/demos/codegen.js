// Code generation — compile an expression with the real compiler, then run the WebAssembly it produced on a
// little stack machine, one instruction at a time, watching the operand stack.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  var PRESETS = ["(a + b) * c", "a + b * c", "a * a - b * b", "(a - b) / c", "a < b and b < c", "not (a == b) or c > 10"];

  // Instructions of $f from the compiler's WAT (comments and local declarations dropped).
  function extract(wat) {
    var lines = wat.split("\n");
    var start = lines.findIndex(function (l) { return /^\s*\(func \$f\b/.test(l); });
    var out = [];
    for (var i = start + 1; start >= 0 && i < lines.length && lines[i] !== "  )"; i++) {
      var s = lines[i].trim();
      if (s && s.slice(0, 2) !== ";;" && s[0] !== "(") out.push({ text: s, indent: lines[i].length - lines[i].trimStart().length - 4 });
    }
    return out;
  }

  // Runs the listing; returns the list of states after each step.
  function simulate(code, env) {
    var match = {}, open = [];
    code.forEach(function (ins, i) {
      var op = ins.text.split(" ")[0];
      if (op === "if") open.push({ at: i, els: -1 });
      else if (op === "else") open[open.length - 1].els = i;
      else if (op === "end") { var o = open.pop(); match[o.at] = o; o.end = i; if (o.els >= 0) match[o.els] = o; }
    });
    var stack = [], pc = 0, states = [], guard = 0;
    while (pc < code.length && guard++ < 500) {
      var parts = code[pc].text.split(" ");
      var op = parts[0], arg = parts[1];
      var next = pc + 1, pushed = false, trap = null, done = false;
      var b, a;
      switch (op) {
        case "i32.const": stack.push(parseInt(arg, 10)); pushed = true; break;
        case "local.get": stack.push(env[arg.replace("$", "")] | 0); pushed = true; break;
        case "i32.eqz": stack.push(stack.pop() === 0 ? 1 : 0); pushed = true; break;
        case "if":
          if (stack.pop() === 0) next = match[pc].els >= 0 ? match[pc].els + 1 : match[pc].end + 1;
          break;
        case "else": next = match[pc].end + 1; break;
        case "end": break;
        case "return": case "unreachable": done = true; break;
        default:
          b = stack.pop(); a = stack.pop(); pushed = true;
          switch (op) {
            case "i32.add": stack.push((a + b) | 0); break;
            case "i32.sub": stack.push((a - b) | 0); break;
            case "i32.mul": stack.push(Math.imul(a, b)); break;
            case "i32.div_s": if (b === 0) trap = "pit"; else stack.push((a / b) | 0); break;
            case "i32.rem_s": if (b === 0) trap = "pit"; else stack.push((a % b) | 0); break;
            case "i32.eq": stack.push(a === b ? 1 : 0); break;
            case "i32.ne": stack.push(a !== b ? 1 : 0); break;
            case "i32.lt_s": stack.push(a < b ? 1 : 0); break;
            case "i32.gt_s": stack.push(a > b ? 1 : 0); break;
            case "i32.le_s": stack.push(a <= b ? 1 : 0); break;
            case "i32.ge_s": stack.push(a >= b ? 1 : 0); break;
            default: trap = "unsupported";
          }
      }
      states.push({ pc: pc, stack: stack.slice(), pushed: pushed, trap: trap });
      if (trap || done) break;
      pc = next;
    }
    return states;
  }

  WH.register("codegen/stack", function (stage) {
    var input = h("input", { type: "text", class: "expr-input", value: PRESETS[0], spellcheck: "false", autocomplete: "off",
      "aria-label": t("cg.input") });
    var chips = h("div", { class: "lang-picker" }, PRESETS.map(function (p) {
      return h("button", { type: "button", class: "chip", onclick: function () { input.value = p; compile(); } }, p);
    }));
    var env = { a: 6, b: 4, c: 7 };
    var vars = h("div", { class: "sm-vars" }, ["a", "b", "c"].map(function (name) {
      var box = h("input", { type: "number", value: env[name], step: "1" });
      box.addEventListener("input", function () { env[name] = parseInt(box.value, 10) || 0; reset(); });
      return h("label", {}, name, " =", box);
    }));
    var listing = h("ol", { class: "sm-code", "aria-label": t("cg.code") });
    var stackBox = h("div", { class: "sm-stack", "aria-label": t("cg.stack"), "aria-live": "polite" });
    var status = h("p", { class: "demo-hint", role: "status" });
    var stepBtn = WH.button(t("cg.step"), step, "primary");
    var runBtn = WH.button(t("cg.run"), function () { while (pos < states.length - 1) pos++; paint(); });
    var resetBtn = WH.button(t("cg.reset"), reset);
    stage.appendChild(input);
    stage.appendChild(chips);
    stage.appendChild(vars);
    stage.appendChild(h("div", { class: "sm" },
      h("div", {}, h("p", { class: "panel-label" }, t("cg.code")), listing),
      h("div", {}, h("p", { class: "panel-label" }, t("cg.stack")), stackBox)));
    stage.appendChild(WH.controls(stepBtn, runBtn, resetBtn));
    stage.appendChild(status);

    var compiler = null, code = [], states = [], pos = -1, isSwitch = false;

    function compile() {
      if (!compiler) return;
      var src = function (ty) { return "pipe f(a: coins, b: coins, c: coins) -> " + ty + " {\n  flag " + input.value + "\n}\nworld 1-1 { }\n"; };
      var r = compiler.check(src("coins"), { optimize: false });
      isSwitch = false;
      if (!r.ok && r.diagnostics.some(function (d) { return d.code === "E209"; })) {
        var r2 = compiler.check(src("switch"), { optimize: false });
        if (r2.ok) { r = r2; isSwitch = true; }
      }
      input.classList.toggle("bad", !r.ok);
      if (!r.ok) {
        var first = r.diagnostics.filter(function (d) { return d.severity === "error"; })[0];
        code = [];
        states = [];
        pos = -1;
        WH.clear(listing);
        WH.clear(stackBox);
        status.textContent = first ? first.title : "";
        return;
      }
      code = extract(r.wat);
      if (code.some(function (c) { return c.text.slice(0, 4) === "call"; })) {
        status.textContent = t("cg.noCalls");
      }
      reset();
    }

    function reset() {
      states = simulate(code, env);
      pos = -1;
      paint();
    }

    function step() {
      if (pos < states.length - 1) pos++;
      paint();
    }

    function show(v) { return isSwitch && pos === states.length - 1 ? v + (v ? " (star)" : " (goomba)") : String(v); }

    function paint() {
      WH.clear(listing);
      var st = pos >= 0 ? states[pos] : null;
      code.forEach(function (ins, i) {
        var cls = st && i === st.pc ? "pc" : "";
        listing.appendChild(h("li", { class: cls }, " ".repeat(Math.max(0, ins.indent)) + ins.text));
      });
      WH.clear(stackBox);
      var stack = st ? st.stack : [];
      if (!stack.length) stackBox.appendChild(h("p", { class: "sm-empty" }, t("cg.empty")));
      stack.forEach(function (v, i) {
        stackBox.appendChild(h("div", { class: "sm-cell" + (st.pushed && i === stack.length - 1 ? " new" : "") }, show(v)));
      });
      stepBtn.disabled = runBtn.disabled = pos >= states.length - 1;
      if (st && st.trap === "pit") status.textContent = t("run.pit");
      else if (pos === states.length - 1 && st) status.textContent = t("cg.result", { v: show(stack[stack.length - 1]), n: states.length });
      else status.textContent = t("cg.progress", { i: pos + 1, n: states.length });
    }

    input.addEventListener("input", compile);
    WHC.compiler().then(function (c) { compiler = c; compile(); }, function () { status.textContent = t("pg.loadError"); });
  });
})();
