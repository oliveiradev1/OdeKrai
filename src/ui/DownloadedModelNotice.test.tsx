import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { DownloadedModelNotice } from './DownloadedModelNotice';

describe('orientação de modelo local baixado', () => {
  it('explica que baixar não inicia o uso e mostra como apontar os servidores para o arquivo', () => {
    const markup = renderToStaticMarkup(<DownloadedModelNotice />);
    expect(markup).toContain('Baixado. Ainda não é usado pelo OdeKrai.');
    expect(markup).toContain('Como usar');
    expect(markup).toContain('ollama create meu-modelo');
    expect(markup).toContain('llama-server.exe');
    expect(markup).toContain('Procurar IA local');
  });
});
