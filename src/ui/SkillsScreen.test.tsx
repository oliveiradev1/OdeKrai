import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { CatalogSkill } from '../core/skills-catalog';
import { SkillsCatalogView } from './SkillsScreen';

const registryFixture: CatalogSkill[] = [
  {
    id: 'files.read',
    name: 'Ler arquivo',
    description: 'Lê texto em uma pasta permitida.',
    risk: 'leitura',
    resultTrust: 'local',
    enabled: true,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'installed',
    configurationRequired: false,
    family: 'arquivos',
    icon: 'folder',
    examples: ['Leia o relatório da pasta permitida.'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'files.write',
    name: 'Gravar arquivo',
    description: 'Grava texto com cópia anterior reversível.',
    risk: 'escrita_reversivel',
    resultTrust: 'local',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'available',
    configurationRequired: false,
    family: 'arquivos',
    icon: 'folder',
    examples: ['Grave um resumo em notas.txt.'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'theme.set',
    name: 'Alterar tema do Windows',
    description: 'Experimental: alterna entre tema claro e escuro.',
    risk: 'escrita_reversivel',
    resultTrust: 'local',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'available',
    configurationRequired: false,
    family: 'computador',
    icon: 'desktop',
    examples: ['Altere o tema do Windows para escuro.'],
    origin: { kind: 'builtin' },
  },
];

const noOperation = () => undefined;

const discordSkill: CatalogSkill = {
  id: 'discord.send',
  name: 'Enviar mensagem ao Discord',
  description: 'Envia mensagem por webhook, sempre após aprovação.',
  risk: 'comunicacao_externa',
  resultTrust: 'local',
  enabled: true,
  morePermissive: false,
  fixedBlocked: false,
  category: 'integration',
  status: 'installed',
  configurationRequired: false,
  discordState: 'active',
  family: 'mensagens',
  icon: 'message',
  examples: ['Envie uma mensagem aprovada.'],
  origin: { kind: 'builtin' },
};

describe('catálogo de habilidades', () => {
  it('exibe o estado Discord calculado no Rust sem mensagens contraditórias', () => {
    const render = (skill: CatalogSkill, onOpenPrivacy?: () => void) =>
      renderToStaticMarkup(
        <SkillsCatalogView
          skills={[skill]}
          initialFamily="mensagens"
          onOpenPrivacy={onOpenPrivacy}
          onFamilyToggle={noOperation}
          onSkillToggle={noOperation}
          onOverrideToggle={noOperation}
        />,
      );

    const activeMarkup = render(discordSkill);
    expect(activeMarkup).toContain('>Ativa</span>');
    expect(activeMarkup).toContain(
      'Ativa e oferecida ao agente em todas as conversas no modo agente',
    );
    expect(activeMarkup).toContain('Cada envio exige aprovação.');
    expect(activeMarkup).not.toContain('Bloqueio fixo');
    expect(activeMarkup).not.toContain('ferramenta disponível para esta conversa');

    const unconfiguredMarkup = render(
      {
        ...discordSkill,
        enabled: false,
        status: 'configuration_required',
        configurationRequired: true,
        discordState: 'not_configured',
      },
      noOperation,
    );
    expect(unconfiguredMarkup).toContain('Cadastre um webhook para liberar esta habilidade.');
    expect(unconfiguredMarkup).toContain('Não configurada');
    expect(unconfiguredMarkup).toContain('Cadastrar webhook');
    expect(unconfiguredMarkup).toContain('aria-checked="false"');
    expect(unconfiguredMarkup).not.toContain('Bloqueio fixo');

    const blockedMarkup = render({
      ...discordSkill,
      enabled: false,
      status: 'blocked',
      discordState: 'blocked',
    });
    expect(blockedMarkup).toContain('>Bloqueada</span>');
    expect(blockedMarkup).toContain('Bloqueada porque a habilidade está desativada.');
    expect(blockedMarkup).not.toContain('Bloqueio fixo');
  });

  it('não mantém estilos dos cartões individuais substituídos pelo catálogo por família', () => {
    const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    const legacySelectors = [
      '.skill-card',
      '.skill-title-row',
      '.skill-status',
      '.skills-catalog-group',
      '.skills-family-details',
    ];

    for (const selector of legacySelectors) {
      const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      expect(styles).not.toMatch(new RegExp(`${escaped}(?=[\\s:{>+~])`));
    }
  });

  it('mantém a chave opcional do Google dentro do formulário e limpa após salvar', () => {
    const source = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    expect(source).toContain('submitSecretForm(event, saveGoogleCredentials');
    expect(source).toContain('id="google-client-secret"');
    expect(source).toContain('name="google-client-secret"');
    expect(source).toContain('autoComplete="off"');
    expect(source).toContain("() => setClientSecret('')");
  });

  it('exibe somente as famílias e habilidades recebidas do registro simulado', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        onOpenPrivacy={noOperation}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(markup).toContain('Habilidades');
    expect(markup).toContain('Procurar habilidades');
    expect(markup).toContain('Computador');
    expect(markup).toContain('Computador');
    expect(markup).not.toContain('Integrações MCP');
    expect(markup).not.toContain('Conexão com o Google');
    expect(markup).not.toContain('Captura de tela: indisponível');
    expect(markup).toContain('Abrir Privacidade e permissões');
    expect(markup.match(/class="skills-family-card"/g)).toHaveLength(2);
    expect(markup).toContain('3 habilidades encontradas');
    expect(markup).toContain('1 família ativa');
  });

  it('mostra erro e ação de nova tentativa quando o carregamento falha, sem exibir catálogo vazio', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[]}
        loadError="Não foi possível carregar as habilidades. Banco indisponível."
        onRetry={noOperation}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain('Banco indisponível.');
    expect(markup).toContain('Tentar de novo');
    expect(markup).not.toContain('Nenhuma habilidade está registrada nesta versão.');
    expect(markup).not.toContain('famílias ativas');
  });

  it('mostra busca por teclado, status, risco e um interruptor por família', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(markup).toContain('type="search"');
    expect(markup).toContain('placeholder="Nome, descrição, exemplo ou família"');
    expect(markup).toContain('aria-live="polite"');
    expect(markup).toContain('aria-label="Ativar todas as habilidades de Arquivos"');
    expect(markup).toContain('aria-label="Ativar todas as habilidades de Computador"');
    expect(markup).toContain('Risco: Escrita reversível');
    expect(markup.match(/role="switch"/g)).toHaveLength(2);
    expect(markup).toContain('aria-label="Famílias ativas"');
    expect(markup).toContain('role="tablist"');
    expect(markup).toContain('role="tabpanel"');
  });

  it('adapta os cartões a janelas estreitas e mantém o foco visível', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr))');
    expect(css).toMatch(
      /\.skills-family-card-heading\s*\{[^}]*grid-template-columns:\s*40px minmax\(0, 1fr\)/,
    );
    expect(css).toMatch(/\.skills-family-card-heading \.risk-badge\s*\{[^}]*grid-column:\s*2/);
    expect(css).toMatch(/\.skills-family-copy h2\s*\{[^}]*word-break:\s*normal/);
    expect(css).toMatch(
      /@media \(max-width: 760px\)[\s\S]*?\.skills-family-grid\s*\{\s*grid-template-columns: minmax\(0, 1fr\)/,
    );
    expect(css).toContain('.skills-catalog-page :is(button, input):focus-visible');
  });

  it('mantém nomes, descrições, etiquetas de risco e categorias legíveis no catálogo e em Gerenciar', () => {
    const longName = `${'Habilidade com nome extenso '.repeat(5)}identificadorsemseparadores`;
    const longDescription = 'Descrição detalhada da habilidade. '.repeat(20);
    const skills: CatalogSkill[] = [
      ...registryFixture,
      discordSkill,
      {
        ...discordSkill,
        id: 'mcp.long-description',
        name: longName,
        description: longDescription,
        risk: 'destrutivo_financeiro',
        category: 'integration',
        family: 'integracoes_mcp',
        origin: { kind: 'mcp', server: 'servidor-de-integracao-com-nome-extenso' },
        examples: [`${'Exemplo de solicitação longo '.repeat(8)}sem-separadores`],
      },
    ];
    const catalog = renderToStaticMarkup(
      <SkillsCatalogView
        skills={skills}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    const details = renderToStaticMarkup(
      <SkillsCatalogView
        skills={skills}
        initialFamily="integracoes_mcp"
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

    expect(catalog.match(/class="skills-family-card"/g)).toHaveLength(4);
    expect(catalog).toContain('Risco: Destrutivo ou financeiro');
    expect(catalog).toContain('Integrações MCP');
    expect(details).toContain(longName);
    expect(details).toContain(longDescription);
    expect(details).toContain('Exemplo de solicitação longo');
    expect(details).toContain('Risco: Destrutivo ou financeiro');
    expect(css).toMatch(/\.skills-detail-title h4\s*\{[^}]*overflow-wrap:\s*anywhere/);
    expect(css).toMatch(
      /\.skills-detail-copy > p,[\s\S]*?overflow-wrap:\s*break-word;\s*word-break:\s*normal/,
    );
  });

  it('mantém estados de carregamento e erro acessíveis', () => {
    const loading = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[]}
        loading
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    const failed = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[]}
        error="Falha ao carregar habilidades."
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(loading).toContain('role="status"');
    expect(loading).toContain('Carregando habilidades');
    expect(failed).toContain('role="alert"');
    expect(failed).toContain('Falha ao carregar habilidades.');
  });

  it('apresenta os servidores MCP públicos e os estados reais do catálogo', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        initialTab="publico"
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
        onConfigurePublicMcp={async () => true}
      />,
    );
    expect(markup).toContain('Pessoal');
    expect(markup).toContain('Público');
    for (const service of [
      'GitHub',
      'Figma',
      'Gmail',
      'Google Calendar',
      'Google Sheets',
      'Lovable',
      'Notion',
    ]) {
      expect(markup).toContain(`<h3>${service}</h3>`);
    }
    expect(markup).toContain('Disponível para configurar');
    expect(markup).toContain('Não disponível');
    expect(markup.match(/Developer Preview/g)).toHaveLength(3);
    expect(markup).not.toContain('>Conectado</span>');
    expect(markup).toContain('Configurar');
    expect(markup).toMatch(/id="skills-panel-personal"[^>]*hidden=""/);
    expect(markup).not.toMatch(/id="skills-panel-public"[^>]*hidden=""/);
    expect(markup).not.toContain('Instalar pacote');
  });

  it('mostra o gerenciamento do PAT GitHub sem exibir o valor salvo', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        initialTab="publico"
        mcpServers={[
          {
            id: 'server-github',
            name: 'GitHub',
            endpoint: 'https://api.githubcopilot.com/mcp/',
            enabled: true,
            status: 'Conectado',
          },
        ]}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
        onSaveGitHubToken={async () => true}
        onRemoveGitHubToken={async () => true}
      />,
    );
    expect(markup).toContain('Substituir token pessoal do GitHub');
    expect(markup).toContain('type="password"');
    expect(markup).toContain('autoComplete="off"');
    expect(markup).toContain('Salvar token');
    expect(markup).toContain('Remover token');
    expect(markup).toContain('Criar token pessoal no GitHub');
    expect(markup).toContain('https://api.githubcopilot.com/mcp/');
    expect(markup).toContain('Metadata: Read');
    expect(markup).not.toContain('github-pat-fixture');

    const source = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    expect(source).toContain("invoke('mcp_set_http_token', { serverId: server.id, value: token })");
    expect(source).toContain("invoke('mcp_remove_http_token', { serverId: server.id })");
    expect(source).toContain("setToken('')");
    expect(source).toContain('event.preventDefault()');
    expect(source).toContain("readOnly={service.name === 'GitHub'}");
    expect(source).toContain('endpoint !== service.endpoint');
  });

  it('lê o cliente OAuth público do Rust e guarda segredo apenas no Credential Manager', () => {
    const source = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    expect(source).toContain("invoke<OAuthRegistrationOptions>('mcp_oauth_registration_options'");
    expect(source).toContain(
      "invoke<PublicOAuthClientSettings>('mcp_oauth_public_client_settings'",
    );
    expect(source).toContain("invoke('mcp_oauth_save_public_client_secret'");
    expect(source).toContain("invoke('mcp_oauth_remove_public_client_secret'");
    expect(source).toContain('Este serviço ainda não foi configurado para o OdeKrai. Veja o guia');
    expect(source).toContain(
      '!oauthClientSettings.clientId.trim() && !oauthClientSettings.dynamicRegistration',
    );
    expect(source).toMatch(
      /<Button type="button" variant="secondary" disabled>\s*Conectar\s*<\/Button>/,
    );
    expect(source).toContain('Segredo do cliente OAuth');
    expect(source).toContain('autoComplete="off"');
    expect(source).toContain("setClientSecret('')");
    expect(source).toContain('Escopos OAuth (opcional)');
    expect(source).toContain('canConnectWithOAuth(registrationOptions, clientId, busy)');
    expect(source).not.toContain('clientSecretSaved && clientSecret');
  });

  it('não confunde servidor salvo com conexão MCP aprovada pelo teste', () => {
    const props = {
      skills: registryFixture,
      initialTab: 'publico' as const,
      onFamilyToggle: noOperation,
      onSkillToggle: noOperation,
      onOverrideToggle: noOperation,
    };
    const configuredMarkup = renderToStaticMarkup(
      <SkillsCatalogView
        {...props}
        mcpServers={[
          {
            id: 'server-github',
            name: 'GitHub',
            endpoint: 'https://api.githubcopilot.com/mcp/',
            enabled: false,
            status: 'Parado',
          },
        ]}
        onTestPublicMcp={async () => true}
      />,
    );
    expect(configuredMarkup).toContain('>Configurado</span>');
    expect(configuredMarkup).toContain('Testar conexão');

    const connectedMarkup = renderToStaticMarkup(
      <SkillsCatalogView
        {...props}
        mcpServers={[
          {
            id: 'server-github',
            name: 'GitHub',
            endpoint: 'https://api.githubcopilot.com/mcp/',
            enabled: true,
            status: 'Conectado',
          },
        ]}
      />,
    );
    expect(connectedMarkup).toContain('>Conectado</span>');
    expect(connectedMarkup).toContain('Teste MCP concluído.');
  });

  it('abre os detalhes, preserva exemplos e deixa overrides avançados recolhidos', () => {
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        initialFamily="arquivos"
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
        onOpenPrivacy={noOperation}
      />,
    );
    expect(markup).toContain('aria-label="Abrir detalhes de Arquivos"');
    expect(markup).toContain('Voltar às habilidades');
    expect(markup).toContain('Habilidades desta família');
    expect(markup).toContain('Categoria');
    expect(markup).toContain('Origem: OdeKrai');
    expect(markup).toContain('Leia o relatório da pasta permitida.');
    expect(markup).toContain('Desativar Ler arquivo');
    expect(markup).toContain('Risco: Leitura');
    expect(markup).toContain('Requisitos e estado');
    expect(markup).toContain('pasta de trabalho disponível');
    expect(markup).toContain('Abrir Privacidade e permissões');
    expect(markup).toMatch(/<details class="skill-advanced skills-override-settings">/);
    expect(markup).toContain('O OdeKrai solicitará confirmação nativa');
    expect(markup).toContain('Controle total não remove aprovações críticas');

    const experimentalMarkup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={registryFixture}
        initialFamily="computador"
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(experimentalMarkup).toContain('Experimental');
  });

  it('explica bloqueios fixos e exibe o requisito ausente de aplicativo sem duplicar a lista', () => {
    const payment: CatalogSkill = {
      ...registryFixture[0],
      id: 'simulated.payment',
      name: 'Pagamento bloqueado',
      risk: 'destrutivo_financeiro',
      status: 'unavailable',
      statusReason: 'Habilidade indisponível nesta versão.',
      fixedBlocked: true,
      enabled: false,
      examples: ['Não solicite pagamentos.'],
    };
    const blockedMarkup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[payment]}
        initialFamily="arquivos"
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(blockedMarkup).toContain('Bloqueio fixo: esta habilidade não pode ser liberada');
    expect(blockedMarkup).toContain('Motivo: Habilidade indisponível nesta versão.');
    expect(blockedMarkup).not.toContain('skills-override-settings');

    const appSkill: CatalogSkill = {
      ...registryFixture[0],
      id: 'app.launch',
      name: 'Iniciar aplicativo permitido',
      family: 'abrir_aplicativos',
      icon: 'app_window',
      examples: ['Inicie o aplicativo cadastrado.'],
    };
    const missingAppMarkup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[appSkill]}
        initialFamily="abrir_aplicativos"
        prerequisites={{ folderAvailable: true, appCount: 0, googleConnected: null }}
        onOpenPrivacy={noOperation}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(missingAppMarkup).toContain('nenhum aplicativo cadastrado');
    expect(missingAppMarkup).toContain('Cadastrar aplicativo');
    expect(missingAppMarkup).toContain('Inicie o aplicativo cadastrado.');
    expect(missingAppMarkup).not.toContain('allowed-folder-list');
  });

  it('mantém busca e aba no mesmo estado da tela enquanto o painel é usado e volta ao catálogo', () => {
    const source = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    const rustSecurity = readFileSync(
      new URL('../../src-tauri/src/security/mod.rs', import.meta.url),
      'utf8',
    );
    expect(source).toContain('const [query, setQuery] = useState');
    expect(source).toContain('const [tab, setTab] = useState');
    expect(source).toContain('setSelectedFamily(null)');
    expect(source).toContain("document.getElementById('skills-search')?.focus()");
    expect(source).not.toContain("setQuery('')");
    expect(source).toContain('setSkillOverrideCommand(skill.id, enabled, invoke)');
    expect(source).not.toContain('window.confirm(');
    expect(rustSecurity).toContain('!confirm_native(format!(');
    expect(rustSecurity).toContain(
      'Esta exceção nunca ignora bloqueios fixos nem conteúdo externo.',
    );
  });

  it('mantém a configuração Google apenas nos detalhes quando as habilidades Google vêm do registro', () => {
    const googleSkill: CatalogSkill = {
      ...registryFixture[0],
      id: 'google.calendar.list_events',
      name: 'Consultar Google Calendar',
      family: 'google',
      icon: 'calendar',
      examples: ['Consulte os eventos desta semana.'],
    };
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[googleSkill]}
        initialFamily="google"
        googleConnection={<section>Conexão com o Google</section>}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );
    expect(markup).toContain('Conexão com o Google');
    expect(markup).toContain('Consultar Google Calendar');
  });

  it('permite ativar a Agenda local quando o Google não está conectado', () => {
    const localCalendarSkill: CatalogSkill = {
      ...registryFixture[0],
      id: 'calendar.list_events',
      name: 'Consultar Agenda local',
      enabled: false,
      family: 'agenda_lembretes',
      icon: 'calendar',
    };
    const markup = renderToStaticMarkup(
      <SkillsCatalogView
        skills={[localCalendarSkill]}
        prerequisites={{ folderAvailable: true, appCount: 0, googleConnected: false }}
        onFamilyToggle={noOperation}
        onSkillToggle={noOperation}
        onOverrideToggle={noOperation}
      />,
    );

    expect(markup).toContain('aria-label="Ativar todas as habilidades de Agenda e lembretes"');
    expect(markup).not.toMatch(
      /<button[^>]*role="switch"[^>]*aria-label="Ativar todas as habilidades de Agenda e lembretes"[^>]*disabled/,
    );
  });

  it('mantém o catálogo baseado em get_skills e a ativação em comandos Rust', () => {
    const source = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    expect(source).toContain("invoke<CatalogSkill[]>('get_skills')");
    expect(source).toContain('setSkillGroupEnabledCommand(family, enabled, invoke)');
    expect(source).toContain('setSkillEnabledCommand(skill.id, enabled, invoke)');
    expect(source).toContain("invoke<Array<{ workspace: boolean }>>('get_allowed_folders')");
    expect(source).toContain("invoke<Array<{ id: string }>>('get_allowed_apps')");
    expect(source).toContain("invoke<GoogleOAuthStatus>('google_oauth_status')");
    expect(source).toContain("skill.id.startsWith('google.calendar.')");
    expect(source).toContain('PrivacyConfigurationSummary onOpen={onOpenPrivacy}');
  });
});
