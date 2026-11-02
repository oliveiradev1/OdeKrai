/** @vitest-environment jsdom */
import type { ComponentProps } from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultSettings } from '../core/settings';
import { SETTINGS_MENU } from '../core/settings-navigation';
import { Overview } from './screens/Overview';
import { ConversationsScreen } from './screens/Conversations';
import { SkillsCatalogView, SkillsScreen } from './SkillsScreen';
import { ProvidersScreen } from './screens/Providers';
import { AssistantScreen } from './screens/Assistant';
import { ComputerScreen } from './screens/Computer';
import { SettingsModal } from './SettingsModal';
import { PermissionsScreen } from './screens/Permissions';
import { ActivityScreen } from './screens/Activity';
import publicCatalogFixture from '../../src-tauri/resources/public-mcp-catalog.json';

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  listen: vi.fn(async () => () => undefined),
  emit: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke, isTauri: tauri.isTauri }));
vi.mock('@tauri-apps/api/event', () => ({ listen: tauri.listen, emit: tauri.emit }));
vi.mock('@tauri-apps/api/window', () => {
  const current = {
    label: 'main',
    isVisible: async () => true,
    isMinimized: async () => false,
    isFocused: async () => true,
    onFocusChanged: async () => () => undefined,
  };
  return { getCurrentWindow: () => current, getAllWindows: async () => [current] };
});
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(async () => null) }));

const noop = () => undefined;
const profile = { displayName: 'Pessoa', nickname: '', avatarDataUrl: null };

function overviewProps(overrides: Partial<ComponentProps<typeof Overview>> = {}) {
  return {
    draft: '',
    setDraft: noop,
    busy: false,
    keyReady: true,
    profile,
    selectedPet: 'character-female-a',
    petState: 'idle' as const,
    reduceMotion: false,
    reduceMascot: false,
    suggestions: [],
    conversations: [],
    checklist: [],
    pendingApprovalCount: 0,
    onStartConversation: noop,
    onSendMessage: async () => true,
    onResumeConversation: noop,
    onResolveChecklistItem: noop,
    onOpenApprovals: noop,
    ...overrides,
  };
}

function conversationsProps(overrides: Partial<ComponentProps<typeof ConversationsScreen>> = {}) {
  return {
    messages: [],
    conversations: [],
    conversationId: null,
    onConversation: noop,
    onNewConversation: noop,
    onDeleteConversation: noop,
    onRenameConversation: noop,
    busy: false,
    draft: '',
    setDraft: noop,
    error: '',
    errorDetails: '',
    canFixConfiguration: false,
    submit: (event: React.FormEvent) => event.preventDefault(),
    cancel: noop,
    providerModel: '',
    providerId: 'openai',
    providerConfigured: false,
    conversationSidebarCollapsed: false,
    conversationSidebarWidth: 230,
    onToggleConversationSidebar: noop,
    onResizeConversationSidebar: noop,
    onOpenLink: noop,
    assistantUsesMascot: true,
    onFixConfiguration: noop,
    onRetry: noop,
    canRetry: false,
    profile,
    selectedPet: 'character-female-a',
    agentEnabled: false,
    agentUnavailableReason: 'Configure a pasta de trabalho do agente.',
    agentWorkspacePath: null,
    agentRunning: false,
    agentAwaitingApproval: false,
    agentActionCards: [],
    onReviewApproval: noop,
    onUndo: noop,
    onOpenArtifact: noop,
    onRevealArtifact: noop,
    onToggleAgent: noop,
    onStopAgent: noop,
    onConfigureAgent: noop,
    microphoneEnabled: false,
    onMicrophonePreferenceChange: noop,
    readRepliesAutomatically: false,
    speechVoiceURI: '',
    speechRate: 1,
    speechVolume: 1,
    ...overrides,
  };
}

function settingsModalProps(section: ComponentProps<typeof SettingsModal>['section']) {
  return {
    open: true,
    section,
    onClose: noop,
    onSelectSection: noop,
    settings: defaultSettings,
    update: noop,
    performanceStatus: { enabled: false, reason: 'Desativado no teste.', loading: false },
    profile,
    setProfile: noop,
    petVisible: false,
    setPetVisible: noop,
    selectedPet: 'character-female-a',
    onSelectPet: noop,
    onError: noop,
    onDeleteAllHistory: async () => true,
    onOpenPage: noop,
    onOpenActivityFullPage: noop,
    onAgentStatus: noop,
    releaseNotesUrl: 'https://example.test/notes',
    onOpenReleaseNotes: noop,
    onUpdateFound: noop,
    sharedAudit: { entries: [], verified: true, loading: false },
  };
}

