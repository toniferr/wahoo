// Shared runtime for every page: theme toggle, demo registry and small DOM helpers.
// Demos live in js/demos/<group>.js and call WH.register("<group>/<name>", init); every element with
// data-demo="<group>/<name>" is handed to its init function once the page has loaded.
(function () {
  "use strict";

  var registry = {};
  var themeListeners = [];
  var strings = {};

  // ------------------------------------------------------------------ strings and numbers

  function t(key, vars) {
    var v = key.split(".").reduce(function (o, k) { return o && o[k]; }, strings);
    if (typeof v !== "string") {
      console.warn("missing string", key);
      return key;
    }
    return vars ? v.replace(/\{(\w+)\}/g, function (_, k) { return k in vars ? vars[k] : "{" + k + "}"; }) : v;
  }

  function lang() { return strings.lang || document.documentElement.lang || "en"; }

  function fmt(x) { return new Intl.NumberFormat(lang()).format(x); }

  // ------------------------------------------------------------------ DOM

  function setAttrs(node, attrs, svg) {
    if (!attrs) return node;
    Object.keys(attrs).forEach(function (k) {
      var v = attrs[k];
      if (v === undefined || v === null || v === false) return;
      if (k === "text") node.textContent = v;
      else if (k === "class") svg ? node.setAttribute("class", v) : (node.className = v);
      else if (k.slice(0, 2) === "on" && typeof v === "function") node.addEventListener(k.slice(2), v);
      else if (k === "dataset") Object.keys(v).forEach(function (d) { node.dataset[d] = v[d]; });
      else node.setAttribute(k, v === true ? "" : v);
    });
    return node;
  }

  function append(node, children) {
    children.forEach(function (c) {
      if (c === null || c === undefined || c === false) return;
      if (Array.isArray(c)) append(node, c);
      else node.appendChild(typeof c === "string" || typeof c === "number" ? document.createTextNode(String(c)) : c);
    });
    return node;
  }

  function h(tag, attrs) {
    return append(setAttrs(document.createElement(tag), attrs, false), [].slice.call(arguments, 2));
  }

  var SVGNS = "http://www.w3.org/2000/svg";
  function s(tag, attrs) {
    return append(setAttrs(document.createElementNS(SVGNS, tag), attrs, true), [].slice.call(arguments, 2));
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
    return node;
  }

  // ------------------------------------------------------------------ controls

  var uid = 0;

  function button(label, onClick, cls) {
    return h("button", { type: "button", class: "btn" + (cls ? " " + cls : ""), onclick: onClick }, label);
  }

  function select(o) {
    var id = "ctl" + ++uid;
    var sel = h("select", { id: id }, o.options.map(function (opt) {
      return h("option", { value: opt.value, selected: opt.value === o.value }, opt.label);
    }));
    sel.addEventListener("change", function () { if (o.onChange) o.onChange(sel.value); });
    return { el: h("div", { class: "ctl ctl-select" }, h("label", { for: id }, o.label), sel), input: sel,
      get: function () { return sel.value; }, set: function (v) { sel.value = v; } };
  }

  function toggle(o) {
    var id = "ctl" + ++uid;
    var box = h("input", { type: "checkbox", id: id, checked: !!o.checked });
    box.addEventListener("change", function () { if (o.onChange) o.onChange(box.checked); });
    return { el: h("div", { class: "ctl ctl-toggle" }, box, h("label", { for: id }, o.label)), input: box,
      get: function () { return box.checked; }, set: function (v) { box.checked = !!v; } };
  }

  function segmented(o) {
    var buttons = o.options.map(function (opt) {
      return h("button", { type: "button", class: "seg", "aria-pressed": opt.value === o.value ? "true" : "false",
        dataset: { value: opt.value }, onclick: function () { set(opt.value, false); } }, opt.label);
    });
    var value = o.value;
    function set(v, silent) {
      value = v;
      buttons.forEach(function (b) { b.setAttribute("aria-pressed", b.dataset.value === v ? "true" : "false"); });
      if (!silent && o.onChange) o.onChange(v);
    }
    var el = h("div", { class: "ctl ctl-seg", role: "group", "aria-label": o.label },
      o.showLabel === false ? null : h("span", { class: "ctl-label" }, o.label), h("div", { class: "seg-group" }, buttons));
    return { el: el, get: function () { return value; }, set: set };
  }

  function controls() {
    return h("div", { class: "controls" }, [].slice.call(arguments));
  }

  function stat(label, cls) {
    var val = h("span", { class: "stat-val" });
    var el = h("div", { class: "stat" + (cls ? " " + cls : "") }, h("span", { class: "stat-label" }, label), val);
    return { el: el, set: function (v) { val.textContent = v; } };
  }

  // ------------------------------------------------------------------ theme and SVG

  function onTheme(fn) { themeListeners.push(fn); }

  function setTheme(theme) {
    document.documentElement.setAttribute("data-theme", theme);
    try { localStorage.setItem("theme", theme); } catch (e) { /* storage blocked */ }
    themeListeners.forEach(function (fn) { try { fn(theme); } catch (e) { console.error(e); } });
  }

  // Responsive SVG with a fixed coordinate system.
  function svgBox(parent, w, hgt, label) {
    var el = s("svg", { viewBox: "0 0 " + w + " " + hgt, class: "demo-svg", role: "img", "aria-label": label || "" });
    parent.appendChild(el);
    return el;
  }

  function clamp(x, lo, hi) { return Math.max(lo, Math.min(hi, x)); }

  // ------------------------------------------------------------------ boot

  function register(name, init) { registry[name] = init; }

  function boot() {
    try { strings = JSON.parse(document.body.getAttribute("data-strings") || "{}"); } catch (e) { strings = {}; }

    var toggleBtn = document.querySelector(".theme-toggle");
    if (toggleBtn) {
      toggleBtn.addEventListener("click", function () {
        setTheme(document.documentElement.getAttribute("data-theme") === "dark" ? "light" : "dark");
      });
    }

    // Close the chapters menu when clicking elsewhere or pressing Escape.
    var menu = document.querySelector(".chapters-menu");
    if (menu) {
      document.addEventListener("click", function (e) { if (menu.open && !menu.contains(e.target)) menu.open = false; });
      document.addEventListener("keydown", function (e) { if (e.key === "Escape") menu.open = false; });
    }

    document.querySelectorAll("[data-demo]").forEach(function (el) {
      var name = el.getAttribute("data-demo");
      var init = registry[name];
      if (!init) {
        el.setAttribute("data-error", "unregistered");
        return;
      }
      try {
        var stage = el.querySelector(".demo-stage");
        if (!stage && el.tagName === "FIGURE") {
          stage = h("div", { class: "demo-stage" });
          el.insertBefore(stage, el.querySelector("figcaption"));
        }
        init(stage || el, el);
        el.setAttribute("data-ready", "");
      } catch (err) {
        console.error(name, err);
        el.setAttribute("data-error", String(err && err.message || err));
        el.appendChild(h("p", { class: "demo-error" }, t("core.error")));
      }
    });
  }

  window.WH = {
    register: register, t: t, lang: lang, fmt: fmt, h: h, s: s, clear: clear,
    button: button, select: select, toggle: toggle, segmented: segmented, controls: controls, stat: stat,
    onTheme: onTheme, svgBox: svgBox, clamp: clamp,
  };

  document.addEventListener("DOMContentLoaded", boot);
})();
