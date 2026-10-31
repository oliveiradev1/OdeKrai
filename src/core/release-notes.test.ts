import { describe, expect, it } from 'vitest';
import { decideReleasePrompt, normalizeReleaseNotesManifest } from './release-notes';

describe('aviso de novidades da versão', () => {
  const base = {
    currentVersion: '1.2.0',
    previousVersion: '1.1.0',
    notesAvailable: true,
    hasExistingPreferences: true,
    enabled: true,
  };

  it('preenche com strings vazias os campos ausentes de notas e listas malformadas', () => {
    expect(
      normalizeReleaseNotesManifest({
        currentVersion: '1.2.0',
        releases: [{ version: '1.2.0', changes: [undefined, 'Mudança'] }],
      }),
    ).toEqual({
      currentVersion: '1.2.0',
      releases: [
        {
          version: '1.2.0',
          title: '',
          date: '',
          summary: '',
          changes: ['', 'Mudança'],
        },
      ],
    });
  });

  it('não reabre para a mesma versão já vista', () => {
    expect(decideReleasePrompt({ ...base, previousVersion: '1.2.0' })).toEqual({
      show: false,
      markSeen: true,
      reason: 'same-version',
    });
  });

  it('abre quando uma instalação existente encontra uma versão nova', () => {
    expect(decideReleasePrompt(base)).toMatchObject({ show: true, markSeen: true });
  });

  it('marca a primeira instalação sem abrir o modal', () => {
    expect(
      decideReleasePrompt({ ...base, previousVersion: null, hasExistingPreferences: false }),
    ).toEqual({ show: false, markSeen: true, reason: 'first-install' });
  });

  it('não exibe nem marca como vista uma versão sem nota', () => {
    expect(decideReleasePrompt({ ...base, notesAvailable: false })).toEqual({
      show: false,
      markSeen: false,
      reason: 'notes-missing',
    });
  });

  it('respeita a opção de desativar o aviso e registra a versão atual', () => {
    expect(decideReleasePrompt({ ...base, enabled: false })).toEqual({
      show: false,
      markSeen: true,
      reason: 'disabled',
    });
  });
});
