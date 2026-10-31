import { appendFileSync } from 'node:fs';

export function buildMetadataOutput(commitSha, date = new Date()) {
  if (!/^[a-f0-9]{12,40}$/i.test(commitSha)) {
    throw new Error('GITHUB_SHA inválido para identificar o build.');
  }
  const commit = commitSha.slice(0, 12);
  const buildDate = date.toISOString().replace(/\.\d{3}Z$/, 'Z');
  return `ODEKRAI_BUILD_COMMIT=${commit}\nODEKRAI_BUILD_DATE=${buildDate}\n`;
}

if (process.argv[1]?.endsWith('set-build-metadata.mjs')) {
  const [, , commitSha, githubEnv] = process.argv;
  if (!commitSha || !githubEnv) {
    throw new Error('Uso: node scripts/set-build-metadata.mjs GITHUB_SHA GITHUB_ENV');
  }
  const output = buildMetadataOutput(commitSha);
  appendFileSync(githubEnv, output, 'utf8');
  const commit = output.match(/^ODEKRAI_BUILD_COMMIT=(.+)$/m)?.[1];
  const buildDate = output.match(/^ODEKRAI_BUILD_DATE=(.+)$/m)?.[1];
  process.stdout.write(`Build identificado: commit=${commit} data=${buildDate} UTC\n`);
}
