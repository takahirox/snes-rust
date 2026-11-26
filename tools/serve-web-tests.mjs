// Production artifacts plus one same-origin, test-only AudioWorklet observer.
// The observer never enters web/dist or the uploaded Pages artifact.
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
const types = {
  'index.html': 'text/html', 'app.js': 'text/javascript', 'saves.js': 'text/javascript', 'style.css': 'text/css',
  'snes_wasm.wasm': 'application/wasm', 'shionari.sfc': 'application/octet-stream', 'yonaki.sfc': 'application/octet-stream',
};
const files = new Map(Object.entries(types).map(([name, type]) => [
  name === 'index.html' ? '/' : `/${name}`, { file: new URL(`../web/dist/${name}`, import.meta.url), type },
]));
files.set('/queued-volume-probe.js', { file: new URL('../web/tests/queued-volume-probe.js', import.meta.url), type: 'text/javascript' });
createServer((request, response) => {
  const entry = files.get(new URL(request.url, 'http://localhost').pathname);
  if (!entry || !['GET', 'HEAD'].includes(request.method)) { response.writeHead(404).end(); return; }
  try {
    const body = readFileSync(entry.file);
    response.writeHead(200, { 'Content-Type': entry.type, 'Content-Length': body.length });
    response.end(request.method === 'HEAD' ? undefined : body);
  } catch { response.writeHead(500).end(); }
}).listen(Number(process.argv[2] ?? 8080), '127.0.0.1');
