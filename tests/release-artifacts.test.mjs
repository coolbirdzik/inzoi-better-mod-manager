import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {assetNames, collectArtifacts, createManifest, releaseVersion} from '../scripts/release-artifacts.mjs';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'inzoimm-artifacts-'));
  const write = (file, text) => {const path = join(root, file); mkdirSync(dirname(path), {recursive: true}); writeFileSync(path, text); return path};
  write('package.json', JSON.stringify({version: '0.2.0'}));
  write('src-tauri/tauri.conf.json', JSON.stringify({version: '0.2.0'}));
  write('src-tauri/Cargo.toml', '[package]\nversion = "0.2.0"\n');
  write('src-tauri/Cargo.lock', '[[package]]\nname = "inzoi-better-mod-manager"\nversion = "0.2.0"\n');
  t.after(() => {
    assert.equal(dirname(resolve(root)), resolve(tmpdir()));
    assert.ok(root.startsWith(join(tmpdir(), 'inzoimm-artifacts-')));
    rmSync(root, {recursive: true, force: true});
  });
  return {root, write};
}
function bundles(write, signed = true) {
  const win = 'src-tauri/target/release/bundle';
  const installer = write(`${win}/nsis/inZOI Better Mod Manager_0.2.0_x64-setup.exe`, 'windows installer');
  const portable = write(`${win}/portable/${assetNames('0.2.0', 'windows-x86_64').portable}`, 'portable zip');
  if (signed) {write(`${win}/nsis/inZOI Better Mod Manager_0.2.0_x64-setup.exe.sig`, 'windows signature'); write(`${win}/portable/${assetNames('0.2.0', 'windows-x86_64').portable}.sig`, 'portable signature')}
  for (const [target, arch] of [['aarch64-apple-darwin', 'aarch64'], ['x86_64-apple-darwin', 'x64']]) {
    const mac = `src-tauri/target/${target}/release/bundle`;
    write(`${mac}/dmg/inZOI Better Mod Manager_0.2.0_${arch}.dmg`, `${arch} installer`);
    if (signed) {write(`${mac}/macos/inZOI Better Mod Manager.app.tar.gz`, `${arch} update`); write(`${mac}/macos/inZOI Better Mod Manager.app.tar.gz.sig`, `${arch} signature`)}
  }
  return {installer, portable};
}
function merged(root, signed = true) {
  const folder = join(root, 'merged'); mkdirSync(folder);
  for (const platform of ['windows-x86_64', 'darwin-aarch64', 'darwin-x86_64']) {
    const output = collectArtifacts(platform, {root, signed});
    for (const name of readdirSync(output)) {
      assert.equal(existsSync(join(folder, name)), false, `Artifact collision: ${name}`);
      copyFileSync(join(output, name), join(folder, name));
    }
  }
  return folder;
}

test('every version must agree with the stable release tag before bundling', t => {
  const {root, write} = fixture(t);
  assert.equal(releaseVersion(root, 'v0.2.0'), '0.2.0');
  assert.throws(() => releaseVersion(root, 'v0.2.1'), /does not match/);
  write('src-tauri/Cargo.lock', 'name = "inzoi-better-mod-manager"\nversion = "0.1.9"\n');
  assert.throws(() => releaseVersion(root), /versions must agree/);
});

test('ordinary CI builds collect Windows installer/portable and both Mac DMGs without signing keys', t => {
  const {root, write} = fixture(t); bundles(write, false);
  const folder = merged(root, false);
  assert.equal(readdirSync(folder).length, 4);
  assert.ok(readdirSync(folder).every(name => !name.endsWith('.sig')));
});

test('signed per-architecture artifacts merge without overwriting Mac updater archives', t => {
  const {root, write} = fixture(t); bundles(write);
  const folder = merged(root);
  assert.equal(readdirSync(folder).length, 10);
  assert.equal(readFileSync(join(folder, assetNames('0.2.0', 'darwin-aarch64').updater), 'utf8'), 'aarch64 update');
  assert.equal(readFileSync(join(folder, assetNames('0.2.0', 'darwin-x86_64').updater), 'utf8'), 'x64 update');
});

test('missing signatures and ambiguous bundle inputs fail before writing output', t => {
  const {root, write} = fixture(t); const {installer} = bundles(write, false);
  assert.throws(() => collectArtifacts('windows-x86_64', {root, signed: true}), /Missing release artifact/);
  assert.equal(existsSync(join(root, 'dist-release')), false);
  write('src-tauri/target/release/bundle/nsis/duplicate_0.2.0_x64.exe', 'duplicate');
  assert.throws(() => collectArtifacts('windows-x86_64', {root}), /found 2/);
  assert.equal(readFileSync(installer, 'utf8'), 'windows installer');
});

test('cached installers from an older version are never packaged as the current release', t => {
  const {root, write} = fixture(t); bundles(write, false);
  write('src-tauri/target/release/bundle/nsis/inZOI Better Mod Manager_0.1.9_x64-setup.exe', 'old installer');
  const folder = collectArtifacts('windows-x86_64', {root});
  assert.equal(readFileSync(join(folder, assetNames('0.2.0', 'windows-x86_64').installer), 'utf8'), 'windows installer');
});

test('manifest covers all four updater platforms and checksums match the distributed bytes', t => {
  const {root, write} = fixture(t); bundles(write); const folder = merged(root);
  const manifest = createManifest(folder, {root, tag: 'v0.2.0', now: new Date('2026-11-08T00:00:00Z')});
  assert.deepEqual(Object.keys(manifest.platforms).sort(), ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'windows-x86_64-portable']);
  for (const entry of Object.values(manifest.platforms)) {
    assert.ok(entry.url.startsWith('https://github.com/coolbirdzik/inzoi-better-mod-manager/releases/download/v0.2.0/'));
    assert.ok(existsSync(join(folder, entry.url.split('/').at(-1))));
    assert.ok(entry.signature);
  }
  const checksums = readFileSync(join(folder, 'SHA256SUMS.txt'), 'utf8').trim().split('\n');
  assert.equal(checksums.length, 11);
  for (const line of checksums) {
    const [hash, name] = line.split('  ');
    assert.equal(hash, createHash('sha256').update(readFileSync(join(folder, name))).digest('hex'));
  }
});

test('incomplete releases, empty signatures and tag mismatches cannot generate an update feed', t => {
  const {root, write} = fixture(t); bundles(write); const folder = merged(root);
  assert.throws(() => createManifest(folder, {root, tag: 'v0.2.1'}), /does not match/);
  const names = assetNames('0.2.0', 'darwin-aarch64');
  writeFileSync(join(folder, `${names.updater}.sig`), '');
  assert.throws(() => createManifest(folder, {root, tag: 'v0.2.0'}), /Empty signature/);
  rmSync(join(folder, names.installer));
  assert.throws(() => createManifest(folder, {root, tag: 'v0.2.0'}), /Missing installer/);
  assert.equal(existsSync(join(folder, 'latest.json')), false);
});
