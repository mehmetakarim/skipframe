/**
 * Collect the licences of everything SkipFrame ships, into one file the app can show offline.
 *
 * SkipFrame is MIT, and so is most of what it is built on — but MIT, BSD and Apache all ask that
 * their notice travels with the binary. The About window's "Lisanslar" reads what this writes.
 *
 * Both halves of the app are covered: the npm packages that end up in the bundle and the Rust
 * crates that end up in the executable. Licence text is taken from the package's own files where
 * there are any; where there are none, the SPDX identifier the package declares is recorded, so
 * a missing file is visible rather than silently dropped.
 *
 *   node scripts/third-party-licenses.mjs
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'src-tauri', 'licenses', 'THIRD-PARTY.txt');

const LICENCE_FILES = /^(licen[cs]e|copying|notice)([-._]|$)/i;

/** The licence text sitting in a package directory, if it ships one. */
function licenceTextIn(dir) {
  if (!dir || !existsSync(dir)) return null;
  const names = readdirSync(dir).filter((n) => LICENCE_FILES.test(n));
  const texts = names
    .map((n) => {
      try {
        return readFileSync(join(dir, n), 'utf8').trim();
      } catch {
        return '';
      }
    })
    .filter(Boolean);
  return texts.length > 0 ? texts.join('\n\n') : null;
}

// -- npm ---------------------------------------------------------------------------------------

/**
 * Production dependencies, from the lockfile — the tree as installed, which is what ships.
 *
 * The lockfile rather than `npm ls`: it is already on disk, it is what CI installs from, and it
 * does not depend on being able to spawn npm from a script.
 */
function npmPackages() {
  const lock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'));
  const found = [];

  for (const [path, info] of Object.entries(lock.packages ?? {})) {
    if (path === '' || info.dev || info.extraneous) continue;
    const dir = join(root, path);
    let manifest = {};
    try {
      manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
    } catch {
      // Not installed on this machine; the lockfile still knows what it is.
    }
    found.push({
      name: path.replace(/^(.*\/)?node_modules\//, ''),
      version: info.version ?? manifest.version ?? '?',
      licence: info.license ?? manifest.license ?? 'bilinmiyor',
      text: licenceTextIn(dir),
    });
  }
  return found.sort((a, b) => a.name.localeCompare(b.name));
}

// -- cargo -------------------------------------------------------------------------------------

/** Crates linked into the executable, with the licence text from the vendored source. */
function cargoPackages() {
  const json = execFileSync(
    'cargo',
    ['metadata', '--format-version', '1', '--filter-platform', hostTriple()],
    { cwd: join(root, 'src-tauri'), encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );
  const meta = JSON.parse(json);
  const workspace = new Set(meta.workspace_members);
  return meta.packages
    .filter((p) => !workspace.has(p.id))
    .map((p) => ({
      name: p.name,
      version: p.version,
      licence: p.license ?? (p.license_file ? 'dosyada' : 'bilinmiyor'),
      text: licenceTextIn(p.manifest_path ? dirname(p.manifest_path) : null),
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function hostTriple() {
  const out = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  return out.match(/^host:\s*(\S+)$/m)?.[1] ?? 'x86_64-unknown-linux-gnu';
}

// -- writing -----------------------------------------------------------------------------------

/**
 * The same licence text ships with hundreds of crates. Listing it once per package turned a
 * readable file into two and a half megabytes of repeated MIT, so identical texts are pooled and
 * the packages that carry them are named above each one.
 */
function pool(packages) {
  const texts = new Map();
  for (const p of packages) {
    if (!p.text) continue;
    const key = p.text.replace(/\s+/g, ' ').trim();
    if (!texts.has(key)) texts.set(key, { text: p.text, users: [] });
    texts.get(key).users.push(`${p.kind}: ${p.name} ${p.version}`);
  }
  return [...texts.values()].sort((a, b) => b.users.length - a.users.length);
}

function inventory(title, packages) {
  const lines = [`${'='.repeat(78)}\n${title}\n${'='.repeat(78)}\n`];
  for (const p of packages) {
    lines.push(`  ${p.name} ${p.version} — ${p.licence}${p.text ? '' : '  (lisans metni yok)'}`);
  }
  lines.push('');
  return lines.join('\n');
}

const npm = npmPackages().map((p) => ({ ...p, kind: 'npm' }));
const cargo = cargoPackages().map((p) => ({ ...p, kind: 'crate' }));
const pooled = pool([...npm, ...cargo]);

const header = `SkipFrame — üçüncü taraf lisanslar

SkipFrame MIT lisanslıdır. Uygulama aşağıdaki açık kaynak projeleri içerir. Önce hangi paketin
hangi lisansla geldiği listelenir, sonra lisans metinleri gelir; aynı metni paylaşan paketler
metnin başında birlikte anılır.

Oluşturulma: ${new Date().toISOString().slice(0, 10)}
Paket sayısı: ${npm.length} npm, ${cargo.length} Rust

`;

const body = pooled
  .map(
    ({ text, users }) => `${'-'.repeat(78)}\n${users.join('\n')}\n${'-'.repeat(78)}\n\n${text}\n`,
  )
  .join('\n');

mkdirSync(dirname(out), { recursive: true });
writeFileSync(
  out,
  header +
    inventory('npm paketleri', npm) +
    inventory('Rust paketleri (crates)', cargo) +
    `${'='.repeat(78)}\nLisans metinleri\n${'='.repeat(78)}\n\n` +
    body,
  'utf8',
);

const size = readFileSync(out).length;
console.log(
  `${out}: ${npm.length} npm + ${cargo.length} crates, ` +
    `${pooled.length} ayrı lisans metni, ${(size / 1024).toFixed(0)} kB`,
);
