import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { findForbiddenArtifacts } from './check-release-artifact.mjs';

const apiBase = 'https://api.github.com';
const checksumName = 'SHA256SUMS.txt';
const installerPattern = /^OdeKrai_(\d+\.\d+\.\d+)_x64-setup\.exe$/;
export const unsignedInstallerNotice =
  "Instalador sem assinatura digital. O Windows pode mostrar o aviso 'O Windows protegeu o computador' (clique em Mais informações → Executar assim mesmo).";
const headers = {
  Accept: 'application/vnd.github+json',
  'X-GitHub-Api-Version': '2026-03-10',
  'Content-Type': 'application/json',
};

export function parseGitHubRepository(remote) {
  const match = remote.match(/(?:github\.com[:/])([^/]+)\/([^/]+?)(?:\.git)?$/i);
  if (!match) throw new Error('A origem Git precisa apontar para um repositório GitHub.');
  return `${match[1]}/${match[2]}`;
}

export function parseReleaseNotes(file, body) {
  const match = file.match(/^v(\d+\.\d+\.\d+)\.md$/);
  if (!match) throw new Error(`Nome de nota inválido: ${file}`);
  const tag = `v${match[1]}`;
  const title = body.match(/^#\s+(.+)$/m)?.[1];
  if (!title) throw new Error(`Título ausente nas notas de ${tag}.`);
  return { tag, title, body };
}

function versionParts(tag) {
  const match = tag.match(/^v(\d+)\.(\d+)\.(\d+)$/);
  return match ? match.slice(1).map(Number) : null;
}

export function compareVersions(a, b) {
  const left = versionParts(a);
  const right = versionParts(b);
  if (!left || !right) throw new Error(`Versão inválida para comparação: ${!left ? a : b}`);
  return left[0] - right[0] || left[1] - right[1] || left[2] - right[2];
}

export async function loadReleases(root) {
  const directory = path.resolve(root, 'docs/releases');
  const files = (await readdir(directory)).filter((file) => /^v\d+\.\d+\.\d+\.md$/.test(file));
  const releases = await Promise.all(
    files.map(async (file) =>
      parseReleaseNotes(file, await readFile(path.resolve(directory, file), 'utf8')),
    ),
  );
  return releases.sort((a, b) => compareVersions(a.tag, b.tag));
}

export async function loadReleaseArtifact(artifactDirectory) {
  const directory = path.resolve(artifactDirectory);
  const violations = await findForbiddenArtifacts([directory], { mode: 'release' });
  if (violations.length) {
    throw new Error(`Pasta de release inválida:\n${violations.join('\n')}`);
  }

  const entries = await readdir(directory, { withFileTypes: true });
  const installerNames = entries
    .map(({ name }) => name)
    .filter((name) => installerPattern.test(name));
  const installerName = installerNames[0];
  const version = installerName.match(installerPattern)?.[1];
  if (!installerName || !version) {
    throw new Error('A pasta deve conter um instalador OdeKrai NSIS com versão SemVer.');
  }

  const checksumText = await readFile(path.join(directory, checksumName), 'utf8');
  const lines = checksumText.trim().split(/\r?\n/);
  const checksum = lines.length === 1 ? lines[0].match(/^([a-f\d]{64})  ([^/\\]+)$/i) : null;
  if (!checksum || checksum[2] !== installerName) {
    throw new Error(
      `${checksumName} deve conter somente o SHA-256 do instalador ${installerName}.`,
    );
  }

  const installerBytes = await readFile(path.join(directory, installerName));
  const actualHash = createHash('sha256').update(installerBytes).digest('hex');
  if (actualHash.toLowerCase() !== checksum[1].toLowerCase()) {
    throw new Error(`SHA-256 divergente para ${installerName}; nenhum arquivo será enviado.`);
  }

  return {
    tag: `v${version}`,
    files: [
      { name: installerName, path: path.join(directory, installerName), bytes: installerBytes },
      {
        name: checksumName,
        path: path.join(directory, checksumName),
        bytes: Buffer.from(checksumText, 'utf8'),
      },
    ],
  };
}

export function appendUnsignedInstallerNotice(body) {
  const trimmed = body.trimEnd();
  if (trimmed.endsWith(unsignedInstallerNotice)) return trimmed;
  return `${trimmed}${trimmed ? '\n\n' : ''}${unsignedInstallerNotice}`;
}

function validateUploadUrl(template, ownerRepo, uploadHost) {
  if (typeof template !== 'string')
    throw new Error('A API não retornou o endereço de upload da release.');
  const rawUrl = template.replace(/\{\?name,label\}$/, '');
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    throw new Error('A API retornou um endereço de upload inválido.');
  }
  const [owner, repo] = ownerRepo.split('/');
  const parts = url.pathname.split('/');
  if (
    url.hostname !== uploadHost ||
    (url.protocol !== 'https:' && uploadHost !== '127.0.0.1' && uploadHost !== 'localhost') ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    parts.length !== 7 ||
    parts[0] !== '' ||
    parts[1] !== 'repos' ||
    parts[2].toLowerCase() !== owner.toLowerCase() ||
    parts[3].toLowerCase() !== repo.toLowerCase() ||
    parts[4] !== 'releases' ||
    !/^\d+$/.test(parts[5]) ||
    parts[6] !== 'assets'
  ) {
    throw new Error('O endereço de upload não pertence à release esperada do GitHub.');
  }
  return url;
}

