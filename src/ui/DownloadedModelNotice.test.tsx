import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { localServiceStatus } from '../core/provider-directory';
import { DownloadedModelNotice } from './DownloadedModelNotice';
import { LocalAiProviderCard } from './LocalAiProviderCard';

describe('orientação de modelo local baixado', () => {
  it('explica que baixar não inicia o uso e mostra como apontar os servidores para o arquivo', () => {
    const markup = renderToStaticMarkup(<DownloadedModelNotice />);
    expect(markup).toContain('Baixado. Ainda não é usado pelo OdeKrai.');
    expect(markup).toContain('Como usar');
    expect(markup).toContain('ollama create meu-modelo');
    expect(markup).toContain('llama-server.exe');
    expect(markup).toContain('Procurar IA local');
  });

  it('mantém o GGUF baixado separado de um servidor local ausente', () => {
    const markup = renderToStaticMarkup(
      <>
        <DownloadedModelNotice />
        <LocalAiProviderCard
          status={localServiceStatus(false, 0, true)}
          port=""
          busy={false}
          onPortChange={() => undefined}
          onProbe={() => undefined}
          onOpenComputer={() => undefined}
        />
      </>,
    );

    expect(markup).toContain('Estado: Baixado. Ainda não é usado pelo OdeKrai.');
    expect(markup).toContain('Servidor não encontrado');
    expect(markup).not.toContain('Conectado');
  });
});
