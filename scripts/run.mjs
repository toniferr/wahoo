// Runs a compiled Wahoo module with Node's WebAssembly engine (V8), as the browser playground does.
//   node scripts/run.mjs program.wasm
// The host side is four imports: print a number, a switch, a text (read from linear memory), and a newline.
import { readFileSync } from "node:fs";

const path = process.argv[2];
if (!path) {
  console.error("usage: node scripts/run.mjs <file.wasm>");
  process.exit(64);
}

let memory = null;
let line = "";
const utf8 = new TextDecoder();

// A text value is the address of a little-endian u32 length followed by that many UTF-8 bytes.
function readText(ptr) {
  const len = new DataView(memory.buffer).getUint32(ptr, true);
  return utf8.decode(new Uint8Array(memory.buffer, ptr + 4, len));
}

const imports = {
  wahoo: {
    print_coins: (n) => { line += String(n); },
    print_switch: (b) => { line += b ? "star" : "goomba"; },
    print_text: (ptr) => { line += readText(ptr); },
    print_newline: () => { process.stdout.write(line + "\n"); line = ""; },
  },
};

const { instance } = await WebAssembly.instantiate(readFileSync(path), imports);
memory = instance.exports.memory;
try {
  instance.exports.main();
} catch (e) {
  if (line) process.stdout.write(line + "\n");
  console.error(e instanceof WebAssembly.RuntimeError ? `trap: ${e.message}` : e);
  process.exit(2);
}
