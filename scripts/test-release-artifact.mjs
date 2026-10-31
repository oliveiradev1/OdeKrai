import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { findForbiddenArtifacts } from './check-release-artifact.mjs';
import { checkFeatureConfiguration } from './check-release-features.mjs';

const allowedRoot = await mkdtemp(path.join(os.tmpdir(), 'odekrai-artifact-good-'));
try {
  await writeFile(path.join(allowedRoot, 'odekrai.exe'), 'binary');
  await writeFile(path.join(allowedRoot, 'SHA256SUMS.txt'), 'hash');
  await writeFile(path.join(allowedRoot, 'LEIA-ME.txt'), 'instruções');
  assert.deepEqual(await findForbiddenArtifacts([allowedRoot]), []);
} finally {
  await rm(allowedRoot, { recursive: true, force: true });
}

const releaseRoot = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-good-'));
try {
  await writeFile(path.join(releaseRoot, 'OdeKrai_0.15.0_x64-setup.exe'), 'installer');
  await writeFile(path.join(releaseRoot, 'SHA256SUMS.txt'), 'hash  OdeKrai_0.15.0_x64-setup.exe\n');
  assert.deepEqual(await findForbiddenArtifacts([releaseRoot], { mode: 'release' }), []);
} finally {
  await rm(releaseRoot, { recursive: true, force: true });
}

const rejectedReleaseRoot = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-bad-'));
try {
  await writeFile(path.join(rejectedReleaseRoot, 'odekrai.exe'), 'loose build executable');
  await writeFile(path.join(rejectedReleaseRoot, 'SHA256SUMS.txt'), 'hash  odekrai.exe\n');
  await writeFile(path.join(rejectedReleaseRoot, 'LEIA-ME.txt'), 'not allowed in a release');
  const violations = await findForbiddenArtifacts([rejectedReleaseRoot], { mode: 'release' });
  assert.ok(violations.some((item) => item.includes('instalador NSIS')));
  assert.ok(violations.some((item) => item.includes('executável')));
} finally {
  await rm(rejectedReleaseRoot, { recursive: true, force: true });
}

const rejectedRoot = await mkdtemp(path.join(os.tmpdir(), 'odekrai-artifact-bad-'));
try {
  await mkdir(path.join(rejectedRoot, 'assets-src'));
  await writeFile(path.join(rejectedRoot, 'model.glb'), 'binary');
  await writeFile(path.join(rejectedRoot, 'model.FBX'), 'binary');
  await writeFile(path.join(rejectedRoot, 'model.obj'), 'text');
  await writeFile(path.join(rejectedRoot, 'random.exe'), 'test executable');
  const violations = await findForbiddenArtifacts([rejectedRoot]);
  assert.ok(violations.some((item) => item.includes('assets-src')));
  assert.ok(violations.some((item) => item.endsWith('model.glb')));
  assert.ok(violations.some((item) => item.endsWith('model.FBX')));
  assert.ok(violations.some((item) => item.endsWith('model.obj')));
  assert.ok(violations.some((item) => item.includes('random.exe')));
} finally {
  await rm(rejectedRoot, { recursive: true, force: true });
}

const rejectedInstallerRoot = await mkdtemp(path.join(os.tmpdir(), 'odekrai-installer-bad-'));
try {
  await writeFile(path.join(rejectedInstallerRoot, 'OdeKrai_0.15.0_x64-setup.exe'), 'installer');
  await writeFile(path.join(rejectedInstallerRoot, 'SHA256SUMS.txt'), 'hash');
  await writeFile(path.join(rejectedInstallerRoot, 'model.glb'), 'prohibited model');
  await mkdir(path.join(rejectedInstallerRoot, 'assets-src'));
  const violations = await findForbiddenArtifacts([rejectedInstallerRoot], { mode: 'release' });
  assert.ok(violations.some((item) => item.includes('model.glb')));
  assert.ok(violations.some((item) => item.includes('assets-src')));
} finally {
  await rm(rejectedInstallerRoot, { recursive: true, force: true });
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const [tauriConfig, cargoToml, releaseWorkflow] = await Promise.all([
  readFile(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8').then(JSON.parse),
  readFile(path.join(root, 'src-tauri/Cargo.toml'), 'utf8'),
  readFile(path.join(root, '.github/workflows/release-windows.yml'), 'utf8'),
]);
assert.deepEqual(tauriConfig.bundle.targets, ['nsis']);
assert.equal(tauriConfig.bundle.createUpdaterArtifacts, false);
assert.equal(tauriConfig.bundle.resources, undefined);
assert.equal(tauriConfig.bundle.externalBin, undefined);
assert.equal(tauriConfig.bundle.windows.nsis.installMode, 'currentUser');
assert.equal(tauriConfig.bundle.windows.certificateThumbprint, null);
assert.equal(tauriConfig.bundle.windows.digestAlgorithm, null);
assert.equal(tauriConfig.bundle.windows.signCommand, null);
assert.equal(tauriConfig.bundle.windows.timestampUrl, null);
assert.equal(tauriConfig.bundle.windows.tsp, false);
assert.equal(tauriConfig.plugins?.updater, undefined);
assert.deepEqual(checkFeatureConfiguration(cargoToml), []);
assert.match(releaseWorkflow, /npm run check:version/);
assert.match(releaseWorkflow, /npm run check:release-features/);
assert.match(releaseWorkflow, /cargo tree --manifest-path src-tauri\/Cargo\.toml/);
assert.match(releaseWorkflow, /tauri feature "devtools"/);
assert.match(releaseWorkflow, /node scripts\/check-release-artifact\.mjs --release \$stage/);
assert.match(releaseWorkflow, /actions\/upload-artifact@/);
assert.doesNotMatch(
  releaseWorkflow,
  /--features diagnostics|TAURI_SIGNING_PRIVATE_KEY|gh release create|softprops\/action-gh-release/,
);
assert.match(releaseWorkflow, /permissions:\s+contents:\s+write/);
assert.match(releaseWorkflow, /workflow_dispatch:/);
assert.match(releaseWorkflow, /tags:[\s\S]*?- 'v\*'/);

const featureConfiguration = `[features]\ndefault = []\ndiagnostics = ["tauri/devtools"]\n\n[dependencies]\ntauri = { version = "2", features = ["tray-icon"] }\n`;
assert.deepEqual(checkFeatureConfiguration(featureConfiguration), []);
assert.notDeepEqual(
  checkFeatureConfiguration(
    featureConfiguration.replace('default = []', 'default = ["diagnostics"]'),
  ),
  [],
);
assert.notDeepEqual(
  checkFeatureConfiguration(
    featureConfiguration.replace(
      'features = ["tray-icon"]',
      'features = ["tray-icon", "devtools"]',
    ),
  ),
  [],
);
assert.notDeepEqual(
  checkFeatureConfiguration(featureConfiguration.replace('diagnostics =', 'release-debug =')),
  [],
);

console.log('Testes de artefatos e features de release aprovados.');
