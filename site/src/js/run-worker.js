// Runs one compiled Wahoo module, off the main thread. The page terminates this worker if it runs too long.
// The host side is four imports: print a number, a switch, a text (read from linear memory), and a newline.
"use strict";

self.onmessage = function (e) {
  var bytes = e.data.bytes;
  var maxLines = e.data.maxLines;
  var memory = null;
  var line = "";
  var pending = [];
  var count = 0;
  var last = Date.now();
  var utf8 = new TextDecoder();

  function flush() {
    if (pending.length) self.postMessage({ type: "lines", lines: pending });
    pending = [];
    last = Date.now();
  }

  // A text value is the address of a little-endian u32 length followed by that many UTF-8 bytes.
  function readText(ptr) {
    var len = new DataView(memory.buffer).getUint32(ptr, true);
    return utf8.decode(new Uint8Array(memory.buffer, ptr + 4, len));
  }

  var imports = {
    wahoo: {
      print_coins: function (n) { line += String(n); },
      print_switch: function (b) { line += b ? "star" : "goomba"; },
      print_text: function (p) { line += readText(p); },
      print_newline: function () {
        pending.push(line);
        line = "";
        if (++count >= maxLines) throw new Error("output");
        if (Date.now() - last > 50) flush();
      },
    },
  };

  WebAssembly.instantiate(bytes, imports).then(function (res) {
    memory = res.instance.exports.memory;
    try {
      res.instance.exports.main();
      if (line) pending.push(line);
      flush();
      self.postMessage({ type: "done" });
    } catch (err) {
      if (line) pending.push(line);
      flush();
      self.postMessage({ type: "trap", message: String(err && err.message || err) });
    }
  }, function (err) {
    self.postMessage({ type: "trap", message: String(err && err.message || err) });
  });
};
