#!/usr/bin/env node

import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { chromium } from "playwright";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 8787;

function parseArgs(argv) {
  const args = {
    docsDir: "target/doc",
    entry: "",
    output: "",
    host: DEFAULT_HOST,
    port: DEFAULT_PORT,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const value = argv[i];
    if (value === "--docs-dir") {
      args.docsDir = argv[++i] ?? "";
    } else if (value === "--entry") {
      args.entry = argv[++i] ?? "";
    } else if (value === "--output") {
      args.output = argv[++i] ?? "";
    } else if (value === "--host") {
      args.host = argv[++i] ?? DEFAULT_HOST;
    } else if (value === "--port") {
      args.port = Number.parseInt(argv[++i] ?? "", 10);
    } else if (value === "--help" || value === "-h") {
      printUsageAndExit(0);
    } else {
      console.error(`Unknown argument: ${value}`);
      printUsageAndExit(2);
    }
  }

  if (!args.entry || !args.output) {
    console.error("Both --entry and --output are required.");
    printUsageAndExit(2);
  }
  if (!Number.isFinite(args.port) || args.port <= 0) {
    console.error(`Invalid --port value: ${args.port}`);
    printUsageAndExit(2);
  }

  return args;
}

function printUsageAndExit(code) {
  console.error(
    "Usage: generate-doc-pdf.mjs --docs-dir <target/doc> --entry <crate/index.html> --output <out.pdf> [--host 127.0.0.1] [--port 8787]",
  );
  process.exit(code);
}

function contentType(pathname) {
  const ext = extname(pathname).toLowerCase();
  switch (ext) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "application/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".json":
      return "application/json; charset=utf-8";
    case ".woff2":
      return "font/woff2";
    case ".woff":
      return "font/woff";
    case ".ttf":
      return "font/ttf";
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    default:
      return "application/octet-stream";
  }
}

function createStaticServer(rootDir, host, port) {
  const root = resolve(rootDir);
  const server = createServer(async (req, res) => {
    try {
      const rawPath = new URL(req.url ?? "/", `http://${host}:${port}`).pathname;
      const requested = normalize(decodeURIComponent(rawPath)).replace(/^[/\\]+/, "");
      const localPath = resolve(join(root, requested));

      // Keep requests inside docs root.
      if (!localPath.startsWith(root)) {
        res.writeHead(403, { "Content-Type": "text/plain; charset=utf-8" });
        res.end("Forbidden");
        return;
      }

      let targetPath = localPath;
      const fileInfo = await stat(targetPath).catch(() => null);
      if (fileInfo?.isDirectory()) {
        targetPath = join(targetPath, "index.html");
      }

      const bytes = await readFile(targetPath).catch(() => null);
      if (!bytes) {
        res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
        res.end("Not found");
        return;
      }

      res.writeHead(200, { "Content-Type": contentType(targetPath) });
      res.end(bytes);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
      res.end(`Internal server error: ${message}`);
    }
  });

  return new Promise((resolveServer, rejectServer) => {
    server.once("error", rejectServer);
    server.listen(port, host, () => resolveServer(server));
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const docsDir = resolve(args.docsDir);
  const outputPath = resolve(args.output);
  const entryPath = args.entry.replace(/^[/\\]+/, "");
  const targetUrl = `http://${args.host}:${args.port}/${entryPath}`;

  const server = await createStaticServer(docsDir, args.host, args.port);
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({
      viewport: { width: 1440, height: 1000 },
    });

    await page.goto(targetUrl, { waitUntil: "networkidle", timeout: 120_000 });
    await page.waitForLoadState("networkidle", { timeout: 120_000 });

    // Wait for webfonts and Mermaid rendering to stabilize.
    await page.evaluate(() => document.fonts?.ready);
    await page.waitForTimeout(1200);

    await page.pdf({
      path: outputPath,
      format: "A4",
      printBackground: true,
      margin: {
        top: "12mm",
        right: "10mm",
        bottom: "12mm",
        left: "10mm",
      },
    });

    console.log(`Generated docs PDF: ${outputPath}`);
  } finally {
    if (browser) {
      await browser.close();
    }
    await new Promise((resolveClose, rejectClose) => {
      server.close((err) => (err ? rejectClose(err) : resolveClose()));
    });
  }
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack ?? err.message : String(err));
  process.exit(1);
});

