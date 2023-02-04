import * as vscode from "vscode";
import {
  WorkspaceFolder,
  DebugConfiguration,
  ProviderResult,
  CancellationToken,
} from "vscode";
import { ClarityDebug } from "./debugSession";

import { initSync } from "../clarity-dap-browser/dap-browser";

export declare const __EXTENSION_URL__: string;

export async function activateMockDebug(context: vscode.ExtensionContext) {
  const wasmURL = new URL("client/dist/dap-browser_bg.wasm", __EXTENSION_URL__);

  const wasmModule = fetch(wasmURL, {}).then((wasm) => wasm.arrayBuffer());

  initSync(await wasmModule);

  const provider = new ClarinetConfigurationProvider();
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider("clarinet", provider),
  );

  const factory = new InlineDebugAdapterFactory();
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory("clarinet", factory),
  );
}

class ClarinetConfigurationProvider
  implements vscode.DebugConfigurationProvider
{
  resolveDebugConfiguration(
    folder: WorkspaceFolder | undefined,
    config: DebugConfiguration,
    token?: CancellationToken,
  ): ProviderResult<DebugConfiguration> {
    console.log("config", config);
    console.log("folder", folder?.uri.toString());
    config.manifest = config.manifest.replace(
      "${workspaceFolder}/",
      folder?.uri.toString(),
    );
    config.stopOnEntry = true;
    console.log("config", config);
    return config;
  }
}

class InlineDebugAdapterFactory
  implements vscode.DebugAdapterDescriptorFactory
{
  createDebugAdapterDescriptor(
    _session: vscode.DebugSession,
  ): ProviderResult<vscode.DebugAdapterDescriptor> {
    return new vscode.DebugAdapterInlineImplementation(new ClarityDebug());
  }
}
