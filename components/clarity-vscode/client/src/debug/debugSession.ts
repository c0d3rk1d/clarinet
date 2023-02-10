import { DebugSession } from "@vscode/debugadapter";
import { DebugProtocol } from "@vscode/debugprotocol";
import * as vscode from "vscode";

import { fileArrayToString } from "../utils/files";

export declare const __EXTENSION_URL__: string;

export class ClarityDebug extends DebugSession {
  forwardMessage: (msg: DebugProtocol.ProtocolMessage) => void;
  messagesQueue: DebugProtocol.ProtocolMessage[];
  ready: boolean;

  public constructor() {
    super();
    const serverMain = vscode.Uri.joinPath(
      vscode.Uri.parse(__EXTENSION_URL__),
      "server-dap/dist/server.js",
    );

    this.ready = false;
    this.messagesQueue = [];

    const worker = new Worker(serverMain.toString(true));
    worker.postMessage("init");
    worker.onmessage = (ev) => {
      const data = ev.data;
      if (data.type === "event") {
        if (data.method === "worker-ready") {
          this.ready = true;
          this.onWorkerReady();
          return;
        }
      }

      if (data.type === "request") {
        if (data.method.startsWith("vfs/")) {
          const id = data.id;
          fileAccessor(data.method, data.data).then((res) => {
            worker.postMessage({ id, type: "response", data: res });
          });
          return;
        }
      }

      if (data.type === "debugger-response") {
        this.sendResponse(data.res);
        return;
      }
      if (data.type === "debugger-event") {
        this.sendEvent(data.event);
        return;
      }
      console.warn(`unhandled message`, data);
    };

    this.forwardMessage = (message) => {
      const sab = new SharedArrayBuffer(1024);
      const int32 = new Int32Array(sab);
      const res = worker.postMessage({ type: "debug-message", message, int32 });
      setTimeout(() => {
        console.log("go2");
        Atomics.store(int32, 0, 123);
        Atomics.notify(int32, 0, 1);
      }, 5000);
    };
  }

  handleMessage(message: DebugProtocol.ProtocolMessage): void {
    console.log("-".repeat(20), message.type);
    if (!this.ready) {
      this.messagesQueue.push(message);
      return;
    }
    this.forwardMessage(message);

    //   // setTimeout(() => {
    //   //   console.log("go2");
    //   //   Atomics.store(int32, 0, 123);
    //   //   Atomics.notify(int32, 0, 1);
    //   // }, 2000);
    //   // console.log("res", res);
    //   // if (request.command === "launch") {
    //   //   this.dap.runDap();
    //   // }
  }

  onWorkerReady(): void {
    this.messagesQueue.forEach((message) => this.handleMessage(message));
  }
}

// function pathToUri(path: string) {
//   try {
//     return vscode.Uri.file(path);
//   } catch (e) {
//     return vscode.Uri.parse(path);
//   }
// }

async function fileAccessor(action: string, event: any) {
  const { fs } = vscode.workspace;

  if (action === "vsf/exists") {
    try {
      await fs.stat(vscode.Uri.parse(event.path));
      return true;
    } catch {
      return false;
    }
  }
  if (action === "vfs/readFile") {
    const content = fileArrayToString(
      await fs.readFile(vscode.Uri.parse(event.path)),
    );
    return content;
  }
  if (action === "vfs/readFiles") {
    const files = await Promise.all(
      event.paths.map(async (p: string) => {
        try {
          const contract = await fs.readFile(vscode.Uri.parse(p));
          return contract;
        } catch (err) {
          console.warn(err);
          return null;
        }
      }),
    );
    return Object.fromEntries(
      files.reduce((acc, f, i) => {
        if (f === null) return acc;
        return acc.concat([[event.paths[i], fileArrayToString(f)]]);
      }, [] as [string, string][]),
    );
  }
  console.warn(`unexpected vfs action ${action}`);
  return false;
}
