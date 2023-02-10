import { DebugProtocol } from "@vscode/debugprotocol";
import { DapWasmBridge, initSync } from "./clarity-dap-browser/dap-browser";
export declare const __EXTENSION_URL__: string;

let dapDebugger: DapWasmBridge;

async function startServer() {
  const wasmURL = new URL(
    "server-dap/dist/dap-browser_bg.wasm",
    __EXTENSION_URL__,
  );

  const wasmModule = fetch(wasmURL, {}).then((wasm) => wasm.arrayBuffer());

  initSync(await wasmModule);

  dapDebugger = new DapWasmBridge(sendRequest, sendResponse, sendEvent);
}
const bootPromise = startServer();

onmessage = (ev) => {
  const data = ev.data;
  if (!data) return;

  if (data === "init") {
    bootPromise
      .then(() => {
        postMessage({ type: "event", method: "worker-ready" });
      })
      .catch((err) => {
        console.error(err);
      });
    return;
  }

  if (data.type === "response") {
    const requestPromise = requests.get(data.id)!;
    requestPromise(data.data);
    return;
  }

  if (data.type === "debug-message") {
    const message = data.message as DebugProtocol.ProtocolMessage;
    if (message.type === "request") {
      const request = data.message as DebugProtocol.Request;

      const res = dapDebugger!.handleMessage(
        BigInt(request.seq),
        request.command,
        request.arguments,
        data.int32,
      );
      if (request.command === "launch") {
        dapDebugger!.runDap();
      }

      return;
    }
  }
};

const requests: Map<string, (value: any) => void> = new Map();

async function sendRequest(method: string, data: any) {
  const id = Math.random().toString(36).slice(2);
  postMessage({ type: "request", id, method, data });
  return new Promise((resolve) => {
    requests.set(id, resolve);
  });
}

function sendResponse(res: Response) {
  postMessage({ type: "debugger-response", res });
}

function sendEvent(event: Event) {
  postMessage({ type: "debugger-event", event });
}
