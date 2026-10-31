import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { releaseNotesManifest } from '../core/release-notes-manifest';
import { ReleaseNotesContent } from './ReleaseNotesModal';

describe('modal de novidades da versão', () => {
  it('apresenta apenas resumo e mudanças vindas da nota atual, com ações nomeadas', () => {
    const current = releaseNotesManifest.releases.find(
      (release) => release.version === releaseNotesManifest.currentVersion,
    );
    if (!current) throw new Error('A nota atual deveria existir no manifesto gerado.');
    const markup = renderToStaticMarkup(
      <ReleaseNotesContent
        showAllVersions={false}
        onShowAllVersions={vi.fn()}
        onShowCurrent={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(markup).toContain(current.summary);
    expect(markup).toContain(current.changes[0]);
    expect(markup).toContain('Ver todas as versões');
    expect(markup).toContain('Fechar');
  });

  it('lista todas as versões embutidas no app', () => {
    const markup = renderToStaticMarkup(
      <ReleaseNotesContent
        showAllVersions
        onShowAllVersions={vi.fn()}
        onShowCurrent={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    for (const release of releaseNotesManifest.releases) {
      expect(markup).toContain(`Versão ${release.version}`);
      expect(markup).toContain(release.summary);
    }
    expect(markup).toContain('Voltar às novidades');
  });

  it('não quebra a renderização quando campos do manifesto estão ausentes', () => {
    const markup = renderToStaticMarkup(
      <ReleaseNotesContent
        manifest={{
          currentVersion: '1.2.3',
          releases: [{ version: '1.2.3', changes: [undefined] }],
        }}
        showAllVersions
        onShowAllVersions={vi.fn()}
        onShowCurrent={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(markup).toContain('Versão 1.2.3');
    expect(markup).not.toContain('undefined');
  });
});
