import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { RiskBadge } from './RiskBadge';

describe('RiskBadge', () => {
  it.each([
    ['leitura', 'Leitura', 'risk-badge--leitura'],
    ['escrita_reversivel', 'Escrita reversível', 'risk-badge--escrita_reversivel'],
    ['comunicação externa', 'Comunicação externa', 'risk-badge--comunicacao_externa'],
    ['destrutivo_financeiro', 'Destrutivo ou financeiro', 'risk-badge--destrutivo_financeiro'],
    ['execução de código (alto)', 'Execução de código (alto)', 'risk-badge--execucao_de_codigo'],
    ['captura_sensivel', 'Captura sensível', 'risk-badge--captura_sensivel'],
  ])('mapeia %s para rótulo e categoria próprios', (risk, label, className) => {
    const markup = renderToStaticMarkup(<RiskBadge risk={risk} />);
    expect(markup).toContain(`class="risk-badge ${className}`);
    expect(markup).toContain(`aria-label="Risco: ${label}"`);
    expect(markup).toContain(`>${label}</span>`);
    expect(markup).toContain('aria-hidden="true"');
  });

  it('não atribui categoria falsa a um risco desconhecido', () => {
    const markup = renderToStaticMarkup(<RiskBadge risk="futuro" />);
    expect(markup).toContain('Risco: Risco não identificado');
    expect(markup).toContain('risk-badge--unknown');
  });
});
