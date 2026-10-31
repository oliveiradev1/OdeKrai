import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const permissionFor = {
  show: 'core:window:allow-show',
  setFocus: 'core:window:allow-set-focus',
  unminimize: 'core:window:allow-unminimize',
  close: 'core:window:allow-close',
  startDragging: 'core:window:allow-start-dragging',
  isVisible: 'core:window:allow-is-visible',
  isMinimized: 'core:window:allow-is-minimized',
  isFocused: 'core:window:allow-is-focused',
  onFocusChanged: 'core:event:allow-listen',
  getAllWindows: 'core:window:allow-get-all-windows',
};

export function findWindowCalls(source) {
  const calls = [];
  const direct = /getCurrentWindow\s*\(\)\s*\.\s*([\w$]+)\s*\(/g;
  for (const match of source.matchAll(direct)) calls.push(match[1]);
  for (const match of source.matchAll(/\bgetAllWindows\s*\(/g)) calls.push('getAllWindows');
  const aliases = [
    ...source.matchAll(/\b(?:const|let|var)\s+([\w$]+)\s*=\s*getCurrentWindow\s*\(\)/g),
  ].map((match) => match[1]);
  for (const alias of aliases) {
    const callsOnAlias = new RegExp(`\\b${alias}\\s*\\.\\s*([\\w$]+)\\s*\\(`, 'g');
    for (const match of source.matchAll(callsOnAlias)) calls.push(match[1]);
  }
  return calls;
}

export function findMissingWindowPermissions(calls, permissions) {
  const allowed = new Set(permissions);
  return [
    ...new Set(
      calls
        .map((call) => permissionFor[call] ?? `window-api:unknown:${call}`)
        .filter(
          (permission) => permission.startsWith('window-api:unknown:') || !allowed.has(permission),
        ),
    ),
  ];
}

const dialogPermissionFor = {
  message: 'dialog:allow-message',
  ask: 'dialog:allow-message',
  confirm: 'dialog:allow-message',
};

/** Retorna chamadas ao plugin, incluindo imports nomeados, aliases e namespace. */
export function findDialogCalls(source) {
  const calls = [];
  const moduleName = '@tauri-apps/plugin-dialog';
  const namedImport = new RegExp(`import\\s*\\{([^}]+)\\}\\s*from\\s*['"]${moduleName}['"]`, 'g');
  for (const match of source.matchAll(namedImport)) {
    for (const specifier of match[1].split(',')) {
      const [importedNameRaw, localNameRaw] = specifier.trim().split(/\s+as\s+/);
      const importedName = importedNameRaw.trim();
      const localName = (localNameRaw ?? importedNameRaw).trim();
      if (!Object.hasOwn(dialogPermissionFor, importedName)) continue;
      const callPattern = new RegExp(`\\b${localName}\\s*\\(`, 'g');
      for (const _ of source.matchAll(callPattern)) calls.push(importedName);
    }
  }

  const namespaces = [
    ...source.matchAll(
      new RegExp(`import\\s+\\*\\s+as\\s+([\\w$]+)\\s+from\\s+['"]${moduleName}['"]`, 'g'),
    ),
  ].map((match) => match[1]);
  for (const namespace of namespaces) {
    for (const method of Object.keys(dialogPermissionFor)) {
      const callPattern = new RegExp(`\\b${namespace}\\s*\\.\\s*${method}\\s*\\(`, 'g');
      for (const _ of source.matchAll(callPattern)) calls.push(method);
    }
  }

  const globalPattern = /window\.__TAURI__\.dialog\.(message|ask|confirm)\s*\(/g;
  for (const match of source.matchAll(globalPattern)) calls.push(match[1]);
  return calls;
}

export function findMissingDialogPermissions(calls, permissions) {
  const allowed = new Set(permissions);
  return [
    ...new Set(
      calls
        .map((call) => dialogPermissionFor[call] ?? `dialog-api:unknown:${call}`)
        .filter(
          (permission) => permission.startsWith('dialog-api:unknown:') || !allowed.has(permission),
        ),
    ),
  ];
}

async function sourceFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await sourceFiles(file)));
    else if (/\.[jt]sx?$/.test(entry.name)) files.push(file);
  }
  return files;
}

const capabilities = await Promise.all(
  (await readdir(path.join(root, 'src-tauri/capabilities')))
    .filter((name) => name.endsWith('.json'))
    .map(async (name) =>
      JSON.parse(await readFile(path.join(root, 'src-tauri/capabilities', name), 'utf8')),
    ),
);
let failed = false;
for (const file of await sourceFiles(path.join(root, 'src'))) {
  const source = await readFile(file, 'utf8');
  const calls = findWindowCalls(source);
  const dialogCalls = findDialogCalls(source);
  if (!calls.length && !dialogCalls.length) continue;
  const window =
    /(?:getCurrentWindow\(\)|\bcurrentWindow)\.label\s*===\s*['"](pet|approval)['"]/.exec(
      source,
    )?.[1] ?? 'main';
  const permissions = capabilities
    .filter((capability) => capability.windows.includes(window))
    .flatMap((capability) => capability.permissions);
  for (const permission of findMissingWindowPermissions(calls, permissions)) {
    failed = true;
    console.error(
      `${path.relative(root, file)} (${window}): chamada exige ${permission} em uma capability dessa janela.`,
    );
  }
  for (const permission of findMissingDialogPermissions(dialogCalls, permissions)) {
    failed = true;
    console.error(
      `${path.relative(root, file)} (${window}): chamada ao plugin de diálogo exige ${permission} em uma capability dessa janela.`,
    );
  }
}
if (failed) process.exitCode = 1;
else console.log('Chamadas de janela e de diálogo cobertas pelas capabilities correspondentes.');
