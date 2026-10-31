import { readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const forbiddenExtensions = new Set(['.glb', '.fbx', '.obj']);
const checksumName = 'SHA256SUMS.txt';
const diagnosticFiles = ['LEIA-ME.txt', checksumName, 'odekrai.exe'];
const releaseInstallerPattern = /^OdeKrai_\d+\.\d+\.\d+_x64-setup\.exe$/i;

async function inspect(entryPath, violations, { allowDiagnosticExecutable = false } = {}) {
  const details = await stat(entryPath);
  if (path.basename(entryPath).toLowerCase() === 'assets-src') {
    violations.push(`${entryPath} (assets-src)`);
  }
  if (details.isDirectory()) {
    for (const name of await readdir(entryPath)) {
      await inspect(path.join(entryPath, name), violations, {
        allowDiagnosticExecutable: allowDiagnosticExecutable && name === 'odekrai.exe',
      });
    }
  } else {
    const extension = path.extname(entryPath).toLowerCase();
    if (forbiddenExtensions.has(extension)) violations.push(entryPath);
    if (
      extension === '.exe' &&
      !(allowDiagnosticExecutable && path.basename(entryPath) === 'odekrai.exe')
    ) {
      violations.push(`${entryPath} (executável fora do instalador NSIS permitido)`);
    }
  }
}

async function isDiagnosticArtifact(root) {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch {
    return false;
  }
  if (
    entries.length !== diagnosticFiles.length ||
    !diagnosticFiles.every((name) => entries.some((entry) => entry.name === name && entry.isFile()))
  ) {
    return false;
  }
  return true;
}

async function inspectReleaseArtifact(root, violations) {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch {
    violations.push(`${root} (não foi possível ler a pasta do artefato de release)`);
    return;
  }

  const files = entries.filter((entry) => entry.isFile()).map((entry) => entry.name);
  const installerNames = files.filter((name) => releaseInstallerPattern.test(name));
  const exactReleaseSet =
    entries.length === 2 &&
    files.length === 2 &&
    files.includes(checksumName) &&
    installerNames.length === 1;

  if (!exactReleaseSet) {
    violations.push(
      `${root} (release deve conter somente um instalador NSIS OdeKrai_<versão>_x64-setup.exe e ${checksumName})`,
    );
  }

  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      violations.push(`${entryPath} (diretório não permitido no artefato de release)`);
    } else if (entry.name !== checksumName && !releaseInstallerPattern.test(entry.name)) {
      const reason =
        path.extname(entry.name).toLowerCase() === '.exe'
          ? 'executável fora do instalador NSIS permitido'
          : 'arquivo não permitido no artefato de release';
      violations.push(`${entryPath} (${reason})`);
    }
  }
}

export async function findForbiddenArtifacts(roots, { mode = 'diagnostic' } = {}) {
  const violations = [];
  for (const root of roots) {
    const absoluteRoot = path.resolve(root);
    if (mode === 'release') {
      await inspectReleaseArtifact(absoluteRoot, violations);
    } else {
      await inspect(absoluteRoot, violations, {
        allowDiagnosticExecutable: await isDiagnosticArtifact(absoluteRoot),
      });
    }
  }
  return [...new Set(violations)];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const mode = args[0] === '--release' ? 'release' : 'diagnostic';
  const roots = mode === 'release' ? args.slice(1) : args;
  if (roots.length === 0) {
    console.error('Informe ao menos uma pasta para verificar após as opções.');
    process.exit(2);
  }
  const violations = await findForbiddenArtifacts(roots, { mode });
  if (violations.length) {
    console.error(`Artefatos proibidos encontrados:\n${violations.join('\n')}`);
    process.exit(1);
  }
  console.log(`Verificação de artefatos aprovada: ${roots.join(', ')}`);
}
