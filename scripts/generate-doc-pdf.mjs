#!/usr/bin/env node

import { createServer } from "node:http";
import { readFile, readdir, stat } from "node:fs/promises";
import {
  dirname,
  extname,
  join,
  normalize,
  posix as pathPosix,
  relative,
  resolve,
  sep,
} from "node:path";
import { chromium } from "playwright";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 0;

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
  if (!Number.isFinite(args.port) || args.port < 0) {
    console.error(`Invalid --port value: ${args.port}`);
    printUsageAndExit(2);
  }

  return args;
}

function printUsageAndExit(code) {
  console.error(
    "Usage: generate-doc-pdf.mjs --docs-dir <target/doc> --entry <crate/index.html> --output <out.pdf> [--host 127.0.0.1] [--port 0]",
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
    case ".webp":
      return "image/webp";
    default:
      return "application/octet-stream";
  }
}

function toPosixPath(filePath) {
  return filePath.split(sep).join("/");
}

function escapeHtml(input) {
  return input
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
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
    server.listen(port, host, () => {
      const addr = server.address();
      if (!addr || typeof addr === "string") {
        rejectServer(new Error("Failed to resolve bound server port."));
        return;
      }
      resolveServer({ server, port: addr.port });
    });
  });
}

function pageRank(relPath, entryPath, allPath) {
  if (relPath === entryPath) return 0;
  if (relPath === allPath) return 1;
  if (relPath.endsWith("/index.html")) return 2;
  if (/\/fn\..+\.html$/.test(relPath)) return 3;
  return 4;
}

async function collectCrateHtmlPages(docsDir, crateDir, entryPath) {
  const crateRoot = resolve(docsDir, crateDir);
  const crateInfo = await stat(crateRoot).catch(() => null);
  if (!crateInfo?.isDirectory()) {
    throw new Error(`Crate docs directory not found: ${crateRoot}`);
  }

  const pages = [];

  async function walk(dirPath) {
    const entries = await readdir(dirPath, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = join(dirPath, entry.name);
      if (entry.isDirectory()) {
        await walk(fullPath);
      } else if (entry.isFile() && entry.name.endsWith(".html")) {
        const rel = toPosixPath(relative(resolve(docsDir), fullPath));
        pages.push(rel);
      }
    }
  }

  await walk(crateRoot);

  const allPath = `${crateDir}/all.html`;
  pages.sort((a, b) => {
    const rankA = pageRank(a, entryPath, allPath);
    const rankB = pageRank(b, entryPath, allPath);
    if (rankA !== rankB) return rankA - rankB;
    return a.localeCompare(b);
  });
  return pages;
}

async function extractPageContent(page, url, relPath) {
  await page.goto(url, { waitUntil: "networkidle", timeout: 120_000 });
  await page.waitForLoadState("networkidle", { timeout: 120_000 });
  await page.evaluate(() => document.fonts?.ready);
  await page.evaluate(async () => {
    const diagrams = Array.from(document.querySelectorAll(".rustdoc-mermaid"));
    if (diagrams.length === 0) return;
    const deadline = Date.now() + 20_000;
    while (Date.now() < deadline) {
      if (diagrams.every((node) => node.querySelector("svg"))) return;
      await new Promise((resolveWait) => setTimeout(resolveWait, 150));
    }
  });
  await page.waitForTimeout(200);

  return page.evaluate((pathLabel) => {
    const contentNode = document.querySelector("main .content");
    const headingText =
      document.querySelector("main .main-heading h1")?.textContent?.trim() ??
      document.title;

    const stylesheetHrefs = Array.from(
      document.querySelectorAll('link[rel="stylesheet"]'),
    ).map((link) => link.href);

    if (!contentNode) {
      return {
        relPath: pathLabel,
        heading: headingText,
        html: `<p>Unable to extract main content from ${pathLabel}</p>`,
        stylesheets: stylesheetHrefs,
      };
    }

    const clone = contentNode.cloneNode(true);
    clone.querySelectorAll("rustdoc-toolbar,#copy-path").forEach((node) => node.remove());

    // Rewrite links and asset references to absolute URLs so stitched content remains navigable.
    clone.querySelectorAll("[href]").forEach((node) => {
      const value = node.getAttribute("href");
      if (!value) return;
      try {
        node.setAttribute("href", new URL(value, document.baseURI).href);
      } catch {
        // Keep original value if URL resolution fails.
      }
    });
    clone.querySelectorAll("[src]").forEach((node) => {
      const value = node.getAttribute("src");
      if (!value) return;
      try {
        node.setAttribute("src", new URL(value, document.baseURI).href);
      } catch {
        // Keep original value if URL resolution fails.
      }
    });

    return {
      relPath: pathLabel,
      heading: headingText,
      html: clone.innerHTML,
      stylesheets: stylesheetHrefs,
    };
  }, relPath);
}

