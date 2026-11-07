import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {basename, dirname, join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {isFileServingAllowed} from 'vite';
import {startDevServer} from '../scripts/dev-server.mjs';
import {DEV_HOST, DEV_PORT, DEV_ID_PATH} from '../scripts/dev-config.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
function fixture() {
 const root = mkdtempSync(join(tmpdir(), 'inzoimm-dev-server-'));
 writeFileSync(join(root, 'index.html'), '<div id="app">Test app</div>');
 writeFileSync(join(root, 'vite.config.ts'), 'export default {optimizeDeps:{noDiscovery:true,include:[]}};');
 return root;
}
function removeFixture(root) {
 const path = realpathSync.native(root);
 assert.equal(dirname(path), realpathSync.native(tmpdir()));
 assert.ok(basename(path).startsWith('inzoimm-dev-server-'));
 rmSync(path, {recursive:true,force:true});
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
 t.after(async () => {await server?.close(); removeFixture(root)});
 server = await startDevServer({root, port});
 assert.equal(await startDevServer({root, port}), null);
 const response = await fetch(`http://${DEV_HOST}:${port}/`);
 assert.equal(response.status, 200);
 assert.match(await response.text(), /id="app"/);
 const identity = await (await fetch(`http://${DEV_HOST}:${port}${DEV_ID_PATH}`)).json();
 assert.equal(identity.app, 'inzoimm');
});

test('Windows short directory names serve the app and reuse the same server as long paths', {skip:process.platform !== 'win32'}, async t => {
 const root = fixture(), outside = fixture();
 let server;
 t.after(async () => {await server?.close(); removeFixture(root); removeFixture(outside)});
 const shortRoot = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
  "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $fso = New-Object -ComObject Scripting.FileSystemObject; $fso.GetFolder($env:INZOIMM_TEST_LONG_PATH).ShortPath"],
 {encoding:'utf8',env:{...process.env,INZOIMM_TEST_LONG_PATH:root}}).trim();
 if (!shortRoot.includes('~')) {t.skip('8.3 short directory names are disabled on this volume'); return}
 const port = await freePort();
 server = await startDevServer({root:shortRoot,port});
 assert.equal((await fetch(`http://${DEV_HOST}:${port}/`)).status, 200);
 assert.equal(await startDevServer({root:realpathSync.native(root),port}), null);
 assert.equal(server.config.server.fs.strict, true);
 const outsideFile = realpathSync.native(join(outside, 'index.html')).replaceAll('\\', '/');
 assert.equal(isFileServingAllowed(server.config, outsideFile), false);
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
 t.after(async () => {await Promise.all(active.map(server => server.close())); removeFixture(root)});
 const servers = await Promise.all([startDevServer({root, port}), startDevServer({root, port})]);
 active.push(...servers.filter(Boolean));
 assert.equal(active.length, 1);
 assert.equal((await fetch(`http://${DEV_HOST}:${port}/`)).status, 200);
});
