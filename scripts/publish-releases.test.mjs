import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  appendUnsignedInstallerNotice,
  loadReleaseArtifact,
  parseArguments,
  parseGitHubRepository,
  publishReleases,
  unsignedInstallerNotice,
} from './publish-releases.mjs';

const roots = [];
const testToken = 'simulated-release-token';
const apiBase = 'https://api.github.test';
const uploadHost = 'uploads.github.test';

async function createFixture({ artifactHash = null } = {}) {
  const root = await mkdtemp(join(tmpdir(), 'odekrai-release-fixture-'));
  roots.push(root);
  const releasesDirectory = join(root, 'docs', 'releases');
  const artifactDirectory = join(root, 'release-artifact');
  await mkdir(releasesDirectory, { recursive: true });
  await mkdir(artifactDirectory, { recursive: true });
  await writeFile(join(releasesDirectory, 'v0.1.0.md'), '# v0.1.0 — Inicial\n\nNotas antigas.\n');
  await writeFile(
    join(releasesDirectory, 'v0.2.0.md'),
    '# v0.2.0 — Recursos\n\nNotas da versão.\n',
  );
  const installerName = 'OdeKrai_0.2.0_x64-setup.exe';
  const installerBytes = Buffer.from('fixture installer bytes');
  await writeFile(join(artifactDirectory, installerName), installerBytes);
  const correctHash = createHash('sha256').update(installerBytes).digest('hex');
  await writeFile(
    join(artifactDirectory, 'SHA256SUMS.txt'),
    `${artifactHash ?? correctHash}  ${installerName}\n`,
  );
  return { root, artifactDirectory, artifactDir: artifactDirectory, installerName, installerBytes };
}

function createMockGitHub(initialReleases = []) {
  const state = { releases: structuredClone(initialReleases), requests: [], uploads: [] };
  const fetchImpl = vi.fn(async (input, options = {}) => {
    const url = new URL(input);
    const body = Buffer.isBuffer(options.body) ? options.body : Buffer.from(options.body ?? '');
    state.requests.push({
      method: options.method ?? 'GET',
      url: `${url.pathname}${url.search}`,
      headers: options.headers,
      body,
    });
    if (options.headers?.Authorization !== `Bearer ${testToken}`) {
      return { ok: false, status: 401, json: async () => ({ message: 'Unauthorized' }) };
    }

    if (url.hostname === 'api.github.test' && options.method !== 'POST') {
      return { ok: true, status: 200, json: async () => state.releases };
    }
    if (
      url.hostname === 'api.github.test' &&
      options.method === 'POST' &&
      url.pathname === '/repos/test/repo/releases'
    ) {
      const payload = JSON.parse(body.toString('utf8'));
      const release = {
        id: state.releases.length + 20,
        ...payload,
        tag_name: payload.tag_name,
        assets: [],
        upload_url: `https://${uploadHost}/repos/test/repo/releases/${state.releases.length + 20}/assets{?name,label}`,
      };
      state.releases.push(release);
      return { ok: true, status: 201, json: async () => release };
    }
    if (url.hostname === uploadHost && options.method === 'POST') {
      const releaseId = Number(url.pathname.match(/\/releases\/(\d+)\/assets$/)?.[1]);
      const release = state.releases.find((entry) => entry.id === releaseId);
      const name = url.searchParams.get('name');
      if (!release || release.assets.some((asset) => asset.name === name)) {
        return { ok: false, status: 422, json: async () => ({ message: 'Validation failed' }) };
      }
      const asset = { id: state.uploads.length + 100, name, size: body.length };
      release.assets.push(asset);
      state.uploads.push({ name, bytes: body, contentType: options.headers?.['Content-Type'] });
      return { ok: true, status: 201, json: async () => asset };
    }
    return { ok: false, status: 404, json: async () => ({ message: 'Not found' }) };
  });
  return { state, fetchImpl };
}

