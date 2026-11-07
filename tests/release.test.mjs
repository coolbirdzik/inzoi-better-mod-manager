import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
function exec(cwd, command, args, options = {}) {
  const result = spawnSync(command, args, {cwd, encoding: 'utf8', ...options});
  assert.ifError(result.error);
  return {...result, output: `${result.stdout ?? ''}${result.stderr ?? ''}`};
}
function git(repo, ...args) {
  const result = exec(repo, 'git', args);
  assert.equal(result.status, 0, result.output);
  return result.stdout.trim();
}
function fixture(t, {invalidCargo = false} = {}) {
  const base = mkdtempSync(join(tmpdir(), 'inzoimm release tooling '));
  const repo = join(base, 'app');
  mkdirSync(join(repo, 'scripts'), {recursive: true});
  mkdirSync(join(repo, 'src-tauri/src'), {recursive: true});
  copyFileSync(join(project, 'scripts/release.mjs'), join(repo, 'scripts/release.mjs'));
  writeFileSync(join(repo, 'package.json'), '{\r\n  "name": "inzoi-better-mod-manager",\r\n  "version": "0.1.3",\r\n  "type": "module"\r\n}\r\n');
  writeFileSync(join(repo, 'pnpm-lock.yaml'), "lockfileVersion: '9.0'\nimporters:\n  .: {}\n");
  writeFileSync(join(repo, 'src-tauri/tauri.conf.json'), '{"version":"0.1.3"}\n');
  writeFileSync(join(repo, 'src-tauri/Cargo.toml'), `[package]\nname = "inzoi-better-mod-manager"\nversion = "0.1.3"\nedition = "${invalidCargo ? 'invalid' : '2021'}"\n`);
  writeFileSync(join(repo, 'src-tauri/Cargo.lock'), '# App and unrelated dependency versions\nversion = 4\n\n[[package]]\nname = "inzoi-better-mod-manager"\nversion = "0.1.3"\n');
  writeFileSync(join(repo, 'src-tauri/src/lib.rs'), 'pub fn app() {}\n');
  git(repo, 'init', '--initial-branch=main');
  git(repo, 'config', 'user.name', 'Release tooling test');
  git(repo, 'config', 'user.email', 'release-test@example.invalid');
  git(repo, 'config', 'commit.gpgsign', 'false');
  git(repo, 'config', 'tag.gpgsign', 'false');
  git(repo, 'config', 'core.autocrlf', 'false');
  const hooks = join(base, 'empty-hooks');
  mkdirSync(hooks);
  git(repo, 'config', 'core.hooksPath', hooks);
  git(repo, 'add', '.');
  git(repo, 'commit', '-m', 'Fixture');
  t.after(() => rmSync(base, {recursive: true, force: true}));
  return {repo, base};
}
function release(repo, ...args) {return exec(repo, process.execPath, ['scripts/release.mjs', ...args])}

test('dry run previews patch/minor/major/exact versions without touching a dirty repo or contacting origin', t => {
  const {repo} = fixture(t);
  git(repo, 'remote', 'add', 'origin', 'file:///release-test/nonexistent');
  writeFileSync(join(repo, 'unfinished.txt'), 'keep me');
  const before = git(repo, 'status', '--porcelain');
  const head = git(repo, 'rev-parse', 'HEAD');
  for (const [target, next] of [['patch', '0.1.4'], ['minor', '0.2.0'], ['major', '1.0.0'], ['v0.2.5', '0.2.5']]) {
    const result = release(repo, target, '--dry-run');
    assert.equal(result.status, 0, result.output);
    assert.match(result.output, new RegExp(`Release 0\\.1\\.3 -> ${next.replaceAll('.', '\\.')}`));
  }
  assert.equal(git(repo, 'rev-parse', 'HEAD'), head);
  assert.equal(git(repo, 'status', '--porcelain'), before);
  assert.equal(git(repo, 'tag', '--list'), '');
  assert.equal(existsSync(join(repo, '.git/FETCH_HEAD')), false);
});

test('local release synchronizes every version, preserves formatting and creates an annotated tag', t => {
  const {repo} = fixture(t);
  const originalLock = readFileSync(join(repo, 'pnpm-lock.yaml'), 'utf8');
  const result = release(repo, 'patch', '--no-push', '--skip-checks', '--yes');
  assert.equal(result.status, 0, result.output);
  assert.equal(JSON.parse(readFileSync(join(repo, 'package.json'))).version, '0.1.4');
  assert.equal(readFileSync(join(repo, 'pnpm-lock.yaml'), 'utf8'), originalLock);
  assert.equal(JSON.parse(readFileSync(join(repo, 'src-tauri/tauri.conf.json'))).version, '0.1.4');
  assert.match(readFileSync(join(repo, 'src-tauri/Cargo.toml'), 'utf8'), /version = "0.1.4"/);
  assert.match(readFileSync(join(repo, 'src-tauri/Cargo.lock'), 'utf8'), /version = "0.1.4"/);
  assert.ok(readFileSync(join(repo, 'package.json'), 'utf8').includes('\r\n'));
  assert.equal(git(repo, 'cat-file', '-t', 'v0.1.4'), 'tag');
  assert.equal(git(repo, 'rev-parse', 'v0.1.4^{commit}'), git(repo, 'rev-parse', 'HEAD'));
  assert.equal(git(repo, 'log', '-1', '--pretty=%s'), 'Release v0.1.4');
  assert.equal(git(repo, 'status', '--porcelain'), '');
});

