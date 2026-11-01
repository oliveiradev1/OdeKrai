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

  it('expõe a porta adicional com limites inteiros e informa que a descoberta é loopback', () => {
    const markup = renderToStaticMarkup(
      <LocalAiProviderCard
        status={localServiceStatus(false, 0, true)}
        port="9931"
        busy={false}
        onPortChange={() => undefined}
        onProbe={() => undefined}
        onOpenComputer={() => undefined}
      />,
    );

    expect(markup).toContain('value="9931"');
    expect(markup).toContain('min="1024"');
    expect(markup).toContain('max="65535"');
    expect(markup).toContain('step="1"');
    expect(markup).toContain('A busca usa somente 127.0.0.1');
    expect(markup).toContain('Servidor não encontrado');
  });
});
