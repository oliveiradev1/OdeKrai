import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { buildMetadataOutput } from '../../scripts/set-build-metadata.mjs';

describe('metadados do build de release', () => {
  it('inclui o hash do commit e a data UTC na saída gravada pelo workflow', () => {
    const output = buildMetadataOutput(
      'abcdef1234567890abcdef1234567890abcdef12',
      new Date('2026-10-31T12:34:56.789Z'),
    );

    expect(output).toBe(
      'ODEKRAI_BUILD_COMMIT=abcdef123456\nODEKRAI_BUILD_DATE=2026-10-31T12:34:56Z\n',
    );
    const workflow = readFileSync(
      new URL('../../.github/workflows/release-windows.yml', import.meta.url),
      'utf8',
    );
    expect(workflow).toContain(
      'node scripts/set-build-metadata.mjs $env:GITHUB_SHA $env:GITHUB_ENV',
    );
  });
});
