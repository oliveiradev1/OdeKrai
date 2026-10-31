import { readFile, readdir, writeFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;

function compareVersions(left, right) {
  const a = left.split('.').map(Number);
  const b = right.split('.').map(Number);
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return b[index] - a[index];
  }
  return 0;
}

function parseReleaseNote(filename, source) {
  const heading = source.match(/^# v(\d+\.\d+\.\d+)(?: — |: )(.+)$/m);
  const expectedVersion = filename.match(/^v(\d+\.\d+\.\d+)\.md$/)?.[1];
  if (!expectedVersion || !heading || heading[1] !== expectedVersion || !heading[2].trim()) {
    throw new Error(
      `${filename}: título deve seguir “# vX.Y.Z — Título” ou “# vX.Y.Z: Título” e corresponder ao nome do arquivo.`,
    );
  }

  const plainSectionNames = [
    'Importante antes de instalar',
    'Novidades',
    'Correções',
    'Limitações conhecidas',
    'Verificação',
  ];
  const plainSectionPattern = new RegExp(`^(${plainSectionNames.join('|')})\\s*$`, 'gm');
  const plainSectionMatches = [...source.matchAll(plainSectionPattern)];
  const plainFormat = plainSectionMatches.length > 0;
  const date = source.match(
    /^Data do (?:marco|commit de fechamento): (\d{4}-\d{2}-\d{2})\.$/m,
  )?.[1];
  const parsedDate = date ? new Date(`${date}T00:00:00Z`) : null;
  if (!plainFormat) {
    if (
      !date ||
      !parsedDate ||
      Number.isNaN(parsedDate.valueOf()) ||
      parsedDate.toISOString().slice(0, 10) !== date
    ) {
      throw new Error(`${filename}: informe “Data do marco: AAAA-MM-DD.”.`);
    }
  }

  if (plainFormat) {
    const summaryEnd = plainSectionMatches[0].index;
    const summary = source
      .slice(heading.index + heading[0].length, summaryEnd)
      .trim()
      .replace(/\s+/g, ' ');
    if (!summary) throw new Error(`${filename}: inclua um resumo após o título.`);

    const sections = new Map();
    for (let index = 0; index < plainSectionMatches.length; index += 1) {
      const current = plainSectionMatches[index];
      const start = current.index + current[0].length;
      const end = plainSectionMatches[index + 1]?.index ?? source.length;
      sections.set(current[1], source.slice(start, end).trim());
    }

    const changes = [];
    for (const sectionName of ['Novidades', 'Correções']) {
      const content = sections.get(sectionName);
      if (!content) continue;
      for (const line of content.split(/\r?\n/)) {
        if (!line.trim()) continue;
        if (!/^[-*]\s+/.test(line)) {
          throw new Error(
            `${filename}: itens de “${sectionName}” devem ser listas com marcadores.`,
          );
        }
        const change = line.replace(/^[-*]\s+/, '').trim();
        if (!change)
          throw new Error(`${filename}: itens de “${sectionName}” não podem estar vazios.`);
        changes.push(change);
      }
    }
    if (changes.length === 0) {
      throw new Error(`${filename}: inclua mudanças em “Novidades” ou “Correções”.`);
    }

    return {
      version: heading[1] ?? '',
      title: heading[2]?.trim() ?? '',
      date: '',
      summary,
      changes: changes.map((change) => change ?? ''),
    };
  }

  const sections = new Map();
  const sectionMatches = [...source.matchAll(/^## (.+)\s*$/gm)];
  for (let index = 0; index < sectionMatches.length; index += 1) {
    const current = sectionMatches[index];
    const start = current.index + current[0].length;
    const end = sectionMatches[index + 1]?.index ?? source.length;
    sections.set(current[1].trim(), source.slice(start, end).trim());
  }
  const summaryText = sections.get('Resumo para pessoas') ?? sections.get('Resumo');
  if (!summaryText)
    throw new Error(`${filename}: seção “## Resumo para pessoas” ausente ou vazia.`);
  const summary = summaryText
    .split(/\r?\n/)
    .map((line) => line.replace(/^[-*](?:\s+|$)/, '').trim())
    .filter(Boolean)
    .join(' ');
  if (!summary.trim()) {
    throw new Error(`${filename}: a seção de resumo não pode estar vazia.`);
  }
  const changes = [];
  for (const sectionName of ['Adicionado', 'Alterado', 'Corrigido']) {
    const content = sections.get(sectionName);
    if (!content) continue;
    for (const line of content.split(/\r?\n/)) {
      if (!line.trim()) continue;
      if (/^[-*]$/.test(line.trim())) {
        throw new Error(`${filename}: itens de “${sectionName}” não podem estar vazios.`);
      }
      if (!/^[-*]\s+/.test(line)) {
        throw new Error(`${filename}: itens de “${sectionName}” devem ser listas com marcadores.`);
      }
      const change = line.replace(/^[-*]\s+/, '').trim();
      if (!change) {
        throw new Error(`${filename}: itens de “${sectionName}” não podem estar vazios.`);
      }
      if (change.toLocaleLowerCase('pt-BR').startsWith('a lista completa dos commits abaixo')) {
        continue;
      }
      changes.push(change);
    }
  }
  if (changes.length === 0) {
    throw new Error(
      `${filename}: inclua ao menos uma mudança em Adicionado, Alterado ou Corrigido.`,
    );
  }

  return {
    version: heading?.[1] ?? '',
    title: heading?.[2]?.trim() ?? '',
    date: date ?? '',
    summary: summary ?? '',
    changes: changes.map((change) => change ?? ''),
  };
}

export async function buildReleaseNotes(directory, currentVersion) {
  if (!VERSION_PATTERN.test(currentVersion)) {
    throw new Error(`Versão do app inválida: ${currentVersion}.`);
  }
  const entries = await readdir(directory, { withFileTypes: true });
  const files = entries.filter((entry) => entry.isFile() && entry.name.endsWith('.md'));
  if (files.length === 0) throw new Error(`Nenhuma nota Markdown encontrada em ${directory}.`);

  const releases = await Promise.all(
    files.map(async (entry) =>
      parseReleaseNote(entry.name, await readFile(path.join(directory, entry.name), 'utf8')),
    ),
  );
  const versions = new Set();
  for (const release of releases) {
    if (versions.has(release.version)) throw new Error(`Nota duplicada para v${release.version}.`);
    versions.add(release.version);
  }
  if (!versions.has(currentVersion)) {
    throw new Error(
      `Nota docs/releases/v${currentVersion}.md não encontrada para a versão do app.`,
    );
  }
  releases.sort((left, right) => compareVersions(left.version, right.version));
  return { currentVersion, releases };
}

async function main() {
  const packageJson = JSON.parse(await readFile(path.join(root, 'package.json'), 'utf8'));
  const manifest = await buildReleaseNotes(
    path.join(root, 'docs', 'releases'),
    packageJson.version,
  );
  await writeFile(
    path.join(root, 'src', 'core', 'release-notes.generated.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8',
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
