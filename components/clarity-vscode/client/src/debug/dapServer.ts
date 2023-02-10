import { initSync } from "../clarity-dap-browser/dap-browser";
export declare const __EXTENSION_URL__: string;

async function startServer() {
  const wasmURL = new URL("client/dist/dap-browser_bg.wasm", __EXTENSION_URL__);

  const wasmModule = fetch(wasmURL, {}).then((wasm) => wasm.arrayBuffer());

  initSync(await wasmModule);
}

self.onmessage = function onMessage(ev) {
  console.log("ev", ev);
};
