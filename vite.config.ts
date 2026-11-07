import {defineConfig} from 'vite';
import {svelte} from '@sveltejs/vite-plugin-svelte';
import {DEV_HOST, DEV_PORT} from './scripts/dev-config.mjs';

export default defineConfig({
 plugins:[svelte()],
 clearScreen:false,
 server:{host:DEV_HOST,port:DEV_PORT,strictPort:true,watch:{ignored:['**/src-tauri/**','**/work/**']}},
 optimizeDeps:{entries:['index.html']},
 envPrefix:['VITE_','TAURI_ENV_'],
});
