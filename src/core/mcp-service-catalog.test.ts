import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import {
  canConnectToPublicOAuth,
  canConnectWithOAuth,
  getPublicMcpStatus,
  oauthClientIdHelp,
  publicOAuthServiceKey,
  type PublicMcpCatalogEntry,
} from './mcp-service-catalog';
import rawCatalog from '../../src-tauri/resources/public-mcp-catalog.json';

const catalog = rawCatalog as unknown as PublicMcpCatalogEntry[];

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
      expect(service.provider).toBeTruthy();
      expect(service.officialLink).toMatch(/^https:\/\//);
      expect(service.endpoint).toMatch(/^https:\/\//);
      expect(service.transport).toBeTruthy();
      expect(service.authentication).toBeTruthy();
      expect(service.authenticationMethods.length).toBeGreaterThan(0);
      expect(['available', 'in_preview', 'configuration_pending', 'not_verified']).toContain(
        service.status,
      );
      expect(service.id).toBeTruthy();
      expect(service.provider).toBeTruthy();
      expect(service.officialLink).toMatch(/^https:\/\//);
      expect(service.requirements).toBeDefined();
      expect(service.permissions).toBeDefined();
      expect(service.connectionType.length).toBeGreaterThan(0);
    }
  });

  it('não apresenta como disponível no OdeKrai serviços com requisito incompatível documentado', () => {
    const unavailable = catalog.filter(({ connectable }) => !connectable).map(({ name }) => name);

    expect(unavailable).toEqual(['Figma']);
  });

  it('identifica o Developer Preview do Google e usa fontes oficiais da Lovable', () => {
    expect(
      catalog
        .filter(({ provider }) => provider === 'Google Workspace')
        .map(({ programStatus }) => programStatus),
    ).toEqual(['Developer Preview', 'Developer Preview', 'Developer Preview']);

    const lovable = catalog.find(({ name }) => name === 'Lovable');
    expect(lovable?.endpoint).toBe('https://mcp.lovable.dev');
    expect(lovable?.officialLink).toBe('https://docs.lovable.dev/integrations/lovable-mcp-server');
    expect(lovable?.sourceCodeUrl).toBe('https://github.com/lovablelabs/mcp');
  });

  it('mantém Figma sem ações de conexão até o OdeKrai entrar no catálogo autorizado', () => {
    const figma = catalog.find(({ name }) => name === 'Figma');
    if (!figma) throw new Error('Serviço Figma ausente do catálogo.');

    expect(figma.endpoint).toBe('https://mcp.figma.com/mcp');
    expect(figma.authenticationMethods).toEqual(['oauth']);
    expect(figma.authentication).toContain('NÃO VERIFICADO');
    expect(figma.status).toBe('configuration_pending');
    expect(figma.requirements.join(' ')).toContain('lista de espera da Figma');
  });

  it('oferece Lovable somente via OAuth e confirma registro dinâmico sem client ID manual', () => {
    const lovable = catalog.find(({ name }) => name === 'Lovable');
    if (!lovable) throw new Error('Serviço Lovable ausente do catálogo.');

    expect(lovable.endpoint).toBe('https://mcp.lovable.dev');
    expect(lovable.authenticationMethods).toEqual(['oauth']);
    expect(lovable.status).toBe('available');
    expect(lovable.requirements.join(' ')).toContain('OdeKrai usa 127.0.0.1');
    expect(lovable.oauthRegistrationNote).toContain('registration_endpoint');
    expect(canConnectWithOAuth({ dynamicRegistration: true }, '')).toBe(true);
    expect(canConnectWithOAuth({ dynamicRegistration: false }, '')).toBe(false);
  });

  it('documenta os passos e escopos mínimos independentes dos três serviços Google', () => {
    const googleServices = catalog.filter(({ provider }) => provider === 'Google Workspace');
    expect(googleServices).toHaveLength(3);
    for (const service of googleServices) {
      expect(service.status).toBe('in_preview');
      expect(service.programStatus).toBe('Developer Preview');
      expect(service.requirements.join(' ')).toContain('Google Cloud');
      expect(service.requirements.join(' ')).toContain('identificador e o segredo');
      expect(service.permissions.length).toBeGreaterThan(0);
    }
    const gmail = googleServices.find(({ name }) => name === 'Gmail');
    expect(gmail?.permissions).toEqual([
      'Leitura de mensagens: https://www.googleapis.com/auth/gmail.readonly',
      'Criar rascunhos: https://www.googleapis.com/auth/gmail.compose (o OdeKrai não oferece ferramenta de envio)',
    ]);
    expect(gmail?.requirements.join(' ')).toContain('Developer Preview Program');
    expect(gmail?.requirements.join(' ')).toContain('Gmail MCP API');
    expect(gmail?.requirements.join(' ')).toContain('127.0.0.1:43117/callback');
  });

  it('mantém o endpoint oficial fixo do GitHub e recomenda permissões de PAT de menor privilégio', () => {
    const github = catalog.find(({ name }) => name === 'GitHub');
    if (!github) throw new Error('Serviço GitHub ausente do catálogo.');

    expect(github.endpoint).toBe('https://api.githubcopilot.com/mcp/');
    expect(github.transport).toBe('Streamable HTTP');
    expect(github.officialLink).toContain('docs.github.com/');
    expect(github.tokenCreationUrl).toBe('https://github.com/settings/personal-access-tokens/new');
    expect(github.personalTokenPermissions).toEqual(
      expect.arrayContaining([
        expect.stringContaining('Metadata: Read'),
        expect.stringContaining('Contents: Read'),
        expect.stringContaining('Issues: Read'),
        expect.stringContaining('Pull requests: Read'),
      ]),
    );
  });

  it('documenta somente OAuth com PKCE e registro dinâmico como ação do Notion', () => {
    const notion = catalog.find(({ name }) => name === 'Notion');
    expect(notion?.authentication).toContain('registro dinâmico de cliente');
    expect(notion?.authenticationMethods).toEqual(['oauth']);
    expect(notion?.oauthRegistrationNote).toContain('identificador é recebido durante a conexão');
    expect(notion?.requirements).toHaveLength(4);
    expect(notion?.requirements[1]).toContain('registration_endpoint');
    expect(notion?.requirements[1]).toContain('não é necessário configurar client ID');
    expect(notion?.officialLink).toBe('https://developers.notion.com/guides/mcp/build-mcp-client');
  });

  it('só marca conectado após o comando de teste confirmar o runtime', () => {
    const github = catalog.find(({ name }) => name === 'GitHub');
    if (!github) throw new Error('Serviço GitHub ausente do catálogo.');

    expect(getPublicMcpStatus(github, [])).toBe('configured');
    expect(
      getPublicMcpStatus(github, [{ endpoint: github.endpoint, enabled: false, status: 'Parado' }]),
    ).toBe('configured');
    expect(
      getPublicMcpStatus(github, [{ endpoint: github.endpoint, enabled: true, status: 'Erro' }]),
    ).toBe('error');
    expect(
      getPublicMcpStatus(github, [
        { endpoint: github.endpoint, enabled: true, status: 'Conectado' },
      ]),
    ).toBe('connected');
    expect(
      getPublicMcpStatus(github, [
        { endpoint: github.endpoint, enabled: true, status: 'Connecting' },
      ]),
    ).toBe('connecting');
    expect(
      getPublicMcpStatus(github, [{ endpoint: github.endpoint, enabled: true, status: 'Error' }]),
    ).toBe('error');
  });

  it('usa mensagens curtas para erros OAuth e nunca devolve a resposta bruta', async () => {
    const { publicMcpErrorMessage } = await import('./mcp-service-catalog');
    expect(publicMcpErrorMessage('HTTP 401: secret raw response')).toBe(
      'O serviço recusou a autorização',
    );
    expect(publicMcpErrorMessage('request timeout')).toBe('A conexão expirou, tente de novo');
    expect(publicMcpErrorMessage('insufficient_scope: private body')).toBe(
      'Permissão insuficiente',
    );
    expect(publicMcpErrorMessage('missing permission')).toBe('Permissão insuficiente');
    expect(publicMcpErrorMessage('unexpected <private response>')).toBe(
      'Não foi possível conectar. Confira a configuração e tente de novo.',
    );
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
      notion.officialLink,
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
    expect(notion.authenticationMethods).toEqual(['oauth']);
  });
});
