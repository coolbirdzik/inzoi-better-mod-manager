// Release tasks for VS Code and the terminal. Pushing vX.Y.Z starts the
// Build and release workflow: test, bundle, verify signatures, then publish.
import {spawnSync} from 'node:child_process';
import {existsSync, readFileSync, writeFileSync} from 'node:fs';
import {dirname, join, resolve} from 'node:path';
import {createInterface} from 'node:readline/promises';
import {fileURLToPath} from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
process.chdir(root);
const usage = 'node scripts/release.mjs <patch|minor|major|X.Y.Z> [--dry-run] [--no-push] [--skip-checks] [--yes]\n       node scripts/release.mjs --check | --build';
const fail = message => {throw new Error(message)};
function run(command, args, {visible = false, env = process.env} = {}) {
  let executable = command, argv = args;
  if (command === 'pnpm') {
    // pnpm 12 supplies its native executable in npm_execpath. Direct VS Code
    // invocations can also use an existing Corepack/pnpm JS shim without a shell.
    const cli = [env.npm_execpath,
      join(dirname(process.execPath), 'node_modules/corepack/dist/pnpm.js'),
      join(dirname(process.execPath), 'node_modules/pnpm/bin/pnpm.cjs')]
      .find(path => path && /pnpm(?:-native|-cli)?\.(?:[cm]?js|exe)$/i.test(path) && existsSync(path));
    if (cli) {
      const js = /\.[cm]?js$/i.test(cli);
      executable = js ? process.execPath : cli;
      argv = js ? [cli, ...args] : args;
    } else if (process.platform === 'win32') fail('Cannot locate pnpm. Run this task with pnpm run release, or install pnpm on PATH.');
  }
  const result = spawnSync(executable, argv, {
    cwd: root, env, encoding: 'utf8', stdio: visible ? 'inherit' : 'pipe',
  });
  if (result.error) fail(`${command}: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} ${args.join(' ')} failed${visible ? '' : `\n${(result.stderr || result.stdout || '').trim()}`}`);
  return (result.stdout ?? '').trim();
}
function checks() {
  console.log('\nRunning release checks…');
  run('pnpm', ['install', '--frozen-lockfile'], {visible: true});
  run('pnpm', ['run', 'test:release'], {visible: true});
  run('pnpm', ['run', 'test:dev'], {visible: true});
  run('pnpm', ['run', 'test:i18n'], {visible: true});
  run('cargo', ['test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml'], {visible: true});
  run('pnpm', ['run', 'build'], {visible: true});
  const env = {...process.env};
  if (process.platform === 'win32' && !env.INZOIMM_TEST_BROWSER
    && [env['ProgramFiles(x86)'], env.ProgramFiles].some(base => base && existsSync(join(base, 'Microsoft/Edge/Application/msedge.exe')))) {
    env.INZOIMM_TEST_BROWSER = 'msedge';
  }
  run('pnpm', ['run', 'test:ui'], {visible: true, env});
}
function build() {
  if (process.platform === 'win32') {
    run('pnpm', ['run', 'tauri', 'build', '--bundles', 'nsis'], {visible: true});
    run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join(root, 'scripts/package-portable.ps1')], {visible: true});
    console.log('\nWindows installer: src-tauri/target/release/bundle/nsis/');
    console.log('Portable ZIP: src-tauri/target/release/bundle/portable/');
  } else if (process.platform === 'darwin') {
    run('pnpm', ['run', 'tauri', 'build', '--bundles', 'app,dmg'], {visible: true});
  } else {
    fail('Local release bundles support Windows and macOS. Push a release tag to build both in CI.');
  }
}
const versionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
function parts(version) {
  if (!versionPattern.test(version)) fail(`Invalid version "${version}". Use stable X.Y.Z; the publish workflow supports stable releases only.`);
  const numbers = version.split('.').map(Number);
  if (!numbers.every(Number.isSafeInteger)) fail('Version numbers are too large.');
  return numbers;
}
function newer(next, current) {
  const a = parts(next), b = parts(current);
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
  return false;
}
// Edit only the matching version text; preserve formatting and line endings.
const specs = [
  ['package.json', /("version"\s*:\s*")([^"]+)(")/],
  ['src-tauri/tauri.conf.json', /("version"\s*:\s*")([^"]+)(")/],
  ['src-tauri/Cargo.toml', /(^version\s*=\s*")([^"]+)(")/m],
  ['src-tauri/Cargo.lock', /(name = "inzoi-better-mod-manager"\r?\nversion = ")([^"]+)(")/],
];
function releaseFiles(current, next) {
  const originals = new Map();
  for (const [file, pattern] of specs) {
    if (!originals.has(file)) originals.set(file, readFileSync(file, 'utf8'));
    const match = pattern.exec(originals.get(file));
    if (!match) fail(`Could not find the app version in ${file}.`);
    if (match[2] !== current) fail(`${file} is at ${match[2]}, but package.json is at ${current}. Fix the mismatch first.`);
  }
  const updated = new Map(originals);
  for (const [file, pattern] of specs) updated.set(file, updated.get(file).replace(pattern, (_, before, _version, after) => `${before}${next}${after}`));
  return {originals, updated};
}
async function main() {
  const args = process.argv.slice(2);
  const allowed = new Set(['--dry-run', '--no-push', '--skip-checks', '--yes', '--check', '--build', '--help']);
  const flags = new Set(args.filter(arg => arg.startsWith('--')));
  const targets = args.filter(arg => !arg.startsWith('--'));
  for (const flag of flags) if (!allowed.has(flag)) fail(`Unknown option ${flag}.\n${usage}`);
  if (flags.has('--help')) {console.log(usage); return}
  if (flags.has('--check') || flags.has('--build')) {
    if (args.length !== 1) fail(usage);
    if (flags.has('--check')) checks(); else build();
    return;
  }
  if (targets.length !== 1) fail(usage);
  const current = JSON.parse(readFileSync('package.json', 'utf8')).version;
  const [major, minor, patch] = parts(current);
  const target = targets[0];
  const next = ({patch: `${major}.${minor}.${patch + 1}`, minor: `${major}.${minor + 1}.0`, major: `${major + 1}.0.0`})[target] ?? target.replace(/^v/, '');
  if (!newer(next, current)) fail(`${next} must be newer than ${current}.`);
  const {originals, updated} = releaseFiles(current, next);
  const files = [...updated.keys()];
  const tag = `v${next}`;
  const push = !flags.has('--no-push');
  const branch = run('git', ['branch', '--show-current']);
  if (branch !== 'main') fail(`Release from main; current branch is "${branch || 'detached HEAD'}".`);
  if (run('git', ['tag', '--list', tag])) fail(`Tag ${tag} already exists locally.`);
  const dirty = run('git', ['status', '--porcelain']);
  console.log(`Release ${current} -> ${next} (${tag})`);
  for (const file of files) console.log(`  ${file}`);
  console.log(push ? 'Then: checks, version commit, annotated tag, atomic push of main + tag.' : 'Then: checks, version commit and annotated local tag (no push).');
  if (flags.has('--dry-run')) {
    if (dirty) console.log('\nWorking tree has changes; commit them before running the release task.');
    console.log('\nDry run: no files, commits, tags or remote refs changed; no network requests.');
    return;
  }
  if (dirty) fail(`Commit or stash your changes before releasing:\n${dirty}`);
  if (push) {
    run('git', ['fetch', 'origin', 'main', '--tags']);
    if (Number(run('git', ['rev-list', '--count', 'HEAD..origin/main'])) > 0) fail('main is behind origin/main. Pull first.');
    if (run('git', ['ls-remote', '--tags', 'origin', `refs/tags/${tag}`])) fail(`Tag ${tag} already exists on origin.`);
  }
  if (!flags.has('--skip-checks')) checks();
  // Checks must not silently add changed sources to a release.
  if (run('git', ['status', '--porcelain'])) fail('Checks changed the working tree. Review and commit those changes before releasing.');
  if (!flags.has('--yes')) {
    if (!process.stdin.isTTY) fail('Interactive confirmation needs a terminal; use --yes for automation.');
    const rl = createInterface({input: process.stdin, output: process.stdout});
    const answer = (await rl.question(`\n${push ? 'Commit, tag and PUSH' : 'Commit and tag'} ${tag}? (y/N) `)).trim().toLowerCase();
    rl.close();
    if (!['y', 'yes'].includes(answer)) fail('Cancelled. No version files, commits or tags changed.');
  }
  const originalHead = run('git', ['rev-parse', 'HEAD']);
  try {
    for (const [file, text] of updated) writeFileSync(file, text);
    run('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--no-deps', '--manifest-path', 'src-tauri/Cargo.toml']);
    run('git', ['add', '--', ...files]);
    run('git', ['commit', '-m', `Release ${tag}`]);
  } catch (error) {
    // Restore only this script's version edits if no release commit was made.
    // Never reset or clean the user's whole working tree.
    if (run('git', ['rev-parse', 'HEAD']) === originalHead) {
      for (const [file, text] of originals) writeFileSync(file, text);
      run('git', ['restore', '--staged', '--', ...files]);
    }
    throw error;
  }
  run('git', ['tag', '-a', tag, '-m', `inZOI Better Mod Manager ${next}`]);
  console.log(`\nCommitted and tagged ${tag}.`);
  if (push) {
    try {run('git', ['push', '--atomic', 'origin', 'main', tag])}
    catch (error) {fail(`${error.message}\nThe local release commit and tag are kept. Retry: git push --atomic origin main ${tag}`)}
    console.log('Pushed main + tag. GitHub Actions builds Windows installer/portable and macOS releases.');
    console.log('Build and release checks signatures and publishes after all Windows and macOS builds succeed.');
  } else console.log(`Push to start release builds: git push --atomic origin main ${tag}`);
}
main().catch(error => {console.error(`\nRelease failed: ${error.message}`); process.exitCode = 1});
