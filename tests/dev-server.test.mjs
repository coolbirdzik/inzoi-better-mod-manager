import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {startDevServer} from '../scripts/dev-server.mjs';
import {DEV_HOST, DEV_PORT, DEV_ID_PATH} from '../scripts/dev-config.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
function fixture() {
 const root = mkdtempSync(join(tmpdir(), 'inzoimm-dev-server-'));
 writeFileSync(join(root, 'index.html'), '<div id="app">Test app</div>');
 writeFileSync(join(root, 'vite.config.ts'), 'export default {optimizeDeps:{noDiscovery:true,include:[]}};');
 return root;
}
async function freePort() {
 const server = createServer();
 server.listen(0, DEV_HOST); await once(server, 'listening');
 const port = server.address().port;
 await new Promise(resolve => server.close(resolve));
 return port;
}
async function otherServer(t, body, status = 200) {
 const server = createServer((_req, res) => {res.writeHead(status); res.end(body)});
 server.listen(0, DEV_HOST); await once(server, 'listening');
 t.after(() => {server.closeAllConnections(); return new Promise(resolve => server.close(resolve))});
 return server.address().port;
}

test('Tauri and Vite use the same dedicated dev address', () => {
 const config = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url)));
 assert.equal(config.build.devUrl, `http://${DEV_HOST}:${DEV_PORT}`);
 assert.equal(config.build.beforeDevCommand, 'pnpm run dev');
 assert.notEqual(DEV_PORT, 1420);
});

test('starting dev twice reuses the same project server and still serves the app', async t => {
 const root = fixture();
 const port = await freePort();
 let server;
 t.after(async () => {await server?.close(); rmSync(root, {recursive:true,force:true})});
 server = await startDevServer({root, port});
 assert.equal(await startDevServer({root, port}), null);
 const response = await fetch(`http://${DEV_HOST}:${port}/`);
 assert.equal(response.status, 200);
 assert.match(await response.text(), /id="app"/);
 const identity = await (await fetch(`http://${DEV_HOST}:${port}${DEV_ID_PATH}`)).json();
 assert.equal(identity.app, 'inzoimm');
});

test('an occupied port from a different project is reported without stopping that server', async t => {
 const body = JSON.stringify({app: 'inzoimm', root: '/some/other/project'});
 const port = await otherServer(t, body);
 await assert.rejects(startDevServer({root, port}), /occupied by another server/);
 assert.equal(await (await fetch(`http://${DEV_HOST}:${port}/`)).text(), body);
});

test('an unrelated HTML service is never reused as this app', async t => {
 const port = await otherServer(t, '<html>Other app</html>', 404);
 await assert.rejects(startDevServer({root, port}), /occupied by another server/);
 assert.equal((await fetch(`http://${DEV_HOST}:${port}/`)).status, 404);
});

test('simultaneous dev starts leave one project server on the selected port', async t => {
 const root = fixture();
 const port = await freePort();
 const active = [];
 t.after(async () => {await Promise.all(active.map(server => server.close())); rmSync(root, {recursive:true,force:true})});
 const servers = await Promise.all([startDevServer({root, port}), startDevServer({root, port})]);
 active.push(...servers.filter(Boolean));
 assert.equal(active.length, 1);
 assert.equal((await fetch(`http://${DEV_HOST}:${port}/`)).status, 200);
});
