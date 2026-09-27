// A tiny static file server, for verifying the frontend the way it is actually
// served (over HTTP) rather than over file://, where Chrome blocks ES modules
// with a CORS error and the screenshot proves nothing.
//
// This lives IN THE REPOSITORY on purpose. It used to sit in a scratch directory
// outside it, which meant the smoke test could only run on the machine it was
// written on -- so it could never run in CI, which is where a smoke test earns
// its keep.
//
// Usage: node scripts/serve-static.mjs <root-dir> [port]

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

const root = process.argv[2];
const port = Number(process.argv[3] ?? 8791);

if (!root) {
  console.error('usage: node scripts/serve-static.mjs <root-dir> [port]');
  process.exit(1);
}

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
};

createServer(async (req, res) => {
  try {
    const urlPath = decodeURIComponent((req.url ?? '/').split('?')[0]);
    const safe = normalize(urlPath).replace(/^([/\\])+/, '');
    const file = join(root, safe);
    const body = await readFile(file);
    res.writeHead(200, { 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('not found');
  }
}).listen(port, '127.0.0.1', () => {
  console.log(`serving ${root} at http://127.0.0.1:${port}`);
});
