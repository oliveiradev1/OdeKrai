import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../core/settings';
import { PrivacyConfigurationSummary, PrivacyConfigurations } from './PrivacyConfigurations';
import { ConfirmationProvider } from './ConfirmationProvider';
import { DiscordWebhooksSettings } from './DiscordWebhooksSettings';

describe('configurações de privacidade', () => {
  it('mantém os controles em Privacidade e o resumo de Habilidades somente para leitura', () => {
    const settingsMarkup = renderToStaticMarkup(
      <ConfirmationProvider>
        <PrivacyConfigurations settings={defaultSettings} update={() => undefined} />
      </ConfirmationProvider>,
    );
    const summaryMarkup = renderToStaticMarkup(
      <PrivacyConfigurationSummary onOpen={() => undefined} />,
    );
    expect(settingsMarkup).toContain('Pastas permitidas');
    expect(settingsMarkup).toContain('Aplicativos permitidos');
    expect(settingsMarkup).toContain('Servidores MCP');
    expect(summaryMarkup).toContain('Abrir Privacidade e permissões');
    expect(summaryMarkup).not.toContain('Escolher aplicativo');
    expect(summaryMarkup).not.toContain('Adicionar Documentos');
  });

  it('preserva os comandos de persistência da interface e seus registros Rust', () => {
    const ui = readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8');
    const skillsUi = readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8');
    const rustCommands = readFileSync(
      new URL('../../src-tauri/src/lib.rs', import.meta.url),
      'utf8',
    );
    const commands = [
      'add_allowed_folder',
      'remove_allowed_folder',
      'register_allowed_app',
      'remove_allowed_app',
      'mcp_list_servers',
      'mcp_add_server',
      'mcp_add_http_server',
      'mcp_set_secret',
      'mcp_set_http_token',
      'mcp_oauth_authorize',
      'mcp_set_enabled',
      'mcp_test_connection',
      'mcp_remove_server',
      'get_trusted_external_domains',
      'trust_external_domain',
      'forget_external_domain',
    ];
    const publicCatalogCommands = new Set([
      'mcp_list_servers',
      'mcp_add_http_server',
      'mcp_set_http_token',
      'mcp_oauth_authorize',
      'mcp_set_enabled',
      'mcp_test_connection',
    ]);
    for (const command of commands) {
      expect(ui, `interface: ${command}`).toContain(command);
      if (publicCatalogCommands.has(command)) {
        expect(skillsUi, `catálogo público reutiliza o fluxo MCP: ${command}`).toContain(command);
      } else {
        expect(skillsUi, `sem duplicação em Habilidades: ${command}`).not.toContain(command);
      }
      expect(rustCommands, `registro Rust: ${command}`).toContain(command);
    }
    for (const readCommand of ['get_allowed_folders', 'get_allowed_apps']) {
      expect(ui, `consulta compartilhada: ${readCommand}`).toContain(readCommand);
      expect(skillsUi, `estado de requisito sem renderizar listas: ${readCommand}`).toContain(
        readCommand,
      );
      expect(rustCommands, `registro Rust: ${readCommand}`).toContain(readCommand);
    }
    const discordUi = readFileSync(
      new URL('./DiscordWebhooksSettings.tsx', import.meta.url),
      'utf8',
    );
    const discordCommands = readFileSync(
      new URL('../core/discord-integration.ts', import.meta.url),
      'utf8',
    );
    expect(discordUi).toContain('discord_list');
    expect(rustCommands).toContain('discord_list');
    for (const command of ['discord_test_connection', 'discord_save', 'discord_remove']) {
      expect(discordCommands).toContain(command);
    }
  });

  it('mantém os segredos MCP em formulários de gravação com nomes e IDs próprios', () => {
    const ui = readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8');
    expect(ui).toContain('submitSecretForm(event, addServer');
    expect(ui).toContain('() => saveServerSecret(server, key)');
    expect(ui).toContain('id={`mcp-new-secret-${key}`}');
    expect(ui).toContain('name={`mcp-new-secret-${key}`}');
    expect(ui).toContain('id={`mcp-secret-${server.id}-${key}`}');
    expect(ui).toContain('name={`mcp-secret-${server.id}-${key}`}');
    expect(ui).toContain('mcp-remote-token-new');
    expect(ui).toContain('mcp-http-token-${server.id}');
    expect(ui).toContain('saveRemoteToken(server)');
    expect(ui).toContain('token não volta a ser exibido');
    expect(ui.match(/autoComplete="off"/g)).toHaveLength(4);
  });

  it('orienta não colocar credenciais em argumentos MCP e limpa o formulário quando detectadas', () => {
    const ui = readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8');
    const rust = readFileSync(
      new URL('../../src-tauri/src/mcp_registry.rs', import.meta.url),
      'utf8',
    );
    expect(ui).toContain('Não informe tokens, senhas ou chaves nos argumentos');
    expect(ui).toContain("setServerCommand('')");
    expect(ui).toContain("setServerArgs('')");
    expect(rust).toContain('contains_inline_credential(command, args)');
    expect(rust).toContain('redact_inline_credential_config(server)');
  });

  it('apresenta ferramentas MCP separadas por disponibilidade e trata anotações como sugestão', () => {
    const ui = readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8');
    expect(ui).toContain('Ferramentas encontradas');
    expect(ui).toContain('Ativar esta ferramenta MCP (desativada por padrão)');
    expect(ui).toContain('Indisponível: {tool.reason}');
    expect(ui).toContain('Sugestão do servidor:');
    expect(ui).toContain('não altera o risco nem a aprovação obrigatória');
    expect(ui).toContain('dados externos não');
    expect(ui).toContain('confiáveis.');
  });

  it('exibe o aviso de revogação sem desfazer a remoção local do servidor', () => {
    const ui = readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8');
    expect(ui).toContain("invoke<string | null>('mcp_remove_server'");
    expect(ui).toContain("setError(warning ?? '')");
  });

  it('submete a URL do Discord pelo formulário de gravação e testa sem recarregar', () => {
    const markup = renderToStaticMarkup(<DiscordWebhooksSettings />);
    const formStart = markup.indexOf('<form>');
    const formEnd = markup.indexOf('</form>', formStart);
    const formMarkup = markup.slice(formStart, formEnd);
    expect(formStart).toBeGreaterThanOrEqual(0);
    expect(formMarkup).toContain('id="discord-webhook-url"');
    expect(formMarkup).toContain('name="discord-webhook-url"');
    expect(formMarkup).toContain('type="password"');
    expect(formMarkup).toContain('autoComplete="off"');
    expect(formMarkup).toContain('Autorizar e salvar');
    expect(formMarkup).toContain('Testar conexão');
    const source = readFileSync(new URL('./DiscordWebhooksSettings.tsx', import.meta.url), 'utf8');
    expect(source).toContain('submitSecretForm(event, save');
  });
});
