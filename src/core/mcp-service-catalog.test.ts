import { describe, expect, it } from 'vitest';
import { canConnectWithOAuth, getPublicMcpStatus, oauthClientIdHelp } from './mcp-service-catalog';
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

  it('mantém o endpoint oficial fixo do GitHub e recomenda permissões de PAT de menor privilégio', () => {
    const github = catalog.find(({ name }) => name === 'GitHub');
    if (!github) throw new Error('Serviço GitHub ausente do catálogo.');

    expect(github.endpoint).toBe('https://api.githubcopilot.com/mcp/');
    expect(github.transport).toBe('HTTP com token Bearer (PAT)');
    expect(github.documentationUrl).toContain('docs.github.com/');
    expect(github.tokenCreationUrl).toBe('https://github.com/settings/personal-access-tokens/new');
    expect(github.tokenPermissions).toEqual(
      expect.arrayContaining([
        expect.stringContaining('Metadata: Read'),
        expect.stringContaining('Contents: Read'),
        expect.stringContaining('Issues: Read'),
        expect.stringContaining('Pull requests: Read'),
      ]),
    );
  });

  it('documenta o registro OAuth dinâmico do Notion e orienta o fluxo', () => {
    const notion = catalog.find(({ name }) => name === 'Notion');
    expect(notion?.authentication).toContain('registro dinâmico de cliente');
    expect(notion?.oauthRegistrationNote).toContain('identificador é recebido durante a conexão');
    expect(notion?.oauthSteps).toHaveLength(4);
    expect(notion?.oauthSteps?.[1]).toContain('não confirma essa alternativa');
    expect(notion?.documentationUrl).toBe(
      'https://developers.notion.com/guides/mcp/build-mcp-client',
    );
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

  it('não exige client ID quando há registro dinâmico e só libera conexão sem isso quando não há', () => {
    const notion = catalog.find(({ name }) => name === 'Notion');
    if (!notion) throw new Error('Serviço Notion ausente do catálogo.');

    expect(canConnectWithOAuth({ dynamicRegistration: true }, '')).toBe(true);
    expect(oauthClientIdHelp(notion, { dynamicRegistration: true })).toBeNull();
    expect(canConnectWithOAuth({ dynamicRegistration: false }, '')).toBe(false);
    expect(oauthClientIdHelp(notion, { dynamicRegistration: false })).toContain(
      'Este servidor exige um identificador de cliente. Veja a documentação do serviço',
    );
    expect(oauthClientIdHelp(notion, { dynamicRegistration: false })).toContain(
      notion.documentationUrl,
    );
    expect(canConnectWithOAuth({ dynamicRegistration: false }, 'manual-client')).toBe(true);
  });

  it('mantém Notion como configurado até o OAuth e o teste MCP concluírem', () => {
    const notion = catalog.find(({ name }) => name === 'Notion');
    if (!notion) throw new Error('Serviço Notion ausente do catálogo.');

    expect(notion.endpoint).toBe('https://mcp.notion.com/mcp');
    expect(getPublicMcpStatus(notion, [])).toBe('available');
    expect(
      getPublicMcpStatus(notion, [
        { name: 'Notion', endpoint: notion.endpoint, enabled: false, status: 'Parado' },
      ]),
    ).toBe('configured');
    expect(
      getPublicMcpStatus(notion, [
        { name: 'Notion', endpoint: notion.endpoint, enabled: true, status: 'Conectado' },
      ]),
    ).toBe('connected');
    expect(notion.oauthRegistrationNote).toContain('o identificador é recebido durante a conexão');
  });
});
