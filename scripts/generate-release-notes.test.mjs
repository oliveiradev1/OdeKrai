import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { buildReleaseNotes } from './generate-release-notes.mjs';

const validNote = `# v1.2.3 — Marco de teste

Data do marco: 2026-10-29.

## Resumo para pessoas

- Resumo escrito nas notas.

## Adicionado

- Recurso documentado.

## Alterado

- Comportamento documentado.
`;

describe('manifesto de notas de versão', () => {
  let temporaryDirectory = '';

  afterEach(async () => {
    if (temporaryDirectory) await rm(temporaryDirectory, { recursive: true, force: true });
    temporaryDirectory = '';
  });

  it('lê data, resumo e mudanças e ordena as versões mais recentes primeiro', async () => {
    temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-notes-'));
    await writeFile(path.join(temporaryDirectory, 'v1.2.3.md'), validNote);
    await writeFile(
      path.join(temporaryDirectory, 'v1.1.0.md'),
      validNote.replaceAll('1.2.3', '1.1.0').replace('2026-10-29', '2026-10-28'),
    );

    const manifest = await buildReleaseNotes(temporaryDirectory, '1.2.3');
    expect(manifest).toEqual({
      currentVersion: '1.2.3',
      releases: [
        {
          version: '1.2.3',
          title: 'Marco de teste',
          date: '2026-10-29',
          summary: 'Resumo escrito nas notas.',
          changes: ['Recurso documentado.', 'Comportamento documentado.'],
        },
        {
          version: '1.1.0',
          title: 'Marco de teste',
          date: '2026-10-28',
          summary: 'Resumo escrito nas notas.',
          changes: ['Recurso documentado.', 'Comportamento documentado.'],
        },
      ],
    });
    for (const release of manifest.releases) {
      expect(typeof release.version).toBe('string');
      expect(typeof release.title).toBe('string');
      expect(typeof release.date).toBe('string');
      expect(typeof release.summary).toBe('string');
      expect(release.version.trim()).not.toBe('');
      expect(release.title.trim()).not.toBe('');
      expect(release.date.trim()).not.toBe('');
      expect(release.summary.trim()).not.toBe('');
      expect(
        release.changes.every((change) => typeof change === 'string' && change.length > 0),
      ).toBe(true);
    }
  });

  it('falha com erro explícito quando uma nota está mal formatada', async () => {
    temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-notes-'));
    await writeFile(path.join(temporaryDirectory, 'v1.2.3.md'), '# nota inválida');
    await expect(buildReleaseNotes(temporaryDirectory, '1.2.3')).rejects.toThrow(
      'título deve seguir',
    );
  });

  it.each([
    ['data ausente', validNote.replace('Data do marco: 2026-10-29.\n', ''), 'Data do marco'],
    [
      'resumo ausente',
      validNote.replace('- Resumo escrito nas notas.', ''),
      'Resumo para pessoas” ausente ou vazia',
    ],
    [
      'item de lista vazio',
      validNote.replace('- Recurso documentado.', '- '),
      'itens de “Adicionado” não podem estar vazios',
    ],
    [
      'item sem marcador',
      validNote.replace('- Recurso documentado.', 'Recurso documentado.'),
      'itens de “Adicionado” devem ser listas',
    ],
  ])('falha com mensagem clara quando falta %s', async (_name, note, message) => {
    temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-notes-'));
    await writeFile(path.join(temporaryDirectory, 'v1.2.3.md'), note);
    await expect(buildReleaseNotes(temporaryDirectory, '1.2.3')).rejects.toThrow(message);
  });

  it('falha quando a nota da versão embutida não existe', async () => {
    temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), 'odekrai-release-notes-'));
    await writeFile(path.join(temporaryDirectory, 'v1.2.3.md'), validNote);
    await expect(buildReleaseNotes(temporaryDirectory, '2.0.0')).rejects.toThrow(
      'Nota docs/releases/v2.0.0.md não encontrada',
    );
  });
});
