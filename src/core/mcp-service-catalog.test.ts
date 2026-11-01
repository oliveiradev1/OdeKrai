import { describe, expect, it } from 'vitest';
import { getPublicMcpStatus } from './mcp-service-catalog';
import catalog from './mcp-service-catalog.json';

describe('catálogo público de servidores MCP', () => {
  it('contém somente os sete serviços pedidos, com fontes e dados de transporte/autenticação', () => {
    expect(catalog.map(({ name }) => name)).toEqual([
      'GitHub',
      'Figma',
      'Gmail',
      'Google Calendar',
      'Google Sheets',
      'Lovable',
      'Notion',
    ]);

    for (const service of catalog) {
      expect(service.supplier).toBeTruthy();
      expect(service.documentationUrl).toMatch(/^https:\/\//);
      expect(service.endpoint).toMatch(/^https:\/\//);
      expect(service.transport).toBeTruthy();
      expect(service.authentication).toBeTruthy();
      expect(service.authenticationMethods.length).toBeGreaterThan(0);
      expect(['available_to_configure', 'not_available']).toContain(service.status);
    }
  });

  it('não apresenta como disponível no OdeKrai serviços com requisito incompatível documentado', () => {
    const unavailable = catalog
      .filter(({ status }) => status === 'not_available')
      .map(({ name }) => name);

    expect(unavailable).toEqual(['Figma', 'Gmail', 'Google Calendar', 'Google Sheets', 'Lovable']);
  });

  it('identifica o Developer Preview do Google e usa fontes oficiais da Lovable', () => {
    expect(
      catalog
        .filter(({ supplier }) => supplier === 'Google Workspace')
        .map(({ programStatus }) => programStatus),
    ).toEqual(['Developer Preview', 'Developer Preview', 'Developer Preview']);

    const lovable = catalog.find(({ name }) => name === 'Lovable');
    expect(lovable?.endpoint).toBe('https://mcp.lovable.dev');
    expect(lovable?.documentationUrl).toBe(
      'https://docs.lovable.dev/integrations/lovable-mcp-server',
    );
    expect(lovable?.sourceCodeUrl).toBe('https://github.com/lovablelabs/mcp');
  });

  it('só marca conectado após o comando de teste confirmar o runtime', () => {
    const github = catalog.find(({ name }) => name === 'GitHub');
    if (!github) throw new Error('Serviço GitHub ausente do catálogo.');

    expect(getPublicMcpStatus(github, [])).toBe('available');
    expect(
      getPublicMcpStatus(github, [{ endpoint: github.endpoint, enabled: false, status: 'Parado' }]),
    ).toBe('configured');
    expect(
      getPublicMcpStatus(github, [{ endpoint: github.endpoint, enabled: true, status: 'Erro' }]),
    ).toBe('configured');
    expect(
      getPublicMcpStatus(github, [
        { endpoint: github.endpoint, enabled: true, status: 'Conectado' },
      ]),
    ).toBe('connected');
  });
});