test('push releases update main and the tag together on an isolated local remote', t => {
  const {repo, base} = fixture(t);
  const remote = join(base, 'remote.git');
  git(base, 'init', '--bare', '--initial-branch=main', remote);
  git(repo, 'remote', 'add', 'origin', remote);
  git(repo, 'push', '-u', 'origin', 'main');
  const result = release(repo, '0.2.0', '--skip-checks', '--yes');
  assert.equal(result.status, 0, result.output);
  assert.equal(git(remote, 'rev-parse', 'refs/heads/main'), git(repo, 'rev-parse', 'HEAD'));
  assert.equal(git(remote, 'rev-parse', 'refs/tags/v0.2.0^{commit}'), git(repo, 'rev-parse', 'HEAD'));
});

test('dirty tree, version mismatch, existing tag and wrong branch fail before mutation', t => {
  const {repo} = fixture(t);
  const head = git(repo, 'rev-parse', 'HEAD');
  writeFileSync(join(repo, 'unfinished.txt'), 'keep me');
  assert.match(release(repo, 'patch', '--no-push', '--yes').output, /Commit or stash/);
  rmSync(join(repo, 'unfinished.txt'));
  git(repo, 'checkout', '-b', 'feature');
  assert.match(release(repo, 'patch', '--no-push', '--yes').output, /Release from main/);
  git(repo, 'checkout', 'main');
  git(repo, 'tag', 'v0.1.4');
  assert.match(release(repo, 'patch', '--no-push', '--yes').output, /already exists locally/);
  git(repo, 'tag', '-d', 'v0.1.4');
  writeFileSync(join(repo, 'src-tauri/tauri.conf.json'), '{"version":"0.0.1"}');
  assert.match(release(repo, 'patch', '--dry-run').output, /Fix the mismatch/);
  assert.equal(git(repo, 'rev-parse', 'HEAD'), head);
  assert.equal(JSON.parse(readFileSync(join(repo, 'package.json'))).version, '0.1.3');
  assert.equal(git(repo, 'tag', '--list'), '');
});

test('invalid, old and prerelease versions are rejected', t => {
  const {repo} = fixture(t);
  for (const target of ['0.1.3', '0.0.9', '01.2.3', '1.2', '1.2.3-beta.1', '1.2.3+metadata']) {
    const result = release(repo, target, '--dry-run');
    assert.equal(result.status, 1, result.output);
  }
  assert.equal(release(repo, 'patch', '--unknown').status, 1);
  assert.equal(git(repo, 'status', '--porcelain'), '');
});

test('failed Cargo validation rolls back only the version edits and leaves no commit or tag', t => {
  const {repo} = fixture(t, {invalidCargo: true});
  const head = git(repo, 'rev-parse', 'HEAD');
  const result = release(repo, 'patch', '--no-push', '--skip-checks', '--yes');
  assert.equal(result.status, 1, result.output);
  assert.match(result.output, /cargo metadata/);
  assert.equal(git(repo, 'status', '--porcelain'), '');
  assert.equal(git(repo, 'rev-parse', 'HEAD'), head);
  assert.equal(git(repo, 'tag', '--list'), '');
});

test('noninteractive invocation requires explicit --yes before changing version files', t => {
  const {repo} = fixture(t);
  const result = release(repo, 'patch', '--no-push', '--skip-checks');
  assert.equal(result.status, 1, result.output);
  assert.match(result.output, /Interactive confirmation needs a terminal/);
  assert.equal(git(repo, 'status', '--porcelain'), '');
});

test('check task invokes pnpm without a shell, including paths with spaces', t => {
  const {repo} = fixture(t);
  const cli = join(repo, 'scripts/pnpm-cli.js');
  writeFileSync(cli, "import {appendFileSync} from 'node:fs'; appendFileSync('pnpm-calls.jsonl',JSON.stringify(process.argv.slice(2))+'\\n');\n");
  const result = exec(repo, process.execPath, ['scripts/release.mjs', '--check'], {
    env: {...process.env, npm_execpath: cli},
  });
  assert.equal(result.status, 0, result.output);
  assert.doesNotMatch(result.output, /DEP0190/);
  const calls = readFileSync(join(repo, 'pnpm-calls.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line));
  assert.deepEqual(calls, [['install', '--frozen-lockfile'], ['run', 'test:release'], ['run', 'test:dev'], ['run', 'test:i18n'], ['run', 'build'], ['run', 'test:ui']]);
});
