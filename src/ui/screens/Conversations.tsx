import { useEffect, useRef, useState, type CSSProperties, type FormEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ChatMessage } from '../../core/provider-messages';
import { filterConversations, presentMessages, type ConversationSummary } from '../../core/chat';
import { safeMarkdownUrl } from '../../core/markdown';
import { Button } from '../DesignSystem';
import { Icon } from '../Icon';
import { PetSprite } from '../PetSprite';
import { MessageMarkdown } from '../MessageMarkdown';
import appIcon from '../../assets/odekrai-app-icon.png';
import { initials, type ProfileData } from '../Profile';
import { ResizeHandle } from '../ResizeHandle';
import { providerById } from '../../core/providers';
import { AgentActionCard, AgentControls, type AgentActionCardData } from '../AgentControls';
import { supportsTranscription, transcribeConfirmedRecording } from '../../core/transcription';
import { getLocalSpeechReader, type SpeechState } from '../../core/speech';
import { MessageComposer, shouldSubmitMessageOnEnter } from '../MessageComposer';
import type { PetAnimationState } from '../../core/pets';
import {
  activateMicrophone,
  microphoneClickAction,
  microphonePermissionError,
} from '../../core/microphone';
import { Modal } from '../Modal';

export function MicrophoneActivationDialogContent({
  busy,
  onCancel,
  onActivate,
}: {
  busy: boolean;
  onCancel: () => void;
  onActivate: () => void;
}) {
  return (
    <div className="microphone-activation-dialog">
      <p>O áudio gravado fica apenas na memória do aplicativo.</p>
      <p>Nada será enviado sem você confirmar uma ação de transcrição.</p>
      <div className="dialog-actions">
        <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
          Agora não
        </Button>
        <Button type="button" variant="primary" onClick={onActivate} disabled={busy}>
          {busy ? 'Solicitando permissão…' : 'Ativar'}
        </Button>
      </div>
    </div>
  );
}

export function shouldFollowChatScroll(
  scrollHeight: number,
  scrollTop: number,
  clientHeight: number,
  threshold = 96,
) {
  return scrollHeight - scrollTop - clientHeight < threshold;
}

export const shouldSubmitChatOnEnter = shouldSubmitMessageOnEnter;

