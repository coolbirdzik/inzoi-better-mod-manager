import {realpathSync} from 'node:fs';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {DEV_HOST, DEV_PORT, DEV_ID_PATH} from './dev-config.mjs';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const identity = root => {
 const path = realpathSync.native(root);
 return process.platform === 'win32' ? path.toLowerCase() : path;
};
async function reuseServer(url, id, port) {
 let response;
 try {response = await fetch(`${url}${DEV_ID_PATH}`, {signal: AbortSignal.timeout(10000)})}
 catch (error) {
  if (error.cause?.code !== 'ECONNREFUSED') throw new Error(`Cannot verify the server on ${url}: ${error.message}`);
 }
 if (response) {
  const existing = await response.json().catch(() => null);
  if (existing?.app === 'inzoimm' && existing.root === id) {
   console.log(`Reusing inZOI dev server at ${url}`);
   return true;
  }
  throw new Error(`Port ${port} is occupied by another server. Close it or choose another matching port in scripts/dev-config.mjs and src-tauri/tauri.conf.json.`);
 }
 return false;
}

export async function startDevServer({root = project, port = DEV_PORT} = {}) {
 // Expand Windows 8.3 aliases such as RUNNER~1 before Vite builds its file
 // requests and allow list. Vite's strict file serving rejects paths with '~'.
 root = realpathSync.native(root);
 const url = `http://${DEV_HOST}:${port}`;
 const id = identity(root);
 if (await reuseServer(url, id, port)) return null;
 const server = await createServer({
  root, configFile: join(root, 'vite.config.ts'),
  server: {host: DEV_HOST, port, strictPort: true},
  plugins: [{name: 'inzoimm-dev-identity', configureServer(server) {
   server.middlewares.use(DEV_ID_PATH, (_request, response) => {
    response.setHeader('Content-Type', 'application/json');
    response.setHeader('Cache-Control', 'no-store');
    response.end(JSON.stringify({app: 'inzoimm', root: id}));
   });
  }}],
 });
 try {await server.listen()} catch (error) {
  await server.close();
  if (await reuseServer(url, id, port)) return null;
  throw error;
 }
 server.printUrls();
 return server;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
 startDevServer().then(server => {
  if (server) {
   let closing = false;
   const close = async () => {if (!closing) {closing = true; await server.close()}};
   process.once('SIGINT', close);
   process.once('SIGTERM', close);
  }
 }).catch(error => {console.error(`Dev server: ${error.message}`); process.exitCode = 1});
}
