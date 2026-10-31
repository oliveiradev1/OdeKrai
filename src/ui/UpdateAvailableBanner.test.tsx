import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { UpdateAvailableBanner } from './UpdateAvailableBanner';

describe('aviso de atualização', () => {
  it('mostra versão e resumo como texto, com ação de abrir a release', () => {
    const markup = renderToStaticMarkup(
      <UpdateAvailableBanner
        update={{
          current_version: '0.15.0',
          available_version: '0.16.0',
          release_url: 'https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.0',
          notes_summary: '<b>Novidades</b>',
          check_succeeded: true,
        }}
        onOpen={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(markup).toContain('Versão 0.16.0 disponível');
    expect(markup).toContain('&lt;b&gt;Novidades&lt;/b&gt;');
    expect(markup).not.toContain('<b>Novidades</b>');
    expect(markup).toContain('Ver e baixar');
    expect(markup).toContain('Dispensar aviso de atualização');
  });

  it('não monta o aviso quando não há versão mais nova', () => {
    const markup = renderToStaticMarkup(
      <UpdateAvailableBanner
        update={{
          current_version: '0.15.0',
          available_version: null,
          release_url: null,
          notes_summary: null,
          check_succeeded: true,
        }}
        onOpen={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(markup).toBe('');
  });

  it('não anuncia atualização com dados antigos quando a última consulta falhou', () => {
    const markup = renderToStaticMarkup(
      <UpdateAvailableBanner
        update={{
          current_version: '0.15.0',
          available_version: '0.16.0',
          release_url: 'https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.0',
          notes_summary: 'Dados em cache',
          check_succeeded: false,
        }}
        onOpen={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(markup).toBe('');
  });
});