export function createReleaseClient({
  fetchImpl = fetch,
  token,
  apiBase: base = apiBase,
  uploadHost = 'uploads.github.com',
}) {
  async function request(url, options = {}) {
    const response = await fetchImpl(url, {
      ...options,
      redirect: 'error',
      headers: {
        ...headers,
        Authorization: `Bearer ${token}`,
        ...options.headers,
      },
    });
    if (!response.ok) throw new Error(`GitHub API respondeu HTTP ${response.status}.`);
    return response.status === 204 ? null : response.json();
  }

  return {
    async listAll(ownerRepo) {
      const all = [];
      for (let page = 1; ; page += 1) {
        const batch = await request(
          `${base}/repos/${ownerRepo}/releases?per_page=100&page=${page}`,
        );
        if (!Array.isArray(batch))
          throw new Error('Resposta inválida ao listar releases do GitHub.');
        all.push(...batch);
        if (batch.length < 100) return all;
      }
    },
    create(ownerRepo, release) {
      return request(`${base}/repos/${ownerRepo}/releases`, {
        method: 'POST',
        body: JSON.stringify({
          tag_name: release.tag,
          name: release.title,
          body: appendUnsignedInstallerNotice(release.body),
          draft: false,
          prerelease: release.prerelease,
          make_latest: release.makeLatest ? 'true' : 'false',
          generate_release_notes: false,
        }),
      });
    },
    async upload(ownerRepo, release, asset) {
      const url = validateUploadUrl(release.upload_url, ownerRepo, uploadHost);
      url.searchParams.set('name', asset.name);
      return request(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: asset.bytes,
      });
    },
  };
}

function git(root, ...args) {
  return execFileSync('git', args, {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
}

function verifyRemoteTag(root, tag, gitImpl) {
  let result;
  try {
    result = gitImpl(root, 'ls-remote', '--exit-code', '--refs', 'origin', `refs/tags/${tag}`);
  } catch (error) {
    if (error.status === 2) throw new Error(`Tag ${tag} não existe na origem remota.`);
    throw new Error(`Não foi possível verificar a tag ${tag} na origem remota.`);
  }
  if (!result) throw new Error(`Tag ${tag} não existe na origem remota.`);
}

function isNewestFinalRelease(tag, existing) {
  return existing
    .filter((release) => !release.draft && !release.prerelease && versionParts(release.tag_name))
    .every((release) => compareVersions(tag, release.tag_name) >= 0);
}

export async function publishReleases({
  root,
  artifactDir,
  execute = false,
  final = false,
  env = process.env,
  fetchImpl = fetch,
  log = console.log,
  ownerRepo: repoOverride,
  gitImpl = git,
  apiBase: base = apiBase,
  uploadHost = 'uploads.github.com',
}) {
  if (!artifactDir) throw new Error('Informe a pasta do artefato com --artifacts <pasta>.');
  const artifact = await loadReleaseArtifact(artifactDir);
  const releases = await loadReleases(root);
  const release = releases.find(({ tag }) => tag === artifact.tag);
  if (!release) throw new Error(`Não há notas em docs/releases/${artifact.tag}.md.`);

  const ownerRepo =
    repoOverride ?? parseGitHubRepository(gitImpl(root, 'remote', 'get-url', 'origin'));
  verifyRemoteTag(root, release.tag, gitImpl);
  if (!execute) {
    const type = final ? 'release final' : 'pré-lançamento';
    log(
      `[simulação] ${ownerRepo} ${release.tag}: ${release.title} (${type}; ${artifact.files.map(({ name }) => name).join(', ')})`,
    );
    return {
      created: [],
      skipped: [],
      simulated: [release.tag],
      uploadedAssets: [],
      skippedAssets: [],
    };
  }

  const token = env.GITHUB_TOKEN;
  if (!token) throw new Error('Defina GITHUB_TOKEN para executar a publicação.');
  const client = createReleaseClient({ fetchImpl, token, apiBase: base, uploadHost });
  const existingReleases = await client.listAll(ownerRepo);
  const existingRelease = existingReleases.find(({ tag_name }) => tag_name === release.tag);
  const created = [];
  const skipped = [];
  const uploadedAssets = [];
  const skippedAssets = [];
  let targetRelease = existingRelease;

  if (targetRelease) {
    skipped.push(release.tag);
    log(`Release já existe: ${release.tag}`);
  } else {
    targetRelease = await client.create(ownerRepo, {
      ...release,
      prerelease: !final,
      makeLatest: final && isNewestFinalRelease(release.tag, existingReleases),
    });
    created.push(release.tag);
    log(`Release ${final ? 'final' : 'pré-lançamento'} criada: ${release.tag}`);
  }

  const existingAssetNames = new Set((targetRelease.assets ?? []).map(({ name }) => name));
  for (const asset of artifact.files) {
    if (existingAssetNames.has(asset.name)) {
      skippedAssets.push(asset.name);
      log(`Anexo já existe, ignorado: ${asset.name}`);
      continue;
    }
    await client.upload(ownerRepo, targetRelease, asset);
    uploadedAssets.push(asset.name);
    log(`Anexo enviado: ${asset.name}`);
  }

  return { created, skipped, simulated: [], uploadedAssets, skippedAssets };
}

export function parseArguments(args) {
  const result = { execute: false, final: false, root: '.', artifactDir: null };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--execute') result.execute = true;
    else if (arg === '--final') result.final = true;
    else if (arg === '--dry-run') result.execute = false;
    else if (arg.startsWith('--root=')) result.root = arg.slice('--root='.length);
    else if (arg === '--root') result.root = args[++index];
    else if (arg.startsWith('--artifacts=')) result.artifactDir = arg.slice('--artifacts='.length);
    else if (arg === '--artifacts') result.artifactDir = args[++index];
    else throw new Error(`Argumento desconhecido: ${arg}`);
  }
  return result;
}

async function main() {
  try {
    const args = parseArguments(process.argv.slice(2));
    await publishReleases({ ...args, root: path.resolve(args.root) });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