function buildMergedHtmlDocument(pages, stylesheetHrefs) {
  const links = [...stylesheetHrefs]
    .map((href) => `<link rel="stylesheet" href="${escapeHtml(href)}">`)
    .join("\n");

  const sections = pages
    .map((page) => {
      const pageTitle = escapeHtml(page.heading);
      const pagePath = escapeHtml(page.relPath);
      return `
        <section class="doc-page">
          <header class="doc-page-meta">
            <h1 class="doc-page-title">${pageTitle}</h1>
            <div class="doc-page-path">${pagePath}</div>
          </header>
          <div class="doc-page-content">${page.html}</div>
        </section>
      `;
    })
    .join("\n");

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>git-sync rustdoc (flattened)</title>
  ${links}
  <style>
    body {
      margin: 0;
      padding: 0;
      background: #ffffff;
      color: #111111;
    }
    .doc-page {
      margin: 0;
      padding: 16px 20px 20px;
      break-after: page;
      page-break-after: always;
    }
    .doc-page:last-child {
      break-after: auto;
      page-break-after: auto;
    }
    .doc-page-meta {
      margin: 0 0 12px;
      padding: 0 0 8px;
      border-bottom: 1px solid #c7c7c7;
    }
    .doc-page-title {
      margin: 0;
      font-size: 1.05rem;
      font-weight: 600;
    }
    .doc-page-path {
      margin-top: 4px;
      font-size: 0.8rem;
      color: #5a5a5a;
      font-family: monospace;
    }
    .doc-page-content .width-limiter {
      max-width: none !important;
      margin: 0 !important;
    }
    .doc-page-content .main-heading {
      margin-top: 0 !important;
    }
    .doc-page-content rustdoc-toolbar,
    .doc-page-content #copy-path {
      display: none !important;
    }
  </style>
</head>
<body>
  ${sections}
</body>
</html>`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const docsDir = resolve(args.docsDir);
  const outputPath = resolve(args.output);
  const entryPath = toPosixPath(args.entry.replace(/^[/\\]+/, ""));
  const crateDir = toPosixPath(dirname(entryPath));
  if (!crateDir || crateDir === "." || crateDir === "/") {
    throw new Error(
      `--entry must point to a crate page like git_sync/index.html (received: ${args.entry})`,
    );
  }

  const pagePaths = await collectCrateHtmlPages(docsDir, crateDir, entryPath);
  if (pagePaths.length === 0) {
    throw new Error(`No HTML pages found under crate docs directory: ${crateDir}`);
  }

  const { server, port } = await createStaticServer(docsDir, args.host, args.port);
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const extractor = await browser.newPage({
      viewport: { width: 1440, height: 1000 },
    });

    const extractedPages = [];
    const stylesheets = new Set();
    for (const relPath of pagePaths) {
      const url = `http://${args.host}:${port}/${relPath}`;
      const extracted = await extractPageContent(extractor, url, relPath);
      extractedPages.push(extracted);
      for (const href of extracted.stylesheets) {
        stylesheets.add(href);
      }
      console.log(`Collected: ${relPath}`);
    }
    await extractor.close();

    const mergedHtml = buildMergedHtmlDocument(extractedPages, stylesheets);
    const outPage = await browser.newPage({
      viewport: { width: 1440, height: 1000 },
    });
    await outPage.setContent(mergedHtml, { waitUntil: "networkidle" });
    await outPage.evaluate(() => document.fonts?.ready);
    await outPage.waitForTimeout(500);

    await outPage.pdf({
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
    await outPage.close();

    console.log(`Generated flattened docs PDF (${extractedPages.length} pages): ${outputPath}`);
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
