import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import {
  canConnectToPublicOAuth,
  canConnectWithOAuth,
  getPublicMcpStatus,
  oauthClientIdHelp,
  publicOAuthServiceKey,
} from './mcp-service-catalog';
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

    expect(unavailable).toEqual(['Figma', 'Lovable']);
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

  it('documenta os passos e escopos mínimos independentes dos três serviços Google', () => {
    const googleServices = catalog.filter(({ supplier }) => supplier === 'Google Workspace');
    expect(googleServices).toHaveLength(3);
    for (const service of googleServices) {
      expect(service.status).toBe('available_to_configure');
      expect(service.programStatus).toBe('Developer Preview');
      expect(service.oauthSetupSteps?.join(' ')).toContain('Google Cloud');
      expect(service.oauthSetupSteps?.join(' ')).toContain('identificador e o segredo');
      expect(service.oauthScopeLabels?.length).toBeGreaterThan(0);
    }
    const gmail = googleServices.find(({ name }) => name === 'Gmail');
    expect(gmail?.oauthScopeLabels).toEqual([
      'Leitura de mensagens: https://www.googleapis.com/auth/gmail.readonly',
      'Criar rascunhos: https://www.googleapis.com/auth/gmail.compose (o OdeKrai não oferece ferramenta de envio)',
    ]);
    expect(gmail?.oauthSetupSteps?.join(' ')).toContain('Developer Preview Program');
    expect(gmail?.oauthSetupSteps?.join(' ')).toContain('Gmail MCP API');
    expect(gmail?.oauthSetupSteps?.join(' ')).toContain('127.0.0.1:43117/callback');
  });

  it('mantém o endpoint oficial fixo do GitHub e recomenda permissões de PAT de menor privilégio', () => {
    const github = catalog.find(({ name }) => name === 'GitHub');
    if (!github) throw new Error('Serviço GitHub ausente do catálogo.');

    expect(github.endpoint).toBe('https://api.githubcopilot.com/mcp/');
    expect(github.transport).toBe('Streamable HTTP');
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
      'Este serviço ainda não foi configurado para o OdeKrai. Veja o guia',
    );
    expect(oauthClientIdHelp(notion, { dynamicRegistration: false })).toContain(
      notion.documentationUrl,
    );
    expect(canConnectWithOAuth({ dynamicRegistration: false }, 'manual-client')).toBe(true);
    expect(oauthClientIdHelp(notion, { dynamicRegistration: false })).toContain(
      'Este serviço ainda não foi configurado para o OdeKrai. Veja o guia',
    );
  });

  it('mapeia os serviços públicos para as chaves do recurso OAuth e bloqueia IDs vazios', () => {
    const google = catalog.find(({ name }) => name === 'Google Calendar');
    const github = catalog.find(({ name }) => name === 'GitHub');
    const notion = catalog.find(({ name }) => name === 'Notion');
    if (!google || !github || !notion) throw new Error('Serviço esperado ausente do catálogo.');

    expect(publicOAuthServiceKey(google)).toBe('google');
    expect(publicOAuthServiceKey(github)).toBe('github');
    expect(publicOAuthServiceKey(notion)).toBe('notion');
    expect(canConnectWithOAuth({ dynamicRegistration: false }, '')).toBe(false);
    expect(canConnectWithOAuth({ dynamicRegistration: false }, 'public-client')).toBe(true);
    expect(canConnectWithOAuth({ dynamicRegistration: true }, '')).toBe(true);
    const blankStaticSettings = {
      service: 'google',
      clientId: '',
      guideUrl: 'https://developers.google.com/workspace/guides/configure-mcp-servers',
      dynamicRegistration: false,
      clientSecretRequired: true,
      clientSecretSaved: false,
    };
    expect(canConnectToPublicOAuth(blankStaticSettings, { dynamicRegistration: false })).toBe(
      false,
    );
    expect(
      canConnectToPublicOAuth(
        { ...blankStaticSettings, clientId: 'public-client' },
        { dynamicRegistration: false },
      ),
    ).toBe(false);
    expect(
      canConnectToPublicOAuth(
        { ...blankStaticSettings, clientId: 'public-client', clientSecretSaved: false },
        { dynamicRegistration: false },
      ),
    ).toBe(false);
    expect(
      canConnectToPublicOAuth(
        { ...blankStaticSettings, clientId: 'public-client', clientSecretSaved: true },
        { dynamicRegistration: false },
      ),
    ).toBe(true);
    expect(
      canConnectToPublicOAuth(
        {
          ...blankStaticSettings,
          service: 'github',
          clientId: 'github-public-client',
          clientSecretRequired: true,
          clientSecretSaved: false,
        },
        { dynamicRegistration: false },
      ),
    ).toBe(false);
    expect(
      canConnectToPublicOAuth(
        {
          ...blankStaticSettings,
          service: 'github',
          clientId: 'github-public-client',
          clientSecretRequired: true,
          clientSecretSaved: true,
        },
        { dynamicRegistration: false },
      ),
    ).toBe(true);
    expect(
      canConnectToPublicOAuth(
        { ...blankStaticSettings, dynamicRegistration: true, clientSecretRequired: false },
        { dynamicRegistration: true },
      ),
    ).toBe(true);
  });

  it('mantém IDs vazios e guias HTTPS em domínios explicitamente permitidos no recurso Rust', () => {
    const oauth = JSON.parse(
      readFileSync(
        new URL('../../src-tauri/resources/oauth-clients.json', import.meta.url),
        'utf8',
      ),
    ) as {
      allowedGuideDomains: string[];
      services: Record<
        string,
        {
          clientId: string;
          guideUrl: string;
          clientSecretRequired: boolean;
          clientSecret?: string;
          client_secret?: string;
        }
      >;
    };
    expect(Object.keys(oauth.services).sort()).toEqual([
      'figma',
      'github',
      'google',
      'lovable',
      'notion',
    ]);
    for (const service of Object.values(oauth.services)) {
      const guide = new URL(service.guideUrl);
      expect(service.clientId).toBe('');
      expect(guide.protocol).toBe('https:');
      expect(oauth.allowedGuideDomains).toContain(guide.hostname);
      expect(service).not.toHaveProperty('clientSecret');
      expect(service).not.toHaveProperty('client_secret');
    }
    expect(oauth.services.github.clientSecretRequired).toBe(true);
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
