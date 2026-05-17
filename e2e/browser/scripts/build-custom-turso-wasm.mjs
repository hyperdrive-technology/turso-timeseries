import { access, mkdir, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(here, '..');
const checkoutRoot = resolve(packageRoot, '.custom', 'turso-wasm-udf');
const tarballDir = resolve(packageRoot, '.custom', 'packs');
const repo = process.env.TURSO_WASM_UDF_REPO ?? 'https://github.com/glommer/limbo.git';
const ref = process.env.TURSO_WASM_UDF_REF ?? 'turso-wasm-udf';

await assertRustTargetInstalled('wasm32-wasip1-threads');

await mkdir(dirname(checkoutRoot), { recursive: true });
if (!(await exists(join(checkoutRoot, '.git')))) {
  exec('git', ['clone', '--depth', '1', '--branch', ref, repo, checkoutRoot], packageRoot);
} else {
  exec('git', ['fetch', '--depth', '1', 'origin', ref], checkoutRoot);
  exec('git', ['checkout', 'FETCH_HEAD'], checkoutRoot);
}

const jsRoot = join(checkoutRoot, 'bindings', 'javascript');
exec('npm', ['install'], jsRoot);
exec(
  'npm',
  [
    'run',
    'build',
    '--workspace=packages/common',
    '--workspace=packages/wasm-common',
    '--workspace=packages/wasm',
  ],
  jsRoot,
);

await rm(tarballDir, { recursive: true, force: true });
await mkdir(tarballDir, { recursive: true });

const packages = [
  join(jsRoot, 'packages', 'common'),
  join(jsRoot, 'packages', 'wasm-common'),
  join(jsRoot, 'packages', 'wasm'),
];
const tarballs = packages.map((pkg) => {
  const output = exec('npm', ['pack', '--pack-destination', tarballDir], pkg);
  return join(tarballDir, output.trim().split(/\r?\n/u).at(-1));
});

exec('npm', ['install', '--no-save', ...tarballs], packageRoot);

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function assertRustTargetInstalled(target) {
  const sysroot = exec('rustc', ['--print', 'sysroot'], packageRoot).trim();
  const targetLib = join(sysroot, 'lib', 'rustlib', target, 'lib');
  if (await exists(targetLib)) {
    return;
  }

  throw new Error(
    [
      `Rust target stdlib is missing: ${target}`,
      `Expected target libraries under: ${targetLib}`,
      'Install it with `rustup target add wasm32-wasip1-threads`, or use a Rust toolchain distribution that includes that target.',
    ].join('\n'),
  );
}

function exec(command, args, cwd) {
  return execFileSync(command, args, {
    cwd,
    stdio: ['ignore', 'pipe', 'inherit'],
    encoding: 'utf8',
  });
}
