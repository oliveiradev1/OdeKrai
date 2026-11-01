import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { localServiceStatus } from '../core/provider-directory';
import { LocalAiProviderCard } from './LocalAiProviderCard';

describe('cartão de IA local gratuita', () => {
  it.each([
    [false, 0, true, 'Servidor não encontrado'],
    [true, 0, true, 'Servidor ativo, sem modelo carregado'],
    [true, 1, true, 'Conectado'],
  ] as const)('mostra o estado local %s/%s/%s', (online, count, checked, expected) => {
    const status = localServiceStatus(online, count, checked);
    const markup = renderToStaticMarkup(
      <LocalAiProviderCard
        status={status}
        port=""
        busy={false}
        onPortChange={() => undefined}
        onProbe={() => undefined}
        onOpenComputer={() => undefined}
      />,
    );
    expect(markup).toContain('IA local (gratuita)');
    expect(markup).toContain(expected);
    expect(markup).toContain('Procurar IA local');
    expect(markup).toContain('Abrir Meu computador');
  });
});
