// Home — the title screen. A line of Wahoo goes through the real compiler as a tiny World 1-1 level: a bot bumps a
// ? block and every hit pops one token from the real lexer; the tokens grow into the real syntax tree, joined by
// pipes; the checker stamps every node; the tree goes down a warp pipe and comes out as the real WebAssembly bytes,
// stacked as a staircase; and the browser runs them: the flag goes up and the program's output appears over the
// castle. Visitors can type their own line. Everything is drawn on one canvas; no game assets are used.
(function () {
  "use strict";
  var h = WH.h, t = WH.t;

  // Keywords stay in English in both languages (as in the game); names and texts are translated.
  var SNIPPETS = {
    en: [
      ['coin purse = 7 * 6', 'wahoo("purse = ", purse)'],
      ['coin lives = 3 + 2 * 4', 'wahoo("lives: ", lives)'],
      ['question 10 > 3 { wahoo("1-UP!") }'],
      ['coin jump = (2 + 3) * 8', 'wahoo("jump ", jump, "!")'],
      ['coin stars = 120 / 4 - 5', 'wahoo(stars, " stars")'],
    ],
    es: [
      ['coin monedero = 7 * 6', 'wahoo("monedero = ", monedero)'],
      ['coin vidas = 3 + 2 * 4', 'wahoo("vidas: ", vidas)'],
      ['question 10 > 3 { wahoo("¡1-UP!") }'],
      ['coin salto = (2 + 3) * 8', 'wahoo("salto de ", salto, "!")'],
      ['coin estrellas = 120 / 4 - 5', 'wahoo(estrellas, " estrellas")'],
    ],
  };
  var PHASES = ["lex", "parse", "check", "gen", "run"];

  // Pixel sprites: one character per pixel, "." is transparent. The bot is an original character.
  var BOT = {
    body: [
      ".....a......",
      ".....a......",
      "...oooooo...",
      "..oooooooo..",
      ".oowwoowwoo.",
      ".oowkoowkoo.",
      ".oooooooooo.",
      ".ooommmmooo.",
      "..oooooooo..",
      "...bbbbbb...",
      "..bbbbbbbb..",
    ],
    run1: ["..ff....ff..", ".fff....fff."],
    run2: ["...ff..ff...", "..fff..fff.."],
    jump: ["..ff....ff..", "..ff....ff.."],
  };
  var QMARK = [".wwww.", "ww..ww", "....ww", "...ww.", "..ww..", "..ww..", "......", "..ww.."];

  function clamp(x, lo, hi) { return Math.max(lo, Math.min(hi, x)); }
  function lerp(a, b, k) { return a + (b - a) * k; }
  function ease(x) { x = clamp(x, 0, 1); return x < 0.5 ? 2 * x * x : 1 - Math.pow(-2 * x + 2, 2) / 2; }
  function easeOut(x) { x = clamp(x, 0, 1); return 1 - Math.pow(1 - x, 3); }
  function hex(b) { return (b < 16 ? "0" : "") + b.toString(16); }

  // ------------------------------------------------------------------ preparing a level with the real compiler

  function programOf(lines) { return "world 1-1 {\n" + lines.map(function (l) { return "  " + l; }).join("\n") + "\n}\n"; }

  // Without the compiler (it failed to load), a rough tokenizer keeps the title screen alive.
  var ROUGH = /("(?:\\.|[^"\\])*"?)|(\d+)|([A-Za-z_]\w*)|(->|==|!=|<=|>=|\S)/g;
  var KW = /^(world|pipe|coin|power_up|damage|by|question|else|bounce|run|from|to|flag|and|or|not|star|goomba)$/;
  function roughLevel(lines) {
    var toks = [];
    lines.forEach(function (line, i) {
      var m;
      ROUGH.lastIndex = 0;
      while ((m = ROUGH.exec(line))) {
        toks.push({ text: m[0], cls: m[1] ? "text" : m[2] ? "number" : m[3] ? (KW.test(m[3]) ? "keyword" : "name") : "symbol",
          line: i, col: m.index });
      }
    });
    var tree = { label: "world 1-1", class: "item", children: lines.map(function (l) {
      return { label: l.split(" ")[0], class: "stmt", children: [] };
    }) };
    return { lines: lines, tokens: toks, tree: tree, bytes: [0, 0x61, 0x73, 0x6d, 1, 0, 0, 0], output: null, error: null };
  }

  function prepare(lines) {
    var src = programOf(lines);
    return WHC.compiler().then(function (c) {
      var toks = c.tokens(src).tokens.filter(function (tk) {
        return tk.class !== "eof" && tk.line >= 2 && tk.line <= lines.length + 1;
      }).map(function (tk) { return { text: tk.text, cls: tk.class, line: tk.line - 2, col: tk.col - 3 }; });
      var ast = c.ast(src).ast;
      var out = c.wasm(src);
      var lvl = { lines: lines, tokens: toks, tree: ast && ast.children[0], bytes: out.ok ? out.bytes : null, output: null, error: null };
      if (!out.ok) {
        lvl.error = out.diagnostics.filter(function (d) { return d.severity === "error"; })[0] || out.diagnostics[0];
        return lvl;
      }
      return WHC.run(out.bytes, { timeout: 2000, maxLines: 10 }).then(function (res) {
        lvl.output = res.error ? [WHC.trapMessage(res.error)] : res.lines.slice(0, 3);
        return lvl;
      });
    }).catch(function () { return roughLevel(lines); });
  }

  // A visitor's line: a `coin` declaration also prints its value, so there is always something to show.
  function linesFor(input) {
    var line = input.replace(/\s+/g, " ").trim().slice(0, 80);
    var m = /^coin\s+([A-Za-z_]\w*)/.exec(line);
    return m ? [line, 'wahoo("' + m[1] + ' = ", ' + m[1] + ")"] : [line];
  }

  WH.register("splash/level", function (root) {
    var canvas = root.querySelector(".splash-canvas");
    var copy = root.querySelector(".splash-copy");
    var ctx = canvas.getContext("2d");
    var reduce = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    var lang = WH.lang() === "es" ? "es" : "en";
    var snippets = SNIPPETS[lang];
    var snippetIndex = 0;

    // ---------------------------------------------------------------- HUD (real text, for everyone)

    var phaseEls = {};
    var caption = h("p", { class: "splash-caption" });
    var pauseBtn = h("button", { type: "button", class: "splash-btn", "aria-pressed": "false" });
    var nextBtn = h("button", { type: "button", class: "splash-btn" }, t("splash.next"));
    var hud = h("div", { class: "splash-hud" },
      h("ol", { class: "splash-phases", "aria-label": t("splash.phasesLabel") }, PHASES.map(function (p, i) {
        phaseEls[p] = h("li", {}, h("span", { class: "n" }, String(i + 1)), t("splash.phase." + p));
        return phaseEls[p];
      })),
      caption,
      h("div", { class: "splash-ctrls" }, pauseBtn, nextBtn));
    root.appendChild(hud);
    canvas.setAttribute("role", "img");
    canvas.setAttribute("aria-label", t("splash.label"));

    var input = root.querySelector(".splash-try input");
    var goBtn = root.querySelector(".splash-try button");

    // ---------------------------------------------------------------- colours and layout

    var C = {};
    function readColors() {
      var cs = getComputedStyle(document.documentElement);
      var v = function (n) { return cs.getPropertyValue(n).trim(); };
      var dark = document.documentElement.getAttribute("data-theme") === "dark";
      C = {
        dark: dark,
        mono: v("--mono") || "monospace",
        skyTop: dark ? "#070b1a" : "#e4f1ff",
        skyBot: dark ? "#1d2b58" : "#9fd0ff",
        cloud: dark ? "rgba(120,140,200,0.22)" : "rgba(255,255,255,0.95)",
        hill: dark ? "#163a30" : "#7fcf6c",
        hillEdge: dark ? "#0f2a22" : "#4f9f45",
        bush: dark ? "#1d4a37" : "#5dbb4c",
        brick: dark ? "#5a2f1c" : "#c8642e",
        mortar: dark ? "#2a140a" : "#7a3410",
        brickHi: dark ? "#7d4428" : "#ec955e",
        block: v("--block") || "#f4b400",
        blockEdge: v("--block-edge") || "#8a4b08",
        used: dark ? "#6b4a2a" : "#a8693a",
        pipe: dark ? "#23884a" : "#2fa84f",
        pipeHi: dark ? "#5fcf86" : "#9ae6a8",
        pipeEdge: "#0f3d22",
        node: dark ? "#2a2116" : "#fff8e6",
        nodeEdge: dark ? "#d99a00" : "#8a4b08",
        nodeText: dark ? "#f5e6c8" : "#3b2408",
        panel: "rgba(16,18,24,0.9)",
        panelEdge: dark ? "rgba(255,255,255,0.14)" : "rgba(0,0,0,0.25)",
        syn: { keyword: "#ff8a7a", name: "#7fb6f5", number: "#f0b45e", text: "#8fd694", symbol: "#c9d1db" },
        good: "#6cc48e",
        bad: "#f07a63",
        castle: dark ? "#3a3f4f" : "#9c8f86",
        castleEdge: dark ? "#22252f" : "#5d524b",
      };
    }

    var L = {};
    function layout() {
      var r = canvas.getBoundingClientRect();
      var W = Math.max(280, r.width), H = Math.max(300, r.height);
      var dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      var P = clamp(Math.floor(Math.min(H / 210, W / 300)), 2, 4);
      var T = 16 * P;
      var groundY = H - 2 * T;
      var cr = copy.getBoundingClientRect();
      var overlay = cr.bottom > r.top + 10 && cr.top < r.bottom;   // the copy sits on top of the canvas (wide screens)
      var right0 = overlay ? Math.max(cr.right - r.left + 32, W * 0.5) : 16;
      var fs = clamp(Math.round(T * 0.4), 11, 15);
      L = { W: W, H: H, P: P, T: T, groundY: groundY, bandTop: groundY - 3 * T, fs: fs, right0: right0, right1: W - 20,
        wide: W >= 640 };
      L.font = "600 " + fs + "px " + C.mono;
      L.small = "600 " + Math.max(10, fs - 2) + "px " + C.mono;
      // Ground objects, from the right edge: castle, flag pole, a staircase of bytes, the warp pipe.
      L.castleX = L.wide ? W - 4.2 * T : W + T;
      L.poleX = L.wide ? L.castleX - 1.1 * T : W - 1.1 * T;
      L.steps = L.wide ? (W >= 1100 ? 6 : 5) : 4;
      L.B = clamp(Math.round(T * 0.72), 18, 30);
      L.stairs1 = L.poleX - 0.5 * T;
      L.stairs0 = L.stairs1 - L.steps * L.B;
      L.pipeW = 2 * T;
      L.pipeX = L.stairs0 - 0.9 * T - L.pipeW;
      L.pipeTop = groundY - 1.6 * T;
      L.blockX = Math.min(right0 + 0.8 * T, L.pipeX - 3 * T);
      L.blockY = groundY - 3 * T;
      // The code sign and the token belt sit on top of the right column; the tree fills what is left above the band.
      L.signX = right0;
      L.signW = Math.min(L.right1 - right0, 560);
      L.signY = overlay ? 24 : 14;
      L.lineH = Math.round(fs * 1.55);
      L.signPad = 12;
      return L;
    }

    // ---------------------------------------------------------------- the level being played

    var level = null, nextLevel = null;
    var phase = "idle", pt = 0, clock = 0, paused = false, visible = true, running = false, skipNext = false;
    var bot, block, coins, chips, nodes, bricks, sparks, fireworks, flag, banner, typed, fade;

    function measure(text, font) { ctx.font = font; return ctx.measureText(text).width; }

    function signHeight() { return L.signPad * 2 + 18 + level.lines.length * L.lineH; }

    function setupLevel(lvl) {
      level = lvl;
      var startX = bot ? clamp(bot.x, 0, L.W) : L.blockX - 2.5 * L.T;
      bot = { x: startX, y: L.groundY, jumpT: -1, frame: 0, face: 1 };
      block = { bump: 0, used: false, hits: 0 };
      coins = []; sparks = []; fireworks = []; bricks = [];
      flag = 0; banner = 0; typed = 0; fade = 1;
      // Token chips: their slot on the belt under the sign.
      var beltY = L.signY + signHeight() + 12;
      var x = L.signX, y = beltY, chipH = L.fs + 12, maxX = L.signX + L.signW;
      chips = level.tokens.map(function (tk) {
        var w = measure(tk.text, L.font) + 14;
        if (x + w > maxX && x > L.signX) { x = L.signX; y += chipH + 6; }
        var c = { tok: tk, w: w, h: chipH, sx: x, sy: y, x: 0, y: 0, born: -1, state: "hidden" };
        x += w + 6;
        return c;
      });
      L.beltBottom = (chips.length ? chips[chips.length - 1].sy + chipH : beltY) + 10;
      layoutTree();
    }

    // Tidy tree layout (as in the parsing chapter), scaled to fit between the belt and the ground band.
    function layoutTree() {
      nodes = [];
      if (!level.tree) return;
      var fs = L.fs, gap = 10, x = 0, maxDepth = 0;
      function walk(n, depth, parent) {
        var me = { label: n.label, cls: n.class, line: n.line, col: n.col, depth: depth, parent: parent, kids: [],
          w: measure(n.label, L.font) + 16 };
        maxDepth = Math.max(maxDepth, depth);
        nodes.push(me);
        if (!n.children.length) { me.x = x + me.w / 2; x += me.w + gap; }
        else {
          me.kids = n.children.map(function (c) { return walk(c, depth + 1, me); });
          me.x = (me.kids[0].x + me.kids[me.kids.length - 1].x) / 2;
        }
        return me;
      }
      walk(level.tree, 0, null);
      var treeW = x - gap, nodeH = fs + 12;
      var top = L.beltBottom + 6, bottom = L.bandTop - 10;
      var levelH = clamp((bottom - top - nodeH) / Math.max(1, maxDepth), nodeH + 6, nodeH * 2.6);
      var right = L.wide ? Math.min(L.right1, L.poleX - 1.4 * L.T) : L.right1;
      var avail = right - L.right0;
      var scale = Math.min(1, avail / treeW, (bottom - top) / (maxDepth * levelH + nodeH));
      L.treeScale = Math.max(0.5, scale);
      var x0 = L.right0 + (avail - treeW * L.treeScale) / 2;
      nodes.forEach(function (n) {
        n.cx = x0 + n.x * L.treeScale;
        n.cy = top + n.depth * levelH * L.treeScale + nodeH / 2;
        n.bw = n.w * L.treeScale; n.bh = nodeH * Math.max(0.75, L.treeScale);
        n.at = -1; n.ok = -1; n.gone = -1;
      });
      L.treeDepth = maxDepth;
    }

    // ---------------------------------------------------------------- phase script

    function go(p) { phase = p; pt = 0; updateHud(); }

    function updateHud() {
      var current = phase === "type" ? "lex" : phase === "rest" || phase === "fade" ? (level && level.error ? null : "run") : phase;
      PHASES.forEach(function (p) {
        var el = phaseEls[p];
        if (p === current) el.setAttribute("aria-current", "step"); else el.removeAttribute("aria-current");
        el.classList.toggle("done", !!current && PHASES.indexOf(p) < PHASES.indexOf(current));
        el.classList.toggle("bad", !!(level && level.error) && phase === "error" && p === errorPhase());
      });
      var msg = "";
      if (!level) msg = "";
      else if (phase === "error") msg = t("splash.cap.error", { msg: level.error.title + (level.error.label ? ": " + level.error.label : "") });
      else if (phase === "type" || phase === "lex") msg = t("splash.cap.lex", { n: level.tokens.length });
      else if (phase === "parse") msg = t("splash.cap.parse", { n: nodes.length });
      else if (phase === "check") msg = t("splash.cap.check");
      else if (phase === "gen") msg = t("splash.cap.gen", { n: level.bytes ? WH.fmt(level.bytes.length) : 0 });
      else msg = t("splash.cap.run");
      caption.textContent = msg;
      caption.classList.toggle("bad", phase === "error");
    }

    function errorPhase() {
      var code = level.error && level.error.code || "";
      var n = parseInt(code.replace(/\D/g, ""), 10) || 0;
      return n < 100 ? "lex" : n < 200 ? "parse" : "check";
    }

    var HIT = 0.36;
    function hitEvery() { return Math.min(HIT, 4.2 / Math.max(1, chips.length)); }

    function step(dt) {
      pt += dt; clock += dt;
      var T = L.T;
      block.bump = Math.max(0, block.bump - dt * 6);

      if (phase === "type") {
        typed = Math.min(1, pt / (0.025 * level.lines.join(" ").length + 0.3));
        walkBot(dt, L.blockX + T / 2);
        if (typed >= 1 && Math.abs(bot.x - (L.blockX + T / 2)) < 2) go("lex");
      } else if (phase === "lex") {
        var every = hitEvery();
        var n = Math.floor(pt / every);
        if (n < chips.length && n >= block.hits) jump(every * 0.9);
        updateBot(dt);
        if (block.hits >= chips.length && pt > chips.length * every + 0.9) {
          block.used = true;
          go(level.error && errorPhase() === "lex" ? "error" : "parse");
        }
      } else if (phase === "parse") {
        // Chips gather at the root, then the tree grows one level at a time.
        var root = nodes[0];
        chips.forEach(function (c, i) {
          var k = easeOut((pt - i * 0.02) / 0.6);
          c.x = lerp(c.sx, root ? root.cx - c.w / 2 : c.sx, k);
          c.y = lerp(c.sy, root ? root.cy - c.h / 2 : c.sy, k);
          c.alpha = 1 - k;
        });
        nodes.forEach(function (nd) { if (nd.at < 0 && pt >= 0.55 + nd.depth * 0.42) nd.at = clock; });
        if (pt > 0.95 + L.treeDepth * 0.42) {
          chips.forEach(function (c) { c.state = "gone"; });
          go(level.error && errorPhase() === "parse" ? "error" : "check");
        }
      } else if (phase === "check") {
        var order = nodes.slice().sort(function (a, b) { return a.depth - b.depth; });
        order.forEach(function (nd, i) {
          if (nd.ok < 0 && pt >= 0.2 + i * 0.11) {
            nd.ok = clock;
            nd.bad = isCulprit(nd);
            burst(nd.cx, nd.cy - nd.bh / 2, nd.bad ? 14 : 6, nd.bad ? C.bad : C.good, 0.6);
          }
        });
        if (pt > 0.5 + order.length * 0.11) go(level.error ? "error" : "gen");
      } else if (phase === "gen") {
        var leaves = nodes.slice().sort(function (a, b) { return b.depth - a.depth || a.cx - b.cx; });
        leaves.forEach(function (nd, i) { if (nd.gone < 0 && pt >= 0.15 + i * 0.07) nd.gone = clock; });
        var start = 0.15 + leaves.length * 0.07 + 0.55;
        var bytes = level.bytes || [];
        var slots = L.steps * (L.steps + 1) / 2;
        var count = Math.min(bytes.length, slots);
        for (var i = bricks.length; i < count && pt >= start + i * 0.075; i++) bricks.push(makeBrick(i, bytes[i]));
        if (pt > start + count * 0.075 + 0.7) go("run");
      } else if (phase === "run") {
        flag = easeOut(pt / 1.1);
        if (pt > 0.9) banner = Math.min(1, (pt - 0.9) / 0.4);
        [1.0, 1.35, 1.7, 2.1].forEach(function (at) {
          if (pt - dt < at && pt >= at) firework();
        });
        if (pt > 3.2) go("rest");
      } else if (phase === "error") {
        banner = Math.min(1, pt / 0.4);
        if (pt > 4.2) go("fade");
      } else if (phase === "rest") {
        if (pt > 1.6) go("fade");
      } else if (phase === "fade") {
        fade = 1 - pt / 0.6;
        if (pt >= 0.6) nextRound();
      }
      if (phase !== "type" && phase !== "lex") updateBot(dt);
      sparks = sparks.filter(function (s) { s.life -= dt; s.vy += 420 * dt; s.x += s.vx * dt; s.y += s.vy * dt; return s.life > 0; });
      coins = coins.filter(function (c) {
        c.t += dt;
        if (c.t > 0.45 && !c.done) { c.done = true; c.chip.state = "fly"; c.chip.born = clock; }
        return c.t < 0.45;
      });
      chips.forEach(function (c) {
        if (c.state === "fly") {
          var k = easeOut((clock - c.born) / 0.55);
          c.x = lerp(c.fx, c.sx, k); c.y = lerp(c.fy, c.sy, k);
          if (k >= 1) c.state = "belt";
        } else if (c.state === "belt" && phase !== "parse") { c.x = c.sx; c.y = c.sy; }
      });
      bricks.forEach(function (b) { b.k = Math.min(1, b.k + dt / 0.45); });
      fireworks.forEach(function (f) { f.t += dt; });
      fireworks = fireworks.filter(function (f) { return f.t < 1.4; });
    }

    function walkBot(dt, target) {
      var d = target - bot.x;
      var v = Math.sign(d) * Math.min(Math.abs(d), dt * L.T * 6);
      bot.x += v;
      bot.face = d < 0 ? -1 : 1;
      if (Math.abs(v) > 0.1) bot.frame += dt * 10;
    }

    function jump(duration) {
      if (bot.jumpT >= 0) return;
      bot.jumpT = 0; bot.jumpD = duration; bot.hit = false;
    }

    function updateBot(dt) {
      if (bot.jumpT < 0) return;
      bot.jumpT += dt;
      var k = bot.jumpT / bot.jumpD;
      var height = L.groundY - (L.blockY + L.T) - 1;            // the head just touches the block at the top
      bot.y = L.groundY - height * Math.sin(Math.PI * clamp(k, 0, 1));
      if (!bot.hit && k >= 0.5) {
        bot.hit = true;
        var underBlock = Math.abs(bot.x - (L.blockX + L.T / 2)) < L.T * 0.7;
        if (underBlock && phase === "lex" && block.hits < chips.length) popToken();
        else if (underBlock) block.bump = 1;
      }
      if (k >= 1) { bot.jumpT = -1; bot.y = L.groundY; }
    }

    function popToken() {
      var chip = chips[block.hits++];
      block.bump = 1;
      chip.fx = L.blockX + L.T / 2 - chip.w / 2;
      chip.fy = L.blockY - L.T * 1.3;
      chip.x = chip.fx; chip.y = chip.fy;
      chip.state = "coin";
      coins.push({ t: 0, x: L.blockX + L.T / 2, y: L.blockY, chip: chip });
    }

    function makeBrick(i, value) {
      // Fill the staircase column by column: column c has c+1 bricks.
      var c = 0, n = i;
      while (n > c) { n -= c + 1; c++; }
      var x = L.stairs0 + c * L.B, y = L.groundY - (n + 1) * L.B;
      return { x: x, y: y, fx: L.pipeX + L.pipeW / 2 - L.B / 2, fy: L.pipeTop - L.B, k: 0, text: hex(value) };
    }

    function burst(x, y, n, color, life) {
      for (var i = 0; i < n; i++) {
        var a = Math.random() * Math.PI * 2, s = 60 + Math.random() * 120;
        sparks.push({ x: x, y: y, vx: Math.cos(a) * s, vy: Math.sin(a) * s - 120, life: life * (0.6 + Math.random() * 0.4), color: color });
      }
    }

    var FW = ["#ffd54a", "#ff8a7a", "#7fb6f5", "#8fd694", "#d39ef0"];
    function firework() {
      var x = lerp(L.poleX - 3 * L.T, L.W - L.T, Math.random());
      var y = lerp(L.signY + 40, L.bandTop - L.T, Math.random() * 0.6);
      fireworks.push({ x: x, y: y, t: 0, color: FW[Math.floor(Math.random() * FW.length)], n: 22 + Math.floor(Math.random() * 10) });
    }

    // ---------------------------------------------------------------- rounds

    function nextRound() {
      var lvlPromise = nextLevel || prepare(snippets[snippetIndex++ % snippets.length]);
      nextLevel = null;
      phase = "wait";
      lvlPromise.then(function (lvl) {
        setupLevel(lvl);
        go("type");
        if (reduce) finishInstantly();
        if (!nextLevel) nextLevel = prepare(snippets[snippetIndex++ % snippets.length]);
        // A visitor's line arrived while this level was loading: play it straight away.
        if (skipNext && !reduce) { skipNext = false; go("fade"); pt = 0.3; }
        else if (skipNext) { skipNext = false; nextRound(); }
        draw();
      });
    }

    // Reduced motion: show the whole level at once, as a still picture.
    function finishInstantly() {
      typed = 1; block.used = true; block.hits = chips.length;
      bot.x = L.blockX - L.T;
      chips.forEach(function (c) { c.state = "hidden"; });
      clock = Math.max(clock, 0) + 20;
      nodes.forEach(function (n) { n.at = clock - 15; n.ok = clock - 15; });
      var bytes = level.bytes || [];
      var count = Math.min(bytes.length, L.steps * (L.steps + 1) / 2);
      for (var i = 0; i < count; i++) { var b = makeBrick(i, bytes[i]); b.k = 1; bricks.push(b); }
      if (level.error) { go("error"); banner = 1; }
      else { phase = "rest"; flag = 1; banner = 1; updateHud(); }
    }

    // ---------------------------------------------------------------- drawing

    function px(sprite, x, y, P, palette, flip) {
      for (var r = 0; r < sprite.length; r++) {
        var row = sprite[r];
        for (var c = 0; c < row.length; c++) {
          var ch = row[flip ? row.length - 1 - c : c];
          if (ch === ".") continue;
          ctx.fillStyle = palette[ch];
          ctx.fillRect(Math.round(x + c * P), Math.round(y + r * P), P, P);
        }
      }
    }

    function roundRect(x, y, w, hh, r) {
      ctx.beginPath();
      ctx.moveTo(x + r, y);
      ctx.arcTo(x + w, y, x + w, y + hh, r);
      ctx.arcTo(x + w, y + hh, x, y + hh, r);
      ctx.arcTo(x, y + hh, x, y, r);
      ctx.arcTo(x, y, x + w, y, r);
      ctx.closePath();
    }

    function drawSky() {
      var g = ctx.createLinearGradient(0, 0, 0, L.groundY);
      g.addColorStop(0, C.skyTop);
      g.addColorStop(1, C.skyBot);
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, L.W, L.H);
      var P = L.P;
      if (C.dark) {
        for (var i = 0; i < 70; i++) {
          var sx = (i * 197.3) % L.W, sy = (i * 73.7) % (L.bandTop - 10);
          var tw = 0.45 + 0.55 * Math.sin(clock * 1.7 + i);
          ctx.fillStyle = "rgba(255,255,240," + (0.25 + 0.5 * tw) + ")";
          ctx.fillRect(Math.round(sx), Math.round(sy), i % 7 ? P / 2 + 0.5 : P, i % 7 ? P / 2 + 0.5 : P);
        }
        ctx.fillStyle = "#f4f1d0";
        ctx.beginPath(); ctx.arc(L.W * 0.9, L.T * 1.4, L.T * 0.55, 0, Math.PI * 2); ctx.fill();
        ctx.fillStyle = C.skyTop;
        ctx.beginPath(); ctx.arc(L.W * 0.9 + L.T * 0.25, L.T * 1.3, L.T * 0.5, 0, Math.PI * 2); ctx.fill();
      }
      // Clouds drift slowly; they live high up and stay faint behind the text.
      var drift = clock * 9;
      for (var k = 0; k < 5; k++) {
        var cw = L.T * (2.2 + (k % 3) * 0.6);
        var cx = ((k * 331 + drift * (0.6 + k * 0.15)) % (L.W + cw * 2)) - cw;
        var cy = L.T * (0.8 + (k * 1.37) % 3.2);
        cloud(cx, cy, cw);
      }
    }

    function cloud(x, y, w) {
      var P = L.P, hh = w * 0.42;
      ctx.fillStyle = C.cloud;
      ctx.fillRect(Math.round(x), Math.round(y + hh * 0.45), Math.round(w), Math.round(hh * 0.55));
      ctx.fillRect(Math.round(x + w * 0.18), Math.round(y + hh * 0.15), Math.round(w * 0.35), Math.round(hh * 0.5));
      ctx.fillRect(Math.round(x + w * 0.45), Math.round(y), Math.round(w * 0.32), Math.round(hh * 0.6));
      ctx.fillRect(Math.round(x + P), Math.round(y + hh), Math.round(w - 2 * P), P);
    }

    function drawHills() {
      var T = L.T;
      var shift = (pointer.x - L.W / 2) * 0.015;
      [[0.1, 3.2], [0.42, 2.2], [0.78, 2.8]].forEach(function (hl, i) {
        var cx = hl[0] * L.W - shift * (i + 1), r = hl[1] * T;
        ctx.fillStyle = C.hill;
        ctx.beginPath(); ctx.ellipse(cx, L.groundY, r * 1.4, r, 0, Math.PI, 0); ctx.fill();
        ctx.strokeStyle = C.hillEdge; ctx.lineWidth = L.P;
        ctx.beginPath(); ctx.ellipse(cx, L.groundY, r * 1.4, r, 0, Math.PI, 0); ctx.stroke();
        ctx.fillStyle = C.hillEdge;
        ctx.fillRect(Math.round(cx - T * 0.4), Math.round(L.groundY - r * 0.7), L.P * 2, L.P * 3);
        ctx.fillRect(Math.round(cx + T * 0.2), Math.round(L.groundY - r * 0.6), L.P * 2, L.P * 3);
      });
      // Bushes along the ground.
      for (var x = T * 0.6; x < L.W; x += T * 5.3) {
        ctx.fillStyle = C.bush;
        ctx.beginPath(); ctx.ellipse(x, L.groundY, T * 0.9, T * 0.45, 0, Math.PI, 0); ctx.fill();
        ctx.beginPath(); ctx.ellipse(x + T * 0.7, L.groundY, T * 0.6, T * 0.35, 0, Math.PI, 0); ctx.fill();
      }
    }

    function drawGround() {
      var T = L.T, P = L.P, bw = T / 2, bh = T / 4;
      ctx.fillStyle = C.brick;
      ctx.fillRect(0, L.groundY, L.W, L.H - L.groundY);
      ctx.fillStyle = C.mortar;
      for (var row = 0, y = L.groundY; y < L.H; row++, y += bh) {
        ctx.fillRect(0, Math.round(y), L.W, P / 2 + 0.5);
        for (var x = (row % 2) * bw / 2; x < L.W; x += bw) ctx.fillRect(Math.round(x), Math.round(y), P / 2 + 0.5, bh);
      }
      ctx.fillStyle = C.brickHi;
      ctx.fillRect(0, L.groundY, L.W, P / 2 + 0.5);
    }

    function drawBlock() {
      var T = L.T, P = L.P;
      var x = L.blockX, y = L.blockY - Math.sin(block.bump * Math.PI) * T * 0.22;
      ctx.fillStyle = C.blockEdge;
      ctx.fillRect(x, y, T, T);
      ctx.fillStyle = block.used ? C.used : C.block;
      ctx.fillRect(x + P, y + P, T - 2 * P, T - 2 * P);
      ctx.fillStyle = C.blockEdge;
      [[2, 2], [13, 2], [2, 13], [13, 13]].forEach(function (p) { ctx.fillRect(x + p[0] * P, y + p[1] * P, P, P); });
      if (!block.used) {
        var glow = 0.75 + 0.25 * Math.sin(clock * 5);
        px(QMARK, x + 5 * P + P, y + 4 * P + P, P, { w: C.blockEdge }, false);
        ctx.globalAlpha = glow;
        px(QMARK, x + 5 * P, y + 4 * P, P, { w: "#fff6d8" }, false);
        ctx.globalAlpha = 1;
      }
    }

    function drawBot() {
      var P = L.P, w = 12 * P, hh = 13 * P;
      var x = bot.x - w / 2, y = bot.y - hh;
      var pal = { a: "#ffd54a", o: "#ff8a1f", w: "#ffffff", k: "#1b1e23", m: "#7a2e00", b: "#2f6db5", f: "#3b2a1a" };
      var legs = bot.jumpT >= 0 ? BOT.jump : (Math.floor(bot.frame) % 2 ? BOT.run2 : BOT.run1);
      var blink = Math.sin(clock * 1.3) > 0.985;
      var body = blink ? BOT.body.map(function (r, i) { return i === 5 ? ".oooooooooo." : r; }) : BOT.body;
      px(body.concat(legs), x, y, P, pal, bot.face < 0);
    }

    function drawPipe() {
      var P = L.P, x = L.pipeX, w = L.pipeW, top = L.pipeTop;
      var squish = 0;
      if (phase === "gen") nodes.forEach(function (n) { if (n.gone >= 0) { var k = (clock - n.gone) / 0.5; if (k > 0.8 && k < 1.2) squish = 1 - Math.abs(k - 1) * 5; } });
      var lip = 0.45 * L.T + squish * P;
      ctx.fillStyle = C.pipeEdge;
      ctx.fillRect(x + P * 2, top + lip - P, w - P * 4, L.groundY - top - lip + P);
      ctx.fillRect(x - squish * P, top, w + squish * 2 * P, lip);
      ctx.fillStyle = C.pipe;
      ctx.fillRect(x + P * 3, top + lip, w - P * 6, L.groundY - top - lip);
      ctx.fillRect(x + P - squish * P, top + P, w - 2 * P + squish * 2 * P, lip - 2 * P);
      ctx.fillStyle = C.pipeHi;
      ctx.fillRect(x + P * 5, top + lip, P * 2, L.groundY - top - lip);
      ctx.fillRect(x + P * 3, top + P * 2, P * 2, lip - 4 * P);
    }

    function drawStairs() {
      ctx.font = L.small;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      bricks.forEach(function (b) {
        var k = easeOut(b.k);
        var x = lerp(b.fx, b.x, k), y = lerp(b.fy, b.y, k) - Math.sin(k * Math.PI) * L.T * 1.4;
        var B = L.B, P = Math.max(1, L.P / 2);
        ctx.fillStyle = C.mortar;
        ctx.fillRect(x, y, B, B);
        ctx.fillStyle = C.brick;
        ctx.fillRect(x + P, y + P, B - 2 * P, B - 2 * P);
        ctx.fillStyle = C.brickHi;
        ctx.fillRect(x + P, y + P, B - 2 * P, P);
        ctx.fillStyle = "#fff4e6";
        ctx.fillText(b.text, x + B / 2, y + B / 2 + 1);
      });
      ctx.textAlign = "left";
    }

    function drawPole() {
      var P = L.P, x = L.poleX, top = Math.max(10, L.bandTop - 2.5 * L.T);
      ctx.fillStyle = C.dark ? "#9aa3ad" : "#5d6670";
      ctx.fillRect(x - P / 2, top, P, L.groundY - top);
      ctx.fillStyle = C.good;
      ctx.beginPath(); ctx.arc(x, top, P * 2, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = C.blockEdge;
      ctx.fillRect(x - L.T * 0.4, L.groundY - L.T * 0.4, L.T * 0.8, L.T * 0.4);
      var low = Math.min(L.groundY - L.T * 1.3, L.groundY - L.steps * L.B - L.T * 0.9);
      var fy = lerp(low, top + P * 3, flag);
      ctx.fillStyle = flag >= 1 ? C.good : "#ffffff";
      ctx.beginPath(); ctx.moveTo(x - P, fy); ctx.lineTo(x - L.T * 1.1, fy + L.T * 0.35); ctx.lineTo(x - P, fy + L.T * 0.7); ctx.fill();
      ctx.fillStyle = flag >= 1 ? "#ffffff" : C.good;
      ctx.font = "700 " + Math.round(L.T * 0.38) + "px " + C.mono;
      ctx.textAlign = "center"; ctx.textBaseline = "middle";
      ctx.fillText("W", x - L.T * 0.45, fy + L.T * 0.36);
      ctx.textAlign = "left";
    }

    function drawCastle() {
      if (!L.wide) return;
      var T = L.T, P = L.P, x = L.castleX, w = 4 * T, base = L.groundY;
      ctx.fillStyle = C.castleEdge;
      ctx.fillRect(x, base - 2.4 * T, w, 2.4 * T);
      ctx.fillRect(x + T, base - 3.6 * T, 2 * T, 1.3 * T);
      ctx.fillStyle = C.castle;
      ctx.fillRect(x + P, base - 2.4 * T + P, w - 2 * P, 2.4 * T - P);
      ctx.fillRect(x + T + P, base - 3.6 * T + P, 2 * T - 2 * P, 1.3 * T);
      ctx.fillStyle = C.castleEdge;
      for (var i = 0; i < 4; i++) ctx.fillRect(x + i * T + T * 0.3, base - 2.4 * T - T * 0.35, T * 0.45, T * 0.4);
      for (var j = 0; j < 2; j++) ctx.fillRect(x + T + j * T + T * 0.3, base - 3.6 * T - T * 0.35, T * 0.45, T * 0.4);
      // The door and windows light up when the program runs.
      var lit = phase === "run" || phase === "rest" || phase === "fade" ? 1 : 0;
      ctx.fillStyle = lit ? "#ffd54a" : C.castleEdge;
      ctx.beginPath(); ctx.arc(x + 2 * T, base - 0.75 * T, T * 0.45, Math.PI, 0); ctx.fill();
      ctx.fillRect(x + 1.55 * T, base - 0.75 * T, 0.9 * T, 0.75 * T);
      ctx.fillRect(x + 1.75 * T, base - 3.2 * T, 0.5 * T, 0.6 * T);
    }

    function drawSign() {
      var x = L.signX, y = L.signY, w = L.signW, hh = signHeight(), pad = L.signPad;
      ctx.fillStyle = C.panel;
      roundRect(x, y, w, hh, 10); ctx.fill();
      ctx.strokeStyle = C.panelEdge; ctx.lineWidth = 1; ctx.stroke();
      ctx.font = "700 10px " + C.mono;
      ctx.fillStyle = "#ffd54a";
      ctx.textBaseline = "top";
      ctx.fillText(level.custom ? t("splash.yours") : "WORLD 1-1 · level.wahoo", x + pad, y + pad - 2);
      var total = level.lines.join("\n").length, shown = Math.floor(total * typed), count = 0;
      var active = phase === "lex" && block.hits > 0 ? chips[block.hits - 1].tok : null;
      ctx.font = L.font;
      var charW = measure("M", L.font);
      level.lines.forEach(function (line, li) {
        var ly = y + pad + 16 + li * L.lineH;
        var visible = Math.max(0, Math.min(line.length, shown - count));
        count += line.length + 1;
        // Colour each character by the token it belongs to, once the lexer has reached it.
        for (var ci = 0; ci < visible; ci++) {
          var tok = tokenAt(li, ci);
          var lexed = tok && (phase !== "type" && phase !== "lex" || chips.indexOf(chipOf(tok)) < block.hits);
          ctx.fillStyle = lexed ? C.syn[tok.cls] || "#ffffff" : "rgba(255,255,255,0.8)";
          if (tok && tok === active) {
            ctx.save(); ctx.fillStyle = "rgba(255,213,74,0.25)";
            ctx.fillRect(x + pad + ci * charW - 1, ly - 2, charW + 2, L.lineH - 2); ctx.restore();
          }
          if (level.error && phase === "error" && level.error.line - 2 === li && ci + 3 >= level.error.col && ci + 3 < level.error.col + Math.max(1, level.error.to - level.error.from)) {
            ctx.fillStyle = C.bad;
            ctx.fillRect(x + pad + ci * charW, ly + L.fs + 2, charW, 2);
          }
          ctx.fillText(line[ci], x + pad + ci * charW, ly);
        }
        // The typing caret sits on the line being typed.
        var lineStart = count - line.length - 1;
        if (typed < 1 && shown >= lineStart && shown <= lineStart + line.length && Math.floor(clock * 3) % 2) {
          ctx.fillStyle = "#ffd54a";
          ctx.fillRect(x + pad + visible * charW, ly, 2, L.fs + 2);
        }
      });
      ctx.textBaseline = "alphabetic";
    }

    var tokIndex = null;
    function tokenAt(line, col) {
      if (!tokIndex || tokIndex.level !== level) {
        tokIndex = { level: level, map: {} };
        level.tokens.forEach(function (tk) { for (var i = 0; i < tk.text.length; i++) tokIndex.map[tk.line + ":" + (tk.col + i)] = tk; });
      }
      return tokIndex.map[line + ":" + col];
    }
    function chipOf(tok) { for (var i = 0; i < chips.length; i++) if (chips[i].tok === tok) return chips[i]; return null; }

    function drawChip(c, alpha) {
      if (alpha <= 0) return;
      ctx.globalAlpha = alpha * fade;
      ctx.fillStyle = C.panel;
      roundRect(c.x, c.y, c.w, c.h, 6); ctx.fill();
      ctx.strokeStyle = C.syn[c.tok.cls] || "#ffffff"; ctx.lineWidth = 1.5; ctx.stroke();
      ctx.fillStyle = C.syn[c.tok.cls] || "#ffffff";
      ctx.font = L.font; ctx.textBaseline = "middle";
      ctx.fillText(c.tok.text, c.x + 7, c.y + c.h / 2 + 1);
      ctx.textBaseline = "alphabetic";
      ctx.globalAlpha = 1;
    }

    function drawCoins() {
      coins.forEach(function (c) {
        var y = c.y - easeOut(c.t / 0.45) * L.T * 1.5;
        var spin = Math.abs(Math.cos(c.t * 18));
        var w = Math.max(L.P, L.T * 0.42 * spin), hh = L.T * 0.6;
        ctx.fillStyle = "#b07800";
        ctx.fillRect(Math.round(c.x - w / 2), Math.round(y - hh), Math.round(w), Math.round(hh));
        ctx.fillStyle = "#ffd54a";
        ctx.fillRect(Math.round(c.x - w / 2 + L.P / 2), Math.round(y - hh + L.P / 2), Math.max(1, Math.round(w - L.P)), Math.round(hh - L.P));
      });
    }

    function drawTree() {
      if (!nodes.length) return;
      var P = L.P, pw = Math.max(4, Math.round(P * 2.5 * L.treeScale));
      // Pipes from each parent down to its children, growing as the children appear.
      nodes.forEach(function (n) {
        if (!n.parent || n.at < 0) return;
        var p = n.parent;
        var a = gone(n), k = easeOut((clock - n.at) / 0.4) * (1 - a);
        if (k <= 0) return;
        var y0 = p.cy + p.bh / 2, y1 = n.cy - n.bh / 2, ym = (y0 + y1) / 2;
        ctx.globalAlpha = fade * (1 - a);
        pipeSeg(p.cx, y0, p.cx, ym, pw, Math.min(1, k * 3));
        if (k > 0.33) pipeSeg(p.cx, ym, n.cx, ym, pw, Math.min(1, (k - 0.33) * 3));
        if (k > 0.66) pipeSeg(n.cx, ym, n.cx, y1, pw, Math.min(1, (k - 0.66) * 3));
        ctx.globalAlpha = 1;
      });
      nodes.forEach(function (n) {
        if (n.at < 0) return;
        var k = easeOut((clock - n.at) / 0.35);
        var g = gone(n);
        var cx = n.cx, cy = n.cy, s = (0.6 + 0.4 * k);
        if (g > 0) {
          var tx = L.pipeX + L.pipeW / 2, ty = L.pipeTop;
          cx = lerp(n.cx, tx, ease(g)); cy = lerp(n.cy, ty, ease(g)) - Math.sin(g * Math.PI) * L.T; s *= 1 - 0.85 * g;
          if (g >= 1) return;
        }
        var w = n.bw * s, hh = n.bh * s;
        ctx.globalAlpha = k * fade;
        ctx.fillStyle = C.nodeEdge;
        ctx.fillRect(cx - w / 2, cy - hh / 2, w, hh);
        ctx.fillStyle = n.bad ? C.bad : n.ok >= 0 && clock - n.ok < 0.5 ? C.good : C.node;
        ctx.fillRect(cx - w / 2 + 2, cy - hh / 2 + 2, w - 4, hh - 4);
        ctx.fillStyle = n.cls === "lit" ? (C.dark ? "#f0b45e" : "#a35a00") : n.cls === "name" ? (C.dark ? "#7fb6f5" : "#1565c0") :
          n.cls === "call" ? (C.dark ? "#f48fb1" : "#c2185b") : C.nodeText;
        ctx.font = "600 " + Math.max(9, Math.round(L.fs * s * Math.max(0.8, L.treeScale))) + "px " + C.mono;
        ctx.textAlign = "center"; ctx.textBaseline = "middle";
        ctx.fillText(n.label, cx, cy + 1);
        if (n.ok >= 0 && g === 0) {
          ctx.fillStyle = n.bad ? C.bad : C.good;
          ctx.fillText(n.bad ? "✗" : "✓", cx + w / 2 + 7, cy - hh / 2 + 2);
        }
        ctx.textAlign = "left"; ctx.textBaseline = "alphabetic";
        ctx.globalAlpha = 1;
      });
    }

    // The node the checker complains about: the deepest one whose span starts where the error does.
    function isCulprit(nd) {
      var e = level.error;
      if (!e) return false;
      var same = nodes.filter(function (o) { return o.line === e.line && o.col === e.col; });
      return same.length ? same[same.length - 1] === nd : false;
    }

    function gone(n) { return n.gone < 0 ? 0 : clamp((clock - n.gone) / 0.5, 0, 1); }

    function pipeSeg(x0, y0, x1, y1, w, k) {
      var x = lerp(x0, x1, k), y = lerp(y0, y1, k);
      ctx.strokeStyle = C.pipeEdge; ctx.lineWidth = w + 2; ctx.lineCap = "square";
      ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x, y); ctx.stroke();
      ctx.strokeStyle = C.pipe; ctx.lineWidth = w;
      ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x, y); ctx.stroke();
      ctx.strokeStyle = C.pipeHi; ctx.lineWidth = Math.max(1, w / 4);
      ctx.beginPath(); ctx.moveTo(x0 - (y0 !== y1 ? w / 5 : 0), y0 - (y0 === y1 ? w / 5 : 0)); ctx.lineTo(x - (y0 !== y1 ? w / 5 : 0), y - (y0 === y1 ? w / 5 : 0)); ctx.stroke();
    }

    function drawEffects() {
      sparks.forEach(function (s) {
        ctx.globalAlpha = clamp(s.life * 2, 0, 1);
        ctx.fillStyle = s.color;
        ctx.fillRect(Math.round(s.x), Math.round(s.y), L.P, L.P);
      });
      fireworks.forEach(function (f) {
        for (var i = 0; i < f.n; i++) {
          var a = (i / f.n) * Math.PI * 2, r = easeOut(f.t / 0.9) * L.T * 1.6;
          var x = f.x + Math.cos(a) * r, y = f.y + Math.sin(a) * r + f.t * f.t * 40;
          ctx.globalAlpha = clamp(1.4 - f.t, 0, 1);
          ctx.fillStyle = f.color;
          ctx.fillRect(Math.round(x), Math.round(y), L.P, L.P);
        }
      });
      ctx.globalAlpha = 1;
    }

    function drawBanner() {
      if (banner <= 0) return;
      var lines = phase === "error" ? [t("splash.gameOver"), level.error.title] : (level.output && level.output.length ? level.output : [t("splash.clear")]);
      ctx.font = L.font;
      var w = Math.max.apply(null, lines.map(function (l) { return measure(l, L.font); })) + 28;
      var hh = lines.length * L.lineH + 18;
      var cx = phase === "error" ? L.blockX + L.T / 2 : (L.wide ? L.castleX + 2 * L.T : L.poleX - L.T * 1.5);
      var x = clamp(cx - w / 2, 8, L.W - w - 8);
      var y = phase === "error" ? L.blockY - hh - L.T * 0.6 : L.bandTop - hh - (L.wide ? L.T * 1.2 : L.T * 0.4);
      y -= (1 - easeOut(banner)) * 16;
      ctx.globalAlpha = easeOut(banner) * fade;
      ctx.fillStyle = C.panel;
      roundRect(x, y, w, hh, 8); ctx.fill();
      ctx.strokeStyle = phase === "error" ? C.bad : "#ffd54a"; ctx.lineWidth = 2; ctx.stroke();
      ctx.textBaseline = "top";
      lines.forEach(function (l, i) {
        ctx.fillStyle = phase === "error" && i === 0 ? C.bad : i === 0 ? "#ffd54a" : "#ffffff";
        ctx.fillText(l, x + 14, y + 10 + i * L.lineH);
      });
      ctx.textBaseline = "alphabetic";
      ctx.globalAlpha = 1;
    }

    function draw() {
      if (!L.W) return;
      ctx.clearRect(0, 0, L.W, L.H);
      drawSky();
      drawHills();
      drawCastle();
      if (!level) { drawGround(); return; }
      drawPole();
      drawPipe();
      drawStairs();
      drawGround();
      ctx.globalAlpha = fade; drawSign(); ctx.globalAlpha = 1;
      drawTree();
      drawBlock();
      drawBot();
      drawCoins();
      chips.forEach(function (c) {
        if (c.state === "fly" || c.state === "belt") drawChip(c, phase === "parse" ? (c.alpha === undefined ? 1 : c.alpha) : 1);
      });
      drawEffects();
      drawBanner();
    }

    // ---------------------------------------------------------------- loop, visibility and controls

    var last = 0;
    function frame(now) {
      if (!running) return;
      var dt = Math.min(0.05, (now - (last || now)) / 1000);
      last = now;
      if (level && phase !== "wait") step(dt);
      else clock += dt;
      draw();
      requestAnimationFrame(frame);
    }

    function setRunning(on) {
      on = on && !paused && visible && !reduce;
      if (on === running) return;
      running = on;
      last = 0;
      if (on) requestAnimationFrame(frame);
    }

    function syncPause() {
      pauseBtn.textContent = paused ? t("splash.play") : t("splash.pause");
      pauseBtn.setAttribute("aria-pressed", paused ? "true" : "false");
    }
    pauseBtn.addEventListener("click", function () { paused = !paused; syncPause(); setRunning(true); });
    nextBtn.addEventListener("click", function () {
      if (phase === "wait") return;
      if (reduce) { nextRound(); return; }
      if (paused) { paused = false; syncPause(); setRunning(true); }
      go("fade"); pt = 0.3;
    });
    syncPause();
    if (reduce) pauseBtn.hidden = true;

    function play(text) {
      if (!text.trim()) return;
      nextLevel = prepare(linesFor(text)).then(function (lvl) { lvl.custom = true; return lvl; });
      if (paused) { paused = false; syncPause(); }
      if (phase === "wait") skipNext = true;
      else if (reduce) nextRound();
      else { go("fade"); pt = 0.3; }
      setRunning(true);
    }
    if (input && goBtn) {
      input.placeholder = t("splash.tryHint");
      goBtn.addEventListener("click", function () { play(input.value); });
      input.addEventListener("keydown", function (e) { if (e.key === "Enter") { e.preventDefault(); play(input.value); } });
      root.querySelector(".splash-try").hidden = false;
    }

    // Clicking the scene makes the bot jump (and bump the block if it is under it).
    var pointer = { x: 0 };
    canvas.addEventListener("pointermove", function (e) { pointer.x = e.clientX - canvas.getBoundingClientRect().left; });
    canvas.addEventListener("pointerdown", function (e) {
      if (!level || phase === "lex" || reduce) return;
      var x = e.clientX - canvas.getBoundingClientRect().left;
      bot.face = x < bot.x ? -1 : 1;
      jump(0.5);
    });

    if ("IntersectionObserver" in window) {
      new IntersectionObserver(function (entries) { visible = entries[0].isIntersecting; setRunning(true); }).observe(root);
    }
    document.addEventListener("visibilitychange", function () { visible = !document.hidden; setRunning(true); });

    function relayout() {
      readColors();
      layout();
      pointer.x = pointer.x || L.W / 2;
      // A resize replays the current level from the start with the new layout (or shows it finished).
      if (level && phase !== "wait") {
        setupLevel(level);
        go("type");
        if (reduce) finishInstantly();
      }
      draw();
    }
    var resizeTimer = 0;
    window.addEventListener("resize", function () { clearTimeout(resizeTimer); resizeTimer = setTimeout(relayout, 150); });
    WH.onTheme(function () { readColors(); draw(); });

    readColors();
    layout();
    pointer.x = L.W / 2;
    draw();
    nextRound();
    setRunning(true);
  });
})();
