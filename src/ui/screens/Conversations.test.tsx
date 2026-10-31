import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  ConversationsScreen,
  MicrophoneActivationDialogContent,
  shouldFollowChatScroll,
  shouldSubmitChatOnEnter,
} from './Conversations';
import { ModalFrame } from '../Modal';

describe('ConversationsScreen', () => {
  it('renderiza o diálogo acessível de ativação com informação e confirmação explícita', () => {
    const markup = renderToStaticMarkup(
      <ModalFrame
        title="Ativar microfone?"
        titleId="microphone-title"
        close={() => undefined}
        closeOnBackdrop={false}
        dialogRef={() => undefined}
      >
        <MicrophoneActivationDialogContent
          busy={false}
          onCancel={() => undefined}
          onActivate={() => undefined}
        />
      </ModalFrame>,
    );

    expect(markup).toContain('role="dialog"');
    expect(markup).toContain('aria-labelledby="microphone-title"');
    expect(markup).toContain('Ativar microfone?');
    expect(markup).toContain('apenas na memória do aplicativo');
    expect(markup).toContain('Nada será enviado sem você confirmar');
    expect(markup).toContain('>Ativar</button>');
  });

  it('mantém a rolagem no fim apenas quando o usuário já está perto do fim', () => {
    expect(shouldFollowChatScroll(1000, 850, 100)).toBe(true);
    expect(shouldFollowChatScroll(1000, 500, 100)).toBe(false);
  });

  it('preserva Enter para enviar e Shift+Enter para inserir quebra de linha', () => {
    expect(shouldSubmitChatOnEnter('Enter', false)).toBe(true);
    expect(shouldSubmitChatOnEnter('Enter', true)).toBe(false);
    expect(shouldSubmitChatOnEnter('Escape', false)).toBe(false);
  });

  it('monta a conversa e identifica mensagens com o nome salvo no perfil', () => {
    const markup = renderToStaticMarkup(
      <ConversationsScreen
        messages={[
          { role: 'user', content: 'Olá', created_at: 1 },
          { role: 'assistant', content: 'Resposta de teste', created_at: 2 },
        ]}
        conversations={[]}
        conversationId={null}
        onConversation={() => undefined}
        onNewConversation={() => undefined}
        onDeleteConversation={() => undefined}
        onRenameConversation={() => undefined}
        busy={false}
        draft=""
        setDraft={() => undefined}
        error="A pasta do OdeKrai está ausente ou incompleta."
        errorDetails="Verificação local da pasta pessoal falhou."
        canFixConfiguration
        submit={(event) => event.preventDefault()}
        cancel={() => undefined}
        providerModel=""
        providerId="openai"
        microphoneEnabled={false}
        onMicrophonePreferenceChange={() => undefined}
        readRepliesAutomatically={false}
        speechVoiceURI=""
        speechRate={1}
        speechVolume={1}
        providerConfigured={false}
        conversationSidebarCollapsed={false}
        conversationSidebarWidth={230}
        onToggleConversationSidebar={() => undefined}
        onResizeConversationSidebar={() => undefined}
        onOpenLink={() => undefined}
        assistantUsesMascot
        onFixConfiguration={() => undefined}
        onRetry={() => undefined}
        canRetry={false}
        profile={{ displayName: 'Oli', nickname: '', avatarDataUrl: null }}
        selectedPet="character-female-a"
        agentEnabled={false}
        agentUnavailableReason="Configure a pasta de trabalho do agente."
        agentWorkspacePath={null}
        agentRunning={false}
        agentAwaitingApproval={false}
        agentActionCards={[]}
        onReviewApproval={() => undefined}
        onUndo={() => undefined}
        onOpenArtifact={() => undefined}
        onRevealArtifact={() => undefined}
        onToggleAgent={() => undefined}
        onStopAgent={() => undefined}
        onConfigureAgent={() => undefined}
      />,
    );
    expect(markup).toContain('Nova conversa');
    expect(markup).toContain('chat-header-main');
    expect(markup).toContain('class="messages-area"');
    expect(markup).toContain('class="composer"');
    expect(markup).toContain('class="chat-composer-actions"');
    expect(markup).not.toContain('Capturar tela');
    expect(markup).toContain('aria-label="Gravar áudio"');
    expect(markup).not.toContain('disabled="" aria-label="Gravar áudio"');
    expect(markup).toContain('svg');
    expect(markup).toContain('Lista de conversas');
    expect(markup).toContain('empty-state-pet');
    expect(markup).toContain('Avatar do OdeKrai: Nina');
    expect(markup).toContain('Recolher lista de conversas');
    expect(markup).toContain('Redimensionar lista de conversas');
    expect(markup).not.toContain('chat-model-chip');
    expect(markup).toContain('message-role">Oli</span>');
    expect(markup).toContain('>Ouvir</button>');
    expect(markup).toContain('Detalhes técnicos');
    expect(markup).toContain('class="chat-error-message"');
    expect(markup).toContain('<pre>Verificação local da pasta pessoal falhou.</pre>');
    expect(markup).toContain('Resolver pasta');
  });

  it('mostra o modelo no cabeçalho apenas quando difere do modelo atual', () => {
    const markup = renderToStaticMarkup(
      <ConversationsScreen
        messages={[]}
        conversations={[
          {
            id: 'one',
            title: 'Conversa anterior',
            preview: '',
            created_at: 1,
            provider_id: 'anthropic',
            model: 'claude-test',
          },
        ]}
        conversationId="one"
        onConversation={() => undefined}
        onNewConversation={() => undefined}
        onDeleteConversation={() => undefined}
        onRenameConversation={() => undefined}
        busy={false}
        draft=""
        setDraft={() => undefined}
        error=""
        errorDetails=""
        canFixConfiguration={false}
        submit={(event) => event.preventDefault()}
        cancel={() => undefined}
        providerModel="gpt-test"
        providerId="openai"
        microphoneEnabled={false}
        onMicrophonePreferenceChange={() => undefined}
        readRepliesAutomatically={false}
        speechVoiceURI=""
        speechRate={1}
        speechVolume={1}
        providerConfigured
        conversationSidebarCollapsed={false}
        conversationSidebarWidth={230}
        onToggleConversationSidebar={() => undefined}
        onResizeConversationSidebar={() => undefined}
        onOpenLink={() => undefined}
        assistantUsesMascot={false}
        onFixConfiguration={() => undefined}
        onRetry={() => undefined}
        canRetry={false}
        profile={{ displayName: 'Teste', nickname: '', avatarDataUrl: null }}
        selectedPet="character-female-a"
        agentEnabled={true}
        agentUnavailableReason={null}
        agentWorkspacePath="D:\\OdeKrai"
        agentRunning={false}
        agentAwaitingApproval={false}
        agentActionCards={[]}
        onReviewApproval={() => undefined}
        onUndo={() => undefined}
        onOpenArtifact={() => undefined}
        onRevealArtifact={() => undefined}
        onToggleAgent={() => undefined}
        onStopAgent={() => undefined}
        onConfigureAgent={() => undefined}
      />,
    );
    expect(markup).toContain('Modelo usado nesta conversa');
    expect(markup).toContain('Anthropic · claude-test');
  });

  it('renderiza o painel de conversas recolhido com controle para expandir', () => {
    const markup = renderToStaticMarkup(
      <ConversationsScreen
        messages={[]}
        conversations={[]}
        conversationId={null}
        onConversation={() => undefined}
        onNewConversation={() => undefined}
        onDeleteConversation={() => undefined}
        onRenameConversation={() => undefined}
        busy={false}
        draft=""
        setDraft={() => undefined}
        error=""
        errorDetails=""
        canFixConfiguration={false}
        submit={(event) => event.preventDefault()}
        cancel={() => undefined}
        providerModel=""
        providerId="openai"
        microphoneEnabled={false}
        onMicrophonePreferenceChange={() => undefined}
        readRepliesAutomatically={false}
        speechVoiceURI=""
        speechRate={1}
        speechVolume={1}
        providerConfigured={false}
        conversationSidebarCollapsed
        conversationSidebarWidth={230}
        onToggleConversationSidebar={() => undefined}
        onResizeConversationSidebar={() => undefined}
        onOpenLink={() => undefined}
        assistantUsesMascot={false}
        onFixConfiguration={() => undefined}
        onRetry={() => undefined}
        canRetry={false}
        profile={{ displayName: 'Teste', nickname: '', avatarDataUrl: null }}
        selectedPet="character-female-a"
        agentEnabled={false}
        agentUnavailableReason="Configure a pasta de trabalho do agente."
        agentWorkspacePath={null}
        agentRunning={false}
        agentAwaitingApproval={false}
        agentActionCards={[]}
        onReviewApproval={() => undefined}
        onUndo={() => undefined}
        onOpenArtifact={() => undefined}
        onRevealArtifact={() => undefined}
        onToggleAgent={() => undefined}
        onStopAgent={() => undefined}
        onConfigureAgent={() => undefined}
      />,
    );
    expect(markup).toContain('conversation-sidebar-collapsed');
    expect(markup).toContain('Expandir lista de conversas');
  });
});
