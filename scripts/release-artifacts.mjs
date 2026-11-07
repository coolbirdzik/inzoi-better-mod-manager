import {copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = 'coolbirdzik/inzoi-better-mod-manager';
const targets = {
  'windows-x86_64': {folder: '', arch: 'windows-x64', kind: 'nsis', extension: '.exe'},
  'darwin-aarch64': {folder: 'aarch64-apple-darwin', arch: 'macos-arm64', kind: 'dmg', extension: '.dmg'},
  'darwin-x86_64': {folder: 'x86_64-apple-darwin', arch: 'macos-x64', kind: 'dmg', extension: '.dmg'},
};
export function releaseVersion(root = projectRoot, tag) {
  const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error('Release version must be stable X.Y.Z.');
  const config = JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
  const cargo = readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const lock = readFileSync(join(root, 'src-tauri/Cargo.lock'), 'utf8').match(/name = "inzoi-better-mod-manager"\r?\nversion = "([^"]+)"/)?.[1];
  if ([config.version, cargo, lock].some(value => value !== version)) throw new Error('Package, Tauri, Cargo and Cargo.lock versions must agree.');
  if (tag !== undefined && tag !== `v${version}`) throw new Error(`Tag ${tag} does not match v${version}.`);
  return version;
}
function oneFile(folder, extension, version) {
  const files = existsSync(folder) ? readdirSync(folder, {withFileTypes: true}).filter(item => item.isFile() && item.name.endsWith(extension) && (!version || item.name.includes(`_${version}_`))).map(item => join(folder, item.name)) : [];
  if (files.length !== 1) throw new Error(`Expected one ${extension} artifact in ${folder}; found ${files.length}.`);
  return files[0];
}
export function assetNames(version, platform) {
  const target = targets[platform];
  if (!target) throw new Error(`Unsupported release platform: ${platform}`);
  const stem = `inZOI-Better-Mod-Manager_${version}_${target.arch}`;
  return platform === 'windows-x86_64'
    ? {installer: `${stem}-setup.exe`, updater: `${stem}-setup.exe`, portable: `${stem}-portable.zip`}
    : {installer: `${stem}.dmg`, updater: `${stem}.app.tar.gz`};
}
export function collectArtifacts(platform, {root = projectRoot, signed = false} = {}) {
  const version = releaseVersion(root), target = targets[platform];
  if (!target) throw new Error(`Unsupported release platform: ${platform}`);
  const bundle = join(root, 'src-tauri/target', target.folder, 'release/bundle');
  const names = assetNames(version, platform);
  const files = [[oneFile(join(bundle, target.kind), target.extension, version), names.installer]];
  if (platform === 'windows-x86_64') files.push([join(bundle, 'portable', names.portable), names.portable]);
  else if (signed) files.push([oneFile(join(bundle, 'macos'), '.app.tar.gz'), names.updater]);
  if (signed) for (const [source, destination] of [...files]) {
    if (destination.endsWith('.dmg')) continue;
    files.push([`${source}.sig`, `${destination}.sig`]);
  }
  for (const [source] of files) if (!existsSync(source)) throw new Error(`Missing release artifact: ${source}`);
  const output = join(root, 'dist-release', platform);
  mkdirSync(output, {recursive: true});
  for (const [source, destination] of files) copyFileSync(source, join(output, destination));
  return output;
}
export function createManifest(folder, {root = projectRoot, tag, now = new Date()} = {}) {
  const version = releaseVersion(root, tag);
  const platforms = {}, assets = new Set();
  const entry = (platform, filename) => {
    const file = join(folder, filename), signatureFile = `${file}.sig`;
    if (!existsSync(file) || !existsSync(signatureFile)) throw new Error(`Missing signed artifact: ${filename}`);
    const signature = readFileSync(signatureFile, 'utf8').trim();
    if (!signature) throw new Error(`Empty signature: ${filename}.sig`);
    platforms[platform] = {signature, url: `https://github.com/${repository}/releases/download/v${version}/${filename}`};
    assets.add(filename); assets.add(`${filename}.sig`);
  };
  for (const platform of Object.keys(targets)) {
    const names = assetNames(version, platform);
    if (!existsSync(join(folder, names.installer))) throw new Error(`Missing installer: ${names.installer}`);
    assets.add(names.installer);
    entry(platform, names.updater);
    if (names.portable) entry('windows-x86_64-portable', names.portable);
  }
  const manifest = {version, notes: 'See the GitHub release notes for changes.', pub_date: now.toISOString(), platforms};
  writeFileSync(join(folder, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  assets.add('latest.json');
  const checksums = [...assets].sort().map(name => `${createHash('sha256').update(readFileSync(join(folder, name))).digest('hex')}  ${name}`).join('\n');
  writeFileSync(join(folder, 'SHA256SUMS.txt'), `${checksums}\n`);
  return manifest;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [command, value, ...flags] = process.argv.slice(2);
    if (command === 'check-version') console.log(`Version: ${releaseVersion(projectRoot, value)}`);
    else if (command === 'collect') console.log(collectArtifacts(value, {signed: flags.includes('--signed')}));
    else if (command === 'manifest' && value) {
      if (!flags[0]) throw new Error('Supply the release tag after the assets directory.');
      createManifest(resolve(value), {tag: flags[0]}); console.log('Created latest.json and SHA256SUMS.txt; verify signatures before publishing.');
    } else throw new Error('Usage: release-artifacts.mjs check-version [vX.Y.Z] | collect <platform> [--signed] | manifest <assets-dir> <vX.Y.Z>');
  } catch (error) {console.error(error.message); process.exitCode = 1}
}
