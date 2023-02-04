import { LoggingDebugSession, Response, Event } from "@vscode/debugadapter";
import { DebugProtocol } from "@vscode/debugprotocol";
import * as vscode from "vscode";

import { DapWasmBridge } from "../clarity-dap-browser/dap-browser";
import { fileArrayToString } from "../utils/files";

export class ClarityDebug extends LoggingDebugSession {
  dap: DapWasmBridge;

  public constructor() {
    super();
    const dapDebugger = new DapWasmBridge(
      fileAccessor,
      async (res: Response) => {
        this.sendResponse(res);
      },
      async (event: Event) => {
        console.log("event", event);
        this.sendEvent(event);
      },
    );
    this.dap = dapDebugger;
  }

  handleMessage(msg: DebugProtocol.ProtocolMessage): void {
    console.log("-".repeat(20));
    console.log("msg", msg);
    if (msg.type === "request") {
      const request = msg as DebugProtocol.Request;
      console.log("request", request.command);
      this.dap
        .handleMessage(BigInt(request.seq), request.command, request.arguments)
        .then((res) => console.log("res", res))
        .catch((err) => console.error(err));
    }
  }

  on(eventName: string | symbol, listener: (...args: any[]) => void): this {
    console.log("eventName", eventName);
    return this;
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
  return Promise.resolve(true);
}
