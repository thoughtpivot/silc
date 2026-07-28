import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { ExtensionContext, window, workspace } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const serverPath = resolveServerPath(context);
  if (!serverPath) {
    void window.showErrorMessage(
      "Silc language server (`sil-lsp`) was not found. Build with `cargo build -p sil-lsp --release` " +
        "or set `silc.languageServerPath`."
    );
    return;
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "silc" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.silc"),
    },
  };

  client = new LanguageClient(
    "silc",
    "Silc Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push({
    dispose: () => {
      void client?.stop();
    },
  });

  await client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

function resolveServerPath(context: ExtensionContext): string | undefined {
  const configured = workspace
    .getConfiguration("silc")
    .get<string>("languageServerPath")
    ?.trim();
  if (configured) {
    if (fs.existsSync(configured)) {
      return configured;
    }
    void window.showWarningMessage(
      `silc.languageServerPath does not exist: ${configured}`
    );
  }

  const bundled = path.join(
    context.extensionPath,
    "server",
    bundledServerName()
  );
  if (fs.existsSync(bundled)) {
    return bundled;
  }

  // Dev fallback: workspace target/release or debug.
  const candidates = [
    path.join(
      context.extensionPath,
      "..",
      "..",
      "target",
      "release",
      binaryName()
    ),
    path.join(
      context.extensionPath,
      "..",
      "..",
      "target",
      "debug",
      binaryName()
    ),
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return path.resolve(candidate);
    }
  }

  // Last resort: binary name on PATH (may still fail at spawn).
  return binaryName();
}

function binaryName(): string {
  return os.platform() === "win32" ? "sil-lsp.exe" : "sil-lsp";
}

function bundledServerName(): string {
  const platform = os.platform();
  const arch = os.arch();
  let triple: string;
  if (platform === "darwin" && arch === "arm64") {
    triple = "darwin-arm64";
  } else if (platform === "darwin") {
    triple = "darwin-x64";
  } else if (platform === "linux") {
    triple = arch === "arm64" ? "linux-arm64" : "linux-x64";
  } else if (platform === "win32") {
    triple = arch === "arm64" ? "win32-arm64" : "win32-x64";
  } else {
    triple = `${platform}-${arch}`;
  }
  return platform === "win32" ? `sil-lsp-${triple}.exe` : `sil-lsp-${triple}`;
}