function gitImpl(root, ...args) {
  expect(root).toBeTruthy();
  expect(args).not.toContain('push');
  if (args[0] === 'remote') return 'https://github.com/test/repo.git';
  if (args[0] === 'ls-remote') return 'abc123 refs/tags/v0.2.0';
  throw new Error(`Comando inesperado: ${args.join(' ')}`);
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

describe('publish-releases', () => {
  it('reconhece remotos GitHub HTTPS e SSH e interpreta flags sem habilitar execução por padrão', () => {
    expect(parseGitHubRepository('https://github.com/Anceryx/OdeKrai.git')).toBe('Anceryx/OdeKrai');
    expect(parseGitHubRepository('git@github.com:Anceryx/OdeKrai.git')).toBe('Anceryx/OdeKrai');
    expect(parseArguments(['--artifacts', 'out/release', '--final'])).toEqual({
      execute: false,
      final: true,
      root: '.',
      artifactDir: 'out/release',
    });
    expect(parseArguments(['--execute', '--dry-run']).execute).toBe(false);
  });

  it('valida a pasta exata e confere o hash do instalador antes de qualquer API', async () => {
    const fixture = await createFixture();
    const artifact = await loadReleaseArtifact(fixture.artifactDirectory);
    expect(artifact.tag).toBe('v0.2.0');
    expect(artifact.files.map(({ name }) => name)).toEqual([
      fixture.installerName,
      'SHA256SUMS.txt',
    ]);

    const badFixture = await createFixture({ artifactHash: '0'.repeat(64) });
    const fetchImpl = vi.fn();
    await expect(
      publishReleases({
        root: badFixture.root,
        artifactDir: badFixture.artifactDirectory,
        execute: true,
        env: { GITHUB_TOKEN: testToken },
        fetchImpl,
        ownerRepo: 'test/repo',
        gitImpl,
      }),
    ).rejects.toThrow('SHA-256 divergente');
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('mantém dry-run como padrão e não chama a API', async () => {
    const fixture = await createFixture();
    const fetchImpl = vi.fn();
    const log = vi.fn();
    const result = await publishReleases({
      ...fixture,
      fetchImpl,
      gitImpl,
      log,
      ownerRepo: 'test/repo',
    });
    expect(result.simulated).toEqual(['v0.2.0']);
    expect(fetchImpl).not.toHaveBeenCalled();
    expect(log).toHaveBeenCalledWith(expect.stringContaining('pré-lançamento'));
  });

  it('cria pré-lançamento, inclui aviso e envia os dois assets em bytes puros', async () => {
    const fixture = await createFixture();
    const mock = createMockGitHub();
    const log = vi.fn();
    const result = await publishReleases({
      ...fixture,
      execute: true,
      env: { GITHUB_TOKEN: testToken },
      fetchImpl: mock.fetchImpl,
      apiBase,
      uploadHost,
      gitImpl,
      log,
      ownerRepo: 'test/repo',
    });
    expect(result).toMatchObject({
      created: ['v0.2.0'],
      uploadedAssets: [fixture.installerName, 'SHA256SUMS.txt'],
      skippedAssets: [],
    });
    const createRequest = mock.state.requests.find(
      (request) => request.url === '/repos/test/repo/releases',
    );
    const payload = JSON.parse(createRequest.body.toString('utf8'));
    expect(payload).toMatchObject({
      tag_name: 'v0.2.0',
      prerelease: true,
      make_latest: 'false',
      draft: false,
      generate_release_notes: false,
    });
    expect(payload.body).toContain('Notas da versão.');
    expect(payload.body).toContain(unsignedInstallerNotice);
    expect(mock.state.uploads.map(({ name }) => name)).toEqual([
      fixture.installerName,
      'SHA256SUMS.txt',
    ]);
    expect(mock.state.uploads[0].bytes).toEqual(fixture.installerBytes);
    expect(
      mock.state.uploads.every(({ contentType }) => contentType === 'application/octet-stream'),
    ).toBe(true);
    expect(log.mock.calls.flat().join(' ')).not.toContain(testToken);
  });

  it('não recria release nem envia novamente assets existentes', async () => {
    const fixture = await createFixture();
    const mock = createMockGitHub([
      {
        id: 31,
        tag_name: 'v0.2.0',
        draft: false,
        prerelease: true,
        assets: [{ id: 101, name: fixture.installerName }],
        upload_url: `https://${uploadHost}/repos/test/repo/releases/31/assets{?name,label}`,
      },
    ]);
    const run = () =>
      publishReleases({
        ...fixture,
        execute: true,
        env: { GITHUB_TOKEN: testToken },
        fetchImpl: mock.fetchImpl,
        apiBase,
        uploadHost,
        gitImpl,
        ownerRepo: 'test/repo',
      });
    const first = await run();
    const second = await run();
    expect(first).toMatchObject({ skipped: ['v0.2.0'], skippedAssets: [fixture.installerName] });
    expect(first.uploadedAssets).toEqual(['SHA256SUMS.txt']);
    expect(second).toMatchObject({
      skipped: ['v0.2.0'],
      skippedAssets: [fixture.installerName, 'SHA256SUMS.txt'],
    });
    expect(
      mock.state.requests.filter(
        ({ method, url }) => method === 'POST' && url === '/repos/test/repo/releases',
      ),
    ).toHaveLength(0);
    expect(mock.state.uploads).toHaveLength(1);
  });

  it('cria release final e só a considera latest quando supera releases finais existentes', async () => {
    const fixture = await createFixture();
    const mock = createMockGitHub([
      { id: 7, tag_name: 'v0.3.0', draft: false, prerelease: false, assets: [] },
    ]);
    await publishReleases({
      ...fixture,
      execute: true,
      final: true,
      env: { GITHUB_TOKEN: testToken },
      fetchImpl: mock.fetchImpl,
      apiBase,
      uploadHost,
      gitImpl,
      ownerRepo: 'test/repo',
    });
    const payload = JSON.parse(
      mock.state.requests
        .find((request) => request.url === '/repos/test/repo/releases')
        .body.toString('utf8'),
    );
    expect(payload.prerelease).toBe(false);
    expect(payload.make_latest).toBe('false');
  });

  it('marca a release final mais nova como latest e ignora pré-lançamentos nessa comparação', async () => {
    const fixture = await createFixture();
    const mock = createMockGitHub([
      { id: 8, tag_name: 'v0.1.0', draft: false, prerelease: false, assets: [] },
      { id: 9, tag_name: 'v0.3.0', draft: false, prerelease: true, assets: [] },
    ]);
    await publishReleases({
      ...fixture,
      execute: true,
      final: true,
      env: { GITHUB_TOKEN: testToken },
      fetchImpl: mock.fetchImpl,
      apiBase,
      uploadHost,
      gitImpl,
      ownerRepo: 'test/repo',
    });
    const payload = JSON.parse(
      mock.state.requests
        .find((request) => request.url === '/repos/test/repo/releases')
        .body.toString('utf8'),
    );
    expect(payload.prerelease).toBe(false);
    expect(payload.make_latest).toBe('true');
  });

  it('acrescenta o aviso fixo uma única vez', () => {
    const body = appendUnsignedInstallerNotice('Notas.');
    expect(body.match(/Instalador sem assinatura digital/g)).toHaveLength(1);
    expect(appendUnsignedInstallerNotice(body)).toBe(body);
  });
});
