import * as vscode from "vscode";
import {
  WorkspaceFolder,
  DebugConfiguration,
  ProviderResult,
  CancellationToken,
} from "vscode";
import { ClarityDebug } from "./debugSession";

export declare const __EXTENSION_URL__: string;

export async function activateClarityDebug(context: vscode.ExtensionContext) {
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
    const clarityDebug = new ClarityDebug();
    return new vscode.DebugAdapterInlineImplementation(clarityDebug);
  }
}