beforeEach(() => {
  tauri.isTauri.mockReturnValue(false);
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation(async () => undefined);
  tauri.listen.mockClear();
  localStorage.clear();
  window.matchMedia = vi.fn().mockReturnValue({
    matches: false,
    media: '',
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(() => false),
  });
});
afterEach(() => cleanup());

describe('renderização das telas principais', () => {
  it('Início monta, mostra o título e o vazio de conversas; erro de habilidades é legível', () => {
    const { rerender } = render(<Overview {...overviewProps()} />);
    expect(screen.getByRole('heading', { name: 'O que vamos fazer hoje?' })).toBeTruthy();
    expect(screen.getByText('As conversas recentes aparecerão aqui.')).toBeTruthy();
    rerender(
      <Overview {...overviewProps({ skillsLoadError: 'Não foi possível carregar sugestões.' })} />,
    );
    expect(screen.getByRole('alert').textContent).toContain('Não foi possível carregar sugestões.');
    expect(screen.getByRole('button', { name: 'Tentar de novo' })).toBeTruthy();
  });

  it('Conversas monta com lista vazia e mostra a falha do comando no cartão da tela', () => {
    const { rerender } = render(<ConversationsScreen {...conversationsProps()} />);
    expect(screen.getByRole('heading', { name: 'Nova conversa' })).toBeTruthy();
    expect(screen.getByText('Suas conversas aparecerão aqui.')).toBeTruthy();
    rerender(
      <ConversationsScreen
        {...conversationsProps({ error: 'Não foi possível ler as conversas.' })}
      />,
    );
    expect(screen.getByRole('alert').textContent).toContain('Não foi possível ler as conversas.');
  });

  it.each([
    ['Pessoal', 'pessoal', 'Nenhuma habilidade está registrada nesta versão.'],
    ['Público', 'publico', 'Servidores MCP públicos'],
  ] as const)(
    'Habilidades, aba %s, monta com registro vazio e erro legível',
    (_label, tab, emptyText) => {
      const props = {
        skills: [],
        initialTab: tab,
        onFamilyToggle: noop,
        onSkillToggle: noop,
        onOverrideToggle: noop,
      };
      const { rerender } = render(<SkillsCatalogView {...props} />);
      expect(screen.getByRole('heading', { name: 'Habilidades' })).toBeTruthy();
      expect(screen.getByText(new RegExp(emptyText))).toBeTruthy();
      rerender(
        <SkillsCatalogView {...props} loadError="Não foi possível carregar as habilidades." />,
      );
      expect(screen.getByRole('alert').textContent).toContain(
        'Não foi possível carregar as habilidades.',
      );
    },
  );

  it('Habilidades apresenta falha de get_skills e estado vazio a partir de comandos simulados', async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_skills') throw new Error('catálogo indisponível');
      return [];
    });
    const { unmount } = render(
      <SkillsScreen onOpenPrivacy={noop} onRemovePublicMcp={async () => null} />,
    );
    expect((await screen.findByRole('alert')).textContent).toContain(
      'Não foi possível carregar as habilidades.',
    );
    tauri.invoke.mockImplementation(async () => []);
    unmount();
    render(<SkillsScreen onOpenPrivacy={noop} onRemovePublicMcp={async () => null} />);
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(screen.getByText('Nenhuma habilidade está registrada nesta versão.')).toBeTruthy();
  });

  it('Habilidades lê os serviços públicos do comando de catálogo do backend', async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_skills') return [];
      if (command === 'mcp_public_catalog') return publicCatalogFixture;
      if (command === 'mcp_list_servers') return [];
      if (command === 'get_allowed_folders' || command === 'get_allowed_apps') return [];
      return null;
    });
    render(<SkillsScreen onOpenPrivacy={noop} onRemovePublicMcp={async () => null} />);
    fireEvent.click(await screen.findByRole('tab', { name: 'Público' }));
    expect(await screen.findByRole('heading', { name: 'GitHub' })).toBeTruthy();
    expect(tauri.invoke).toHaveBeenCalledWith('mcp_public_catalog');
  });

  it('Conectores de IA monta sem conexões e informa falha ao consultar credenciais', async () => {
    const props: ComponentProps<typeof ProvidersScreen> = {
      settings: { ...defaultSettings, activeProviderId: '' },
      selectedId: 'openai',
      onSelect: noop,
      onSetPrimary: noop,
      onToggleEnabled: noop,
      onSave: noop,
      onChat: noop,
      onCredentialRemoved: noop,
      onOpenComputer: noop,
    };
    const { unmount } = render(<ProvidersScreen {...props} />);
    expect(screen.getByRole('heading', { name: 'Conecte IAs ao OdeKrai' })).toBeTruthy();
    expect(screen.getByText('Nenhum provedor principal configurado')).toBeTruthy();
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('credential query failed'));
    unmount();
    render(<ProvidersScreen {...props} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Conectados' }));
    expect(
      await screen.findByText(
        'Não foi possível consultar as credenciais. A lista pode estar incompleta.',
      ),
    ).toBeTruthy();
  });

  it('Assistente monta e mostra a falha ao ler o modo salvo', async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('mode unavailable'));
    render(
      <AssistantScreen
        settings={defaultSettings}
        update={noop}
        petVisible={false}
        onPetVisibilityChange={noop}
        onError={noop}
        selectedPet="character-female-a"
        onSelectPet={noop}
      />,
    );
    expect(screen.getByText('Escolha seu mascote')).toBeTruthy();
    await waitFor(() => expect(tauri.invoke).toHaveBeenCalledWith('get_pet_mode'));
    expect(await screen.findByText('Não foi possível carregar o modo salvo do pet.')).toBeTruthy();
  });

  it('Meu computador monta com catálogo sem arquivos e mostra falha do hardware', async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('hardware unavailable'));
    vi.spyOn(window, 'dispatchEvent').mockImplementation(() => true);
    render(<ComputerScreen />);
    expect(screen.getByRole('heading', { name: 'Seu computador' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Escolher modelo' })).toBeTruthy();
    expect(
      await screen.findByText(/Não foi possível consultar o hardware deste computador/),
    ).toBeTruthy();
  });

  it('Meu computador trata pasta de modelos ainda inexistente como estado vazio sem erro global', async () => {
    const missingFolder = '/app-data/models-not-created-yet';
    localStorage.setItem('odekrai.modelFolder', missingFolder);
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_hardware_profile') {
        return {
          total_ram_bytes: 0,
          available_ram_bytes: 0,
          models_free_bytes: 0,
          cpu_cores: 0,
          avx: false,
          avx2: false,
          avx512: false,
          adapters: [],
          warnings: [],
        };
      }
      if (command === 'inspect_local_models') return [];
      return undefined;
    });
    const dispatch = vi.spyOn(window, 'dispatchEvent').mockImplementation(() => true);

    render(<ComputerScreen />);
    await waitFor(() =>
      expect(tauri.invoke).toHaveBeenCalledWith('inspect_local_models', {
        destination: missingFolder,
      }),
    );
    await waitFor(() =>
      expect(tauri.invoke).toHaveBeenCalledWith(
        'record_diagnostic_event',
        expect.objectContaining({
          command: 'inspect_local_models',
          event: 'startup_command_finish',
          outcome: 'ok',
        }),
      ),
    );
    expect(screen.queryByRole('alert')).toBeNull();
    expect(dispatch.mock.calls.some(([event]) => event instanceof ErrorEvent)).toBe(false);
  });

  it.each(SETTINGS_MENU.map(({ id, title }) => [id, title]))(
    'Configurações: seção %s monta na região %s sem lançar erro',
    async (section, title) => {
      const key = section as ComponentProps<typeof SettingsModal>['section'];
      render(<SettingsModal {...settingsModalProps(key)} />);
      expect(await screen.findByRole('region', { name: title })).toBeTruthy();
    },
  );

  it('Configurações mostra erro de limites legível quando o comando falha', async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('limit read failed'));
    render(<SettingsModal {...settingsModalProps('geral')} />);
    expect((await screen.findByRole('alert')).textContent).toContain(
      'Não foi possível carregar os limites do agente.',
    );
  });

  it('Permissões monta e expõe falha da leitura de política', async () => {
    const { unmount } = render(<PermissionsScreen />);
    expect(screen.getByRole('heading', { name: 'Permissões' })).toBeTruthy();
    expect(screen.getByText(/disponíveis no aplicativo nativo/)).toBeTruthy();
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('policy unavailable'));
    unmount();
    render(<PermissionsScreen />);
    expect(
      await screen.findByText(
        'Não foi possível ler as configurações de segurança. Configure a pasta do agente para usar o log de atividade.',
      ),
    ).toBeTruthy();
  });

  it('Atividade mostra o vazio real e o erro de leitura em português', async () => {
    const emptyAudit = { entries: [], verified: true, loading: false };
    const { unmount } = render(<ActivityScreen sharedAudit={emptyAudit} />);
    expect(screen.getByRole('heading', { name: 'Atividade' })).toBeTruthy();
    expect(screen.getByText('Nenhum evento de auditoria para mostrar.')).toBeTruthy();
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue(new Error('audit unavailable'));
    unmount();
    render(<ActivityScreen />);
    expect(
      await screen.findByText(/Não foi possível ler as configurações de segurança/),
    ).toBeTruthy();
  });
});