export function ConversationsScreen({
  messages,
  conversations,
  conversationId,
  onConversation,
  onNewConversation,
  onDeleteConversation,
  onRenameConversation,
  busy,
  draft,
  setDraft,
  error,
  errorDetails,
  canFixConfiguration,
  submit,
  cancel,
  providerModel,
  providerId,
  providerConfigured,
  conversationSidebarCollapsed,
  conversationSidebarWidth,
  onToggleConversationSidebar,
  onResizeConversationSidebar,
  onOpenLink,
  assistantUsesMascot,
  onFixConfiguration,
  onRetry,
  canRetry,
  profile,
  selectedPet,
  petState = 'idle',
  reduceMotion = false,
  reduceMascot = false,
  agentEnabled,
  agentUnavailableReason,
  agentWorkspacePath,
  agentRunning,
  agentAwaitingApproval,
  agentActionCards,
  onReviewApproval,
  onConfigureDiscord,
  onUndo,
  onOpenArtifact,
  onRevealArtifact,
  onToggleAgent,
  onStopAgent,
  onConfigureAgent,
  microphoneEnabled,
  onMicrophonePreferenceChange,
  readRepliesAutomatically,
  speechVoiceURI,
  speechRate,
  speechVolume,
}: {
  messages: ChatMessage[];
  conversations: ConversationSummary[];
  conversationId: string | null;
  onConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
  onRenameConversation: (id: string, title: string) => void;
  busy: boolean;
  draft: string;
  setDraft: (value: string) => void;
  error: string;
  errorDetails: string;
  canFixConfiguration: boolean;
  submit: (event: FormEvent) => void | Promise<unknown>;
  cancel: () => void;
  providerModel: string;
  providerId: string;
  providerConfigured: boolean;
  conversationSidebarCollapsed: boolean;
  conversationSidebarWidth: number;
  onToggleConversationSidebar: () => void;
  onResizeConversationSidebar: (width: number) => void;
  onOpenLink: (url: string) => void;
  assistantUsesMascot: boolean;
  onFixConfiguration: () => void;
  onRetry: () => void;
  canRetry: boolean;
  profile: ProfileData;
  selectedPet: string;
  petState?: PetAnimationState;
  reduceMotion?: boolean;
  reduceMascot?: boolean;
  agentEnabled: boolean;
  agentUnavailableReason: string | null;
  agentWorkspacePath: string | null;
  agentRunning: boolean;
  agentAwaitingApproval: boolean;
  agentActionCards: AgentActionCardData[];
  onReviewApproval: () => void;
  onConfigureDiscord?: () => void;
  onUndo: (action: AgentActionCardData) => void;
  onOpenArtifact: (path: string) => void;
  onRevealArtifact: (path: string) => void;
  onToggleAgent: (enabled: boolean) => void;
  onStopAgent: () => void;
  onConfigureAgent: () => void;
  microphoneEnabled: boolean;
  onMicrophonePreferenceChange: (enabled: boolean) => void | Promise<void>;
  readRepliesAutomatically: boolean;
  speechVoiceURI: string;
  speechRate: number;
  speechVolume: number;
}) {
  const [search, setSearch] = useState('');
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [titleDraft, setTitleDraft] = useState('');
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const messagesAreaRef = useRef<HTMLDivElement>(null);
  const followMessagesRef = useRef(true);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const recordingStartRef = useRef(false);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<BlobPart[]>([]);
  const startedAtRef = useRef(0);
  const recordedMillisecondsRef = useRef(0);
  const [recordingState, setRecordingState] = useState<'idle' | 'recording' | 'paused' | 'ready'>(
    'idle',
  );
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const [recordingLevel, setRecordingLevel] = useState(0);
  const [recordingError, setRecordingError] = useState('');
  const [microphoneActivationOpen, setMicrophoneActivationOpen] = useState(false);
  const [microphoneActivationBusy, setMicrophoneActivationBusy] = useState(false);
  const microphoneActivationBusyRef = useRef(false);
  const [recordingUrl, setRecordingUrl] = useState<string | null>(null);
  const [transcriptionDialogOpen, setTranscriptionDialogOpen] = useState(false);
  const [transcriptionPrice, setTranscriptionPrice] = useState('');
  const [transcriptionBusy, setTranscriptionBusy] = useState(false);
  const recordingBlobRef = useRef<Blob | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const recordingUrlRef = useRef<string | null>(null);
  const lastSpeechMessageRef = useRef<{
    conversationId: string | null;
    message: ChatMessage | null;
  } | null>(null);
  const ignoreNextLoadedMessageRef = useRef(false);
  const [speechState, setSpeechState] = useState<SpeechState>({ key: null, speaking: false });
  const [speechError, setSpeechError] = useState('');
  const activeConversation = conversations.find((item) => item.id === conversationId);
  const conversationModelDiffers = Boolean(
    activeConversation?.model &&
    (activeConversation.provider_id !== providerId || activeConversation.model !== providerModel),
  );
  const filteredConversations = filterConversations(conversations, search);
  const presentedMessages = presentMessages(
    messages.filter((message) => message.role !== 'system'),
  );
  useEffect(
    () => setTitleDraft(activeConversation?.title ?? ''),
    [conversationId, activeConversation?.title],
  );
  useEffect(() => {
    const reader = getLocalSpeechReader();
    if (!reader) return;
    const unsubscribe = reader.subscribe(setSpeechState);
    return () => {
      reader.stop();
      unsubscribe();
    };
  }, []);
  useEffect(() => {
    const reader = getLocalSpeechReader();
    reader?.stop();
    ignoreNextLoadedMessageRef.current = true;
  }, [conversationId]);
  useEffect(() => {
    const reader = getLocalSpeechReader();
    const message = messages.at(-1) ?? null;
    const previous = lastSpeechMessageRef.current;
    if (!previous) {
      lastSpeechMessageRef.current = { conversationId, message };
      return;
    }
    if (previous.conversationId !== conversationId) {
      lastSpeechMessageRef.current = { conversationId, message };
      return;
    }
    if (previous.message === message) return;
    lastSpeechMessageRef.current = { conversationId, message };
    reader?.stop();
    if (ignoreNextLoadedMessageRef.current) {
      ignoreNextLoadedMessageRef.current = false;
      return;
    }
    if (message?.role === 'assistant' && readRepliesAutomatically) {
      try {
        reader?.speak(message.content, `message-${messages.length - 1}`, {
          voiceURI: speechVoiceURI,
          rate: speechRate,
          volume: speechVolume,
        });
        setSpeechError('');
      } catch (error) {
        setSpeechError(
          error instanceof Error ? error.message : 'Não foi possível iniciar a leitura.',
        );
      }
    }
  }, [
    conversationId,
    messages,
    readRepliesAutomatically,
    speechRate,
    speechVolume,
    speechVoiceURI,
  ]);
  useEffect(() => {
    followMessagesRef.current = true;
  }, [conversationId]);
  useEffect(() => {
    const area = messagesAreaRef.current;
    if (!area) return;
    if (followMessagesRef.current) area.scrollTop = area.scrollHeight;
  }, [messages, busy, agentActionCards]);

  useEffect(() => {
    const area = messagesAreaRef.current;
    if (!area) return;
    let observedContent: Element | null = null;
    const keepAtBottom = () => {
      if (followMessagesRef.current) area.scrollTop = area.scrollHeight;
    };
    const resizeObserver =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(keepAtBottom);
    const observeContent = () => {
      const nextContent = area.firstElementChild;
      if (nextContent === observedContent) return;
      if (observedContent) resizeObserver?.unobserve(observedContent);
      observedContent = nextContent;
      if (observedContent) resizeObserver?.observe(observedContent);
      keepAtBottom();
    };
    const mutationObserver =
      typeof MutationObserver === 'undefined' ? null : new MutationObserver(observeContent);
    observeContent();
    mutationObserver?.observe(area, { childList: true });
    window.addEventListener('resize', keepAtBottom);
    return () => {
      mutationObserver?.disconnect();
      resizeObserver?.disconnect();
      window.removeEventListener('resize', keepAtBottom);
    };
  }, []);

  useEffect(
    () => () => {
      streamRef.current?.getTracks().forEach((track) => track.stop());
      recorderRef.current = null;
      recordingBlobRef.current = null;
      if (recordingUrlRef.current) URL.revokeObjectURL(recordingUrlRef.current);
      void audioContextRef.current?.close();
    },
    [],
  );

  useEffect(() => {
    if (recordingState !== 'recording') return;
    const timer = window.setInterval(() => {
      const elapsed = recordedMillisecondsRef.current + Date.now() - startedAtRef.current;
      setRecordingSeconds(Math.floor(elapsed / 1000));
      if (elapsed >= 60_000) finishRecording();
    }, 100);
    return () => window.clearInterval(timer);
  }, [recordingState]);

  useEffect(() => {
    const stopInBackground = () => {
      if (document.visibilityState === 'hidden' && recorderRef.current?.state !== 'inactive') {
        cancelRecording();
        setRecordingError('A gravação foi descartada porque o aplicativo saiu de primeiro plano.');
      }
    };
    document.addEventListener('visibilitychange', stopInBackground);
    return () => document.removeEventListener('visibilitychange', stopInBackground);
  }, [recordingUrl]);

  async function startRecording(streamOverride?: MediaStream) {
    setRecordingError('');
    if (!streamOverride && microphoneClickAction(microphoneEnabled) === 'activate') {
      setMicrophoneActivationOpen(true);
      return;
    }
    if (recordingStartRef.current) return;
    recordingStartRef.current = true;
    let acquiredStream = streamOverride ?? null;
    try {
      if (!navigator.mediaDevices?.getUserMedia || !window.MediaRecorder) {
        setRecordingError('A gravação de áudio não está disponível neste WebView.');
        return;
      }
      const stream = acquiredStream ?? (await navigator.mediaDevices.getUserMedia({ audio: true }));
      acquiredStream = stream;
      streamRef.current = stream;
      chunksRef.current = [];
      const recorder = new MediaRecorder(stream);
      recorderRef.current = recorder;
      recorder.ondataavailable = (event) => {
        if (event.data.size) chunksRef.current.push(event.data);
      };
      recorder.onstop = () => {
        if (chunksRef.current.length && recordingBlobRef.current !== null) {
          const blob = new Blob(chunksRef.current, { type: recorder.mimeType || 'audio/webm' });
          recordingBlobRef.current = blob;
          const url = URL.createObjectURL(blob);
          recordingUrlRef.current = url;
          setRecordingUrl(url);
          setRecordingState('ready');
        } else {
          setRecordingState('idle');
        }
        chunksRef.current = [];
        stream.getTracks().forEach((track) => track.stop());
        streamRef.current = null;
        void audioContextRef.current?.close();
        audioContextRef.current = null;
        setRecordingLevel(0);
      };
      stream.getAudioTracks().forEach((track) => {
        track.addEventListener(
          'ended',
          () => {
            if (recorderRef.current === recorder) {
              setRecordingError('O microfone foi desconectado. A gravação foi interrompida.');
              recordActiveSegment();
              recordingBlobRef.current = null;
              recorder.stop();
            }
          },
          { once: true },
        );
      });
      const context = new AudioContext();
      audioContextRef.current = context;
      const analyser = context.createAnalyser();
      const source = context.createMediaStreamSource(stream);
      source.connect(analyser);
      const samples = new Uint8Array(analyser.fftSize);
      const meter = () => {
        if (recorderRef.current !== recorder || recorder.state !== 'recording') return;
        analyser.getByteTimeDomainData(samples);
        let sum = 0;
        for (const sample of samples) sum += ((sample - 128) / 128) ** 2;
        setRecordingLevel(Math.min(1, Math.sqrt(sum / samples.length) * 4));
        requestAnimationFrame(meter);
      };
      recorder.start(200);
      recordedMillisecondsRef.current = 0;
      startedAtRef.current = Date.now();
      recordingBlobRef.current = new Blob();
      setRecordingSeconds(0);
      setRecordingState('recording');
      requestAnimationFrame(meter);
    } catch (error) {
      acquiredStream?.getTracks().forEach((track) => track.stop());
      if (streamRef.current === acquiredStream) streamRef.current = null;
      recorderRef.current = null;
      void audioContextRef.current?.close();
      audioContextRef.current = null;
      setRecordingError(microphonePermissionError(error));
    } finally {
      recordingStartRef.current = false;
    }
  }

  async function confirmMicrophoneActivation() {
    if (microphoneActivationBusyRef.current) return;
    microphoneActivationBusyRef.current = true;
    setMicrophoneActivationBusy(true);
    setRecordingError('');
    try {
      if (!navigator.mediaDevices?.getUserMedia || !window.MediaRecorder) {
        throw new Error('A gravação de áudio não está disponível neste WebView.');
      }
      await activateMicrophone(
        () => onMicrophonePreferenceChange(true),
        () => navigator.mediaDevices.getUserMedia({ audio: true }),
        async (stream) => {
          setMicrophoneActivationOpen(false);
          await startRecording(stream);
        },
      );
    } catch (error) {
      setMicrophoneActivationOpen(false);
      setRecordingError(microphonePermissionError(error));
    } finally {
      microphoneActivationBusyRef.current = false;
      setMicrophoneActivationBusy(false);
    }
  }

  function finishRecording() {
    const recorder = recorderRef.current;
    if (!recorder || recorder.state === 'inactive') return;
    recordActiveSegment();
    setRecordingSeconds(Math.min(60, Math.ceil(recordedMillisecondsRef.current / 1000)));
    recordingBlobRef.current = new Blob();
    recorder.stop();
  }

  function recordActiveSegment() {
    if (startedAtRef.current) {
      recordedMillisecondsRef.current += Date.now() - startedAtRef.current;
      startedAtRef.current = 0;
    }
  }

  function pauseRecording() {
    if (recorderRef.current?.state !== 'recording') return;
    recordActiveSegment();
    setRecordingSeconds(Math.floor(recordedMillisecondsRef.current / 1000));
    recorderRef.current.pause();
    setRecordingState('paused');
  }

  function resumeRecording() {
    if (recorderRef.current?.state !== 'paused') return;
    startedAtRef.current = Date.now();
    recorderRef.current.resume();
    setRecordingState('recording');
  }

  function cancelRecording() {
    recordingBlobRef.current = null;
    chunksRef.current = [];
    const recorder = recorderRef.current;
    if (recorder && recorder.state !== 'inactive') recorder.stop();
    else {
      streamRef.current?.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
      setRecordingState('idle');
    }
    if (recordingUrlRef.current) URL.revokeObjectURL(recordingUrlRef.current);
    recordingUrlRef.current = null;
    setRecordingUrl(null);
    setRecordingSeconds(0);
    recordedMillisecondsRef.current = 0;
    startedAtRef.current = 0;
  }

  function discardRecording() {
    cancelRecording();
    recordingBlobRef.current = null;
    setRecordingState('idle');
  }

  async function confirmTranscription() {
    const audio = recordingBlobRef.current;
    if (!audio || !supportsTranscription(providerId)) return;
    setTranscriptionBusy(true);
    setRecordingError('');
    try {
      const transcript = await transcribeConfirmedRecording(
        true,
        providerId,
        audio,
        (selectedProvider, mediaType, audioBase64) =>
          invoke<string>('transcribe_audio', {
            providerId: selectedProvider,
            mediaType,
            audioBase64,
          }),
      );
      if (transcript) setDraft(transcript);
      setTranscriptionDialogOpen(false);
    } catch (error) {
      setRecordingError(
        error instanceof Error ? error.message : 'A transcrição falhou. Tente novamente.',
      );
      setTranscriptionDialogOpen(false);
    } finally {
      recordingBlobRef.current = null;
      chunksRef.current = [];
      if (recordingUrlRef.current) URL.revokeObjectURL(recordingUrlRef.current);
      recordingUrlRef.current = null;
      setRecordingUrl(null);
      setRecordingState('idle');
      setTranscriptionBusy(false);
    }
  }

  return (
    <div
      className={`chat-view ${conversationSidebarCollapsed ? 'conversation-sidebar-collapsed' : ''}`}
      style={
        {
          '--conversation-sidebar-width': `${conversationSidebarCollapsed ? 0 : conversationSidebarWidth}px`,
        } as CSSProperties
      }
    >
      <aside className="conversation-sidebar" aria-label="Lista de conversas">
        <Button type="button" variant="primary" onClick={onNewConversation} disabled={busy}>
          Nova conversa
        </Button>
        <label className="conversation-search">
          <span className="sr-only">Buscar conversas</span>
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Buscar conversas"
          />
        </label>
        <div className="conversation-list">
          {filteredConversations.map((item) => (
            <div
              className={`conversation-item ${conversationId === item.id ? 'selected' : ''}`}
              key={item.id}
            >
              <button
                type="button"
                className="conversation-select"
                onClick={() => onConversation(item.id)}
                disabled={busy}
                aria-current={conversationId === item.id ? 'page' : undefined}
              >
                <strong>{item.title}</strong>
                <time dateTime={new Date(item.created_at * 1000).toISOString()}>
                  {new Date(item.created_at * 1000).toLocaleDateString('pt-BR')}
                </time>
                <span>{item.preview}</span>
              </button>
              <button
                type="button"
                className="conversation-menu-trigger"
                aria-label={`Opções da conversa ${item.title}`}
                aria-expanded={openMenuId === item.id}
                onClick={() => setOpenMenuId(openMenuId === item.id ? null : item.id)}
                disabled={busy}
              >
                <Icon name="more" />
              </button>
              {openMenuId === item.id && (
                <div className="conversation-menu" role="menu">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setOpenMenuId(null);
                      const title = window.prompt('Novo título da conversa:', item.title);
                      if (title !== null) onRenameConversation(item.id, title);
                    }}
                  >
                    Renomear
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setOpenMenuId(null);
                      onDeleteConversation(item.id);
                    }}
                  >
                    Apagar
                  </button>
                </div>
              )}
            </div>
          ))}
          {filteredConversations.length === 0 && (
            <div className="conversation-list-empty">
              <PetSprite
                estado={petState}
                petId={selectedPet}
                reduceMotion={reduceMotion}
                staticImage={reduceMascot}
                className="empty-state-pet"
              />
              <span>
                {search ? 'Nenhuma conversa encontrada.' : 'Suas conversas aparecerão aqui.'}
              </span>
            </div>
          )}
        </div>
        {!conversationSidebarCollapsed && (
          <ResizeHandle
            label="Redimensionar lista de conversas"
            value={conversationSidebarWidth}
            min={170}
            max={360}
            onResize={onResizeConversationSidebar}
          />
        )}
      </aside>
      <button
        type="button"
        className="conversation-panel-toggle"
        aria-label={
          conversationSidebarCollapsed
            ? 'Expandir lista de conversas'
            : 'Recolher lista de conversas'
        }
        aria-expanded={!conversationSidebarCollapsed}
        onClick={onToggleConversationSidebar}
      >
        {conversationSidebarCollapsed ? '›' : '‹'}
      </button>
      <section className="chat-thread" aria-label="Conversa atual">
        <header className="chat-header">
          <div className="chat-header-main">
            <div className="chat-title-wrap">
              <span className="eyebrow">CONVERSA</span>
              {activeConversation ? (
                <input
                  className="chat-title-input"
                  value={titleDraft}
                  maxLength={120}
                  aria-label="Título da conversa"
                  onChange={(event) => setTitleDraft(event.target.value)}
                  onBlur={() => {
                    if (titleDraft.trim() && titleDraft.trim() !== activeConversation.title)
                      onRenameConversation(activeConversation.id, titleDraft);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') event.currentTarget.blur();
                    if (event.key === 'Escape') setTitleDraft(activeConversation.title);
                  }}
                />
              ) : (
                <h2>Nova conversa</h2>
              )}
            </div>
            {conversationModelDiffers && activeConversation && (
              <span className="chat-model-chip" title="Modelo usado nesta conversa">
                {activeConversation.provider_id !== providerId
                  ? `${providerById(activeConversation.provider_id ?? '')?.name ?? activeConversation.provider_id} · `
                  : ''}
                {activeConversation.model}
              </span>
            )}
          </div>
          <AgentControls
            enabled={agentEnabled}
            disabledReason={agentUnavailableReason}
            workspacePath={agentWorkspacePath}
            running={agentRunning}
            awaitingApproval={agentAwaitingApproval}
            onToggle={onToggleAgent}
            onStop={onStopAgent}
            onConfigure={onConfigureAgent}
          />
        </header>
        {!providerConfigured && (
          <div className="chat-provider-needed" role="status">
            <span>Configure um provedor de IA para começar a conversar.</span>
            <Button type="button" variant="secondary" onClick={onFixConfiguration}>
              Abrir Conectores
            </Button>
          </div>
        )}
        <div
          className="messages-area"
          aria-live="polite"
          ref={messagesAreaRef}
          onScroll={(event) => {
            const area = event.currentTarget;
            followMessagesRef.current = shouldFollowChatScroll(
              area.scrollHeight,
              area.scrollTop,
              area.clientHeight,
            );
          }}
        >
          {messages.length === 0 && agentActionCards.length === 0 ? (
            <div className="empty-chat">
              <PetSprite
                estado={petState}
                petId={selectedPet}
                reduceMotion={reduceMotion}
                staticImage={reduceMascot}
                className="empty-state-pet empty-chat-pet"
              />
              <strong>O que você gostaria de explorar?</strong>
              <p>Escreva uma pergunta, uma ideia ou algo que queira organizar.</p>
              <div className="prompt-chips">
                <button
                  type="button"
                  onClick={() => setDraft('Ajude-me a organizar minhas prioridades de hoje.')}
                >
                  Organizar meu dia <Icon name="external-link" />
                </button>
                <button type="button" onClick={() => setDraft('Quero explorar uma ideia nova: ')}>
                  Explorar uma ideia <Icon name="external-link" />
                </button>
                <button type="button" onClick={() => setDraft('Explique de forma simples: ')}>
                  Entender algo novo <Icon name="external-link" />
                </button>
              </div>
            </div>
          ) : (
            <div className="message-list">
              {presentedMessages.map(({ message, side }, index) => (
                <article className={`message message--${side}`} key={`${index}-${message.role}`}>
                  {message.role === 'assistant' && (
                    <span className="message-avatar">
                      {assistantUsesMascot ? (
                        <PetSprite
                          estado={petState}
                          petId={selectedPet}
                          reduceMotion={reduceMotion}
                          staticImage={reduceMascot}
                          avatar
                        />
                      ) : (
                        <img src={appIcon} alt="" />
                      )}
                    </span>
                  )}
                  <div className="message-content">
                    <span className="message-role">
                      {message.role === 'user' ? profile.displayName || 'Você' : 'OdeKrai'}
                    </span>
                    {message.role === 'assistant' ? (
                      <MessageMarkdown text={message.content} onOpenLink={onOpenLink} />
                    ) : (
                      <p className="message-plain">{message.content}</p>
                    )}
                    {message.citations?.length ? (
                      <div className="message-citations">
                        {message.citations.map((citation) =>
                          safeMarkdownUrl(citation.url) ? (
                            <button
                              type="button"
                              role="link"
                              key={citation.url}
                              className="message-link message-citation-link"
                              title={safeMarkdownUrl(citation.url) ?? citation.url}
                              onClick={() => onOpenLink(safeMarkdownUrl(citation.url) ?? '')}
                            >
                              {citation.title}
                            </button>
                          ) : (
                            <span key={citation.url}>{citation.title}</span>
                          ),
                        )}
                      </div>
                    ) : null}
                    <time className="message-time">
                      {new Date(
                        (message.created_at ?? Math.floor(Date.now() / 1000)) * 1000,
                      ).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })}
                    </time>
                    {message.role === 'assistant' && (
                      <button
                        type="button"
                        className="secondary-button small-button message-speech-button"
                        aria-label={
                          speechState.key === `message-${index}` ? 'Parar leitura' : 'Ouvir'
                        }
                        onClick={() => {
                          const reader = getLocalSpeechReader();
                          if (speechState.key === `message-${index}`) {
                            reader?.stop();
                            setSpeechError('');
                            return;
                          }
                          try {
                            if (!reader)
                              throw new Error(
                                'A leitura em voz alta não está disponível neste WebView.',
                              );
                            reader.speak(message.content, `message-${index}`, {
                              voiceURI: speechVoiceURI,
                              rate: speechRate,
                              volume: speechVolume,
                            });
                            setSpeechError('');
                          } catch (error) {
                            setSpeechError(
                              error instanceof Error
                                ? error.message
                                : 'Não foi possível iniciar a leitura.',
                            );
                          }
                        }}
                      >
                        {speechState.key === `message-${index}` ? 'Parar' : 'Ouvir'}
                      </button>
                    )}
                  </div>
                  {message.role === 'user' && (
                    <span className="message-avatar message-avatar-user">
                      {profile.avatarDataUrl ? (
                        <img src={profile.avatarDataUrl} alt="" />
                      ) : (
                        initials(profile.displayName)
                      )}
                    </span>
                  )}
                </article>
              ))}
              {speechError && (
                <p className="speech-error" role="alert">
                  {speechError}
                </p>
              )}
              {agentActionCards.map((action) => (
                <AgentActionCard
                  action={action}
                  key={action.id}
                  running={agentRunning && !agentAwaitingApproval && action.status === 'Executando'}
                  onStop={cancel}
                  onReviewApproval={onReviewApproval}
                  onConfigureDiscord={onConfigureDiscord}
                  onOpenArtifact={onOpenArtifact}
                  onRevealArtifact={onRevealArtifact}
                  onUndo={() => onUndo(action)}
                />
              ))}
              {busy && !agentAwaitingApproval && (
                <div className="thinking-row">
                  <span className="thinking-dots">
                    <i />
                    <i />
                    <i />
                  </span>
                  <span>OdeKrai está pensando</span>
                </div>
              )}
            </div>
          )}
        </div>
        {error && (
          <div className="chat-error" role="alert">
            <p className="chat-error-message">{error}</p>
            {errorDetails && (
              <details className="chat-error-details">
                <summary>Detalhes técnicos</summary>
                <pre>{errorDetails}</pre>
              </details>
            )}
            <div>
              {canRetry && (
                <Button type="button" variant="secondary" onClick={onRetry} disabled={busy}>
                  Tentar novamente
                </Button>
              )}
              {canFixConfiguration && (
                <Button type="button" variant="ghost" onClick={onFixConfiguration}>
                  {error.toLocaleLowerCase('pt-BR').includes('pasta do odekrai')
                    ? 'Resolver pasta'
                    : error.toLocaleLowerCase('pt-BR').includes('habilidade')
                      ? 'Ativar habilidade'
                      : error.toLocaleLowerCase('pt-BR').includes('limite diário')
                        ? 'Ver limites'
                        : 'Corrigir configuração'}
                </Button>
              )}
            </div>
          </div>
        )}
        <MessageComposer
          draft={draft}
          setDraft={setDraft}
          busy={busy}
          onSubmit={submit}
          onCancel={cancel}
          textareaRef={composerRef}
          footer={(sendButton) => (
            <div className="composer-bottom">
              {!recordingUrl && recordingState === 'idle' && (
                <button
                  type="button"
                  className="secondary-button small-button"
                  aria-label="Gravar áudio"
                  title="Gravar áudio"
                  onClick={() => void startRecording()}
                >
                  <Icon name="microphone" size={18} />
                </button>
              )}
              {(recordingState === 'recording' || recordingState === 'paused') && (
                <div className="audio-recording" role="status" aria-live="polite">
                  <strong>
                    {recordingState === 'recording' ? 'Gravando' : 'Gravação pausada'} ·{' '}
                    {Math.floor(recordingSeconds / 60)}:
                    {String(recordingSeconds % 60).padStart(2, '0')}
                  </strong>
                  {recordingState === 'recording' && (
                    <span className="audio-level" aria-label="Nível do microfone">
                      <i style={{ transform: `scaleX(${recordingLevel})` }} />
                    </span>
                  )}
                  <span>Nenhum áudio será enviado. A gravação para ao sair desta tela.</span>
                  {recordingState === 'recording' ? (
                    <button type="button" onClick={pauseRecording}>
                      Pausar
                    </button>
                  ) : (
                    <button type="button" onClick={resumeRecording}>
                      Retomar
                    </button>
                  )}
                  <button type="button" onClick={cancelRecording}>
                    Cancelar
                  </button>
                  <button type="button" onClick={finishRecording}>
                    Concluir
                  </button>
                </div>
              )}
              {recordingUrl && recordingState === 'ready' && (
                <div className="audio-recording" role="group" aria-label="Gravação local concluída">
                  <audio controls src={recordingUrl} />
                  {providerConfigured && supportsTranscription(providerId) && (
                    <button type="button" onClick={() => setTranscriptionDialogOpen(true)}>
                      Transcrever com {providerById(providerId)?.name ?? providerId}
                    </button>
                  )}
                  <button type="button" onClick={discardRecording}>
                    Descartar
                  </button>
                </div>
              )}
              <span className="chat-composer-helper">Enter envia · Shift + Enter quebra linha</span>
              <div className="chat-composer-actions">{sendButton}</div>
            </div>
          )}
        >
          {recordingError && (
            <p className="audio-recording-error" role="alert">
              {recordingError}
            </p>
          )}
          {transcriptionDialogOpen && recordingBlobRef.current && (
            <div
              className="audio-transcription-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby="audio-transcription-title"
            >
              <h3 id="audio-transcription-title">Confirmar envio do áudio</h3>
              <p>
                Duração: {Math.floor(recordingSeconds / 60)}:
                {String(recordingSeconds % 60).padStart(2, '0')}
              </p>
              <p>Provedor: {providerById(providerId)?.name ?? providerId}</p>
              <p>O áudio será enviado a este provedor para transcrição.</p>
              <label>
                Preço informado por minuto (USD), opcional
                <input
                  type="number"
                  min="0"
                  step="0.0001"
                  value={transcriptionPrice}
                  onChange={(event) => setTranscriptionPrice(event.target.value)}
                />
              </label>
              {transcriptionPrice !== '' &&
                Number.isFinite(Number(transcriptionPrice)) &&
                Number(transcriptionPrice) >= 0 && (
                  <p>
                    Custo estimado: $
                    {((recordingSeconds / 60) * Number(transcriptionPrice)).toFixed(4)} USD
                  </p>
                )}
              <div>
                <button
                  type="button"
                  disabled={transcriptionBusy}
                  onClick={() => setTranscriptionDialogOpen(false)}
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  disabled={transcriptionBusy}
                  onClick={() => void confirmTranscription()}
                >
                  {transcriptionBusy ? 'Transcrevendo…' : 'Confirmar e transcrever'}
                </button>
              </div>
            </div>
          )}
        </MessageComposer>
        <Modal
          open={microphoneActivationOpen}
          title="Ativar microfone?"
          onClose={() => {
            if (!microphoneActivationBusy) setMicrophoneActivationOpen(false);
          }}
        >
          <MicrophoneActivationDialogContent
            busy={microphoneActivationBusy}
            onCancel={() => setMicrophoneActivationOpen(false)}
            onActivate={() => void confirmMicrophoneActivation()}
          />
        </Modal>
        <p className="chat-disclaimer">
          As respostas podem conter imprecisões. As mensagens são enviadas ao provedor configurado.
        </p>
      </section>
    </div>
  );
}
