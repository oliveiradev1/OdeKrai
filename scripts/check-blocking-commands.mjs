import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const blockingPatterns = [
  ['reqwest::blocking', /\breqwest\s*::\s*blocking\b/],
  ['TcpListener', /\bTcpListener\b/],
  ['TcpStream', /\bTcpStream\b/],
  ['Command::new', /\bCommand\s*::\s*new\s*\(/],
  ['.spawn()', /\.\s*spawn\s*\(/],
];

function functionBody(source, start) {
  let opening = start;
  while (opening < source.length && source[opening] !== '{') opening += 1;
  if (opening === source.length) return '';

  let depth = 0;
  let state = 'code';
  let rawStringHashes = 0;
  for (let index = opening; index < source.length; index += 1) {
    const char = source[index];
    const next = source[index + 1];

    if (state === 'line-comment') {
      if (char === '\n') state = 'code';
      continue;
    }
    if (state === 'block-comment') {
      if (char === '*' && next === '/') {
        state = 'code';
        index += 1;
      }
      continue;
    }
    if (state === 'string') {
      if (char === '\\') index += 1;
      else if (char === '"') state = 'code';
      continue;
    }
    if (state === 'raw-string') {
      if (
        char === '"' &&
        source.slice(index + 1, index + 1 + rawStringHashes) === '#'.repeat(rawStringHashes)
      ) {
        index += rawStringHashes;
        state = 'code';
      }
      continue;
    }

    if (char === '/' && next === '/') {
      state = 'line-comment';
      index += 1;
      continue;
    }
    if (char === '/' && next === '*') {
      state = 'block-comment';
      index += 1;
      continue;
    }
    if (char === 'r') {
      let cursor = index + 1;
      while (source[cursor] === '#') cursor += 1;
      if (source[cursor] === '"') {
        rawStringHashes = cursor - index - 1;
        state = 'raw-string';
        index = cursor;
        continue;
      }
    }
    if (char === '"') {
      state = 'string';
      continue;
    }
    if (char === '{') depth += 1;
    if (char === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(opening, index + 1);
    }
  }
  return source.slice(opening);
}

export function findSynchronousBlockingCommands(source) {
  const matches = [];
  const commandAttribute = /#\s*\[\s*tauri::command(?:\([^\]]*\))?\s*\]/g;
  for (const attribute of source.matchAll(commandAttribute)) {
    const declarationStart = attribute.index + attribute[0].length;
    const declaration = source.slice(declarationStart);
    const signature = declaration.match(
      /^\s*(?:(?:pub(?:\s*\([^)]*\))?)\s+)?(?:(async)\s+)?fn\s+([A-Za-z_]\w*)\b[^{}]*\{/s,
    );
    if (!signature || signature[1]) continue;

    const bodyStart = declarationStart + signature[0].lastIndexOf('{');
    const body = functionBody(source, bodyStart);
    const calls = blockingPatterns
      .filter(([, pattern]) => pattern.test(body))
      .map(([name]) => name);
    if (calls.length) matches.push({ name: signature[2], calls });
  }
  return matches;
}

function verifyExamples() {
  const goodAsync = `#[tauri::command]\npub async fn good() { reqwest::blocking::get("x"); TcpListener::bind("x"); TcpStream::connect("x"); Command::new("x").spawn(); }`;
  const goodSync = '#[tauri::command] pub fn read_only() { Ok::<_, String>(()) }';
  const bad = `#[tauri::command]\npub fn bad() {\n reqwest::blocking::get("x");\n TcpListener::bind("x"); TcpStream::connect("x");\n Command::new("x").spawn();\n}`;
  const nestedBrace =
    '#[tauri::command] pub fn safe() { let text = "}"; if true { let _ = text; } }';

  assert.deepEqual(findSynchronousBlockingCommands(goodAsync), []);
  assert.deepEqual(findSynchronousBlockingCommands(goodSync), []);
  assert.deepEqual(findSynchronousBlockingCommands(nestedBrace), []);
  assert.deepEqual(findSynchronousBlockingCommands(bad), [
    { name: 'bad', calls: blockingPatterns.map(([name]) => name) },
  ]);
}

async function rustFiles(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await rustFiles(fullPath)));
    else if (entry.isFile() && entry.name.endsWith('.rs')) result.push(fullPath);
  }
  return result;
}

verifyExamples();
const failures = [];
for (const file of await rustFiles(path.join(root, 'src-tauri', 'src'))) {
  const violations = findSynchronousBlockingCommands(await readFile(file, 'utf8'));
  for (const violation of violations) {
    failures.push(`${path.relative(root, file)}: ${violation.name}: ${violation.calls.join(', ')}`);
  }
}

if (failures.length) {
  console.error('Comandos Tauri síncronos com operações bloqueantes:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    'Nenhum comando Tauri síncrono contém operações de rede ou processos bloqueantes; exemplos bom e ruim validados.',
  );
}
