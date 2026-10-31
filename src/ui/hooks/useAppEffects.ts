import { useEffect } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { AI_PROVIDERS } from '../../core/providers';
import { sanitizeDiagnosticDetails } from '../../core/diagnostics';
import { loadSettings } from '../../core/settings';
import { stopLocalSpeech } from '../../core/speech';
import { normalizeSoundPreferences, playSound } from '../../core/sounds';
import type { ChatMessage } from '../../core/provider-messages';
import { mergeApprovedProfile, normalizeProfileData } from '../../core/identity';
import type { useAppState } from './useAppState';
import type { Conversation } from '../screens/types';
import type { ProfileData } from '../Profile';

export { sanitizeDiagnosticDetails } from '../../core/diagnostics';

const STARTUP_TIMEOUT_MS = 5000;

function correlationId(): string {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (character) => {
    const value = Math.floor(Math.random() * 16);
    return (character === 'x' ? value : (value & 0x3) | 0x8).toString(16);
  });
}

function timeout<T>(promise: Promise<T>, command: string): Promise<T> {
  let timer: number | undefined;
  return Promise.race([
    promise,
    new Promise<T>(
      (_, reject) =>
        (timer = window.setTimeout(
          () => reject(new Error(`${command} excedeu o tempo limite`)),
          STARTUP_TIMEOUT_MS,
        )),
    ),
  ]).finally(() => {
    if (timer !== undefined) window.clearTimeout(timer);
  });
}

function diagnosticMessage(command: string) {
  const messages: Record<string, string> = {
    get_selected_pet: 'Não foi possível carregar o personagem salvo.',
    get_permission_settings: 'Não foi possível carregar as permissões.',
    get_profile: 'Não foi possível carregar o perfil local.',
    agent_home_status: 'Não foi possível verificar a pasta do OdeKrai.',
    agent_home_read_core: 'Não foi possível recuperar o perfil aprovado.',
    list_conversations: 'Não foi possível carregar o histórico local.',
    load_conversation: 'Não foi possível abrir a conversa anterior.',
    has_provider_credential: 'Não foi possível verificar a credencial do provedor.',
  };
  return messages[command] ?? 'A inicialização encontrou uma falha.';
}

export async function startupInvoke<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  reportDiagnostic: (command: string, message: string, details?: string) => void,
): Promise<T> {
  const started = performance.now();
  const id = correlationId();
  let outcome = 'ok';
  void invoke('record_diagnostic_event', {
    event: 'startup_command_start',
    command,
    outcome: 'ok',
    durationMs: 0,
    section: 'startup',
    category: 'initialization',
    correlationId: id,
  }).catch(() => console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.'));
  try {
    const result = await timeout(invoke<T>(command, args), command);
    reportDiagnostic(command, '');
    return result;
  } catch (error) {
    outcome = 'error';
    reportDiagnostic(command, diagnosticMessage(command), sanitizeDiagnosticDetails(error));
    throw error;
  } finally {
    const durationMs = Math.max(0, Math.round(performance.now() - started));
    void invoke('record_diagnostic_event', {
      event: 'startup_command_finish',
      command,
      outcome,
      durationMs,
      section: 'startup',
      category: 'initialization',
      correlationId: id,
    }).catch(() =>
      console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.'),
    );
  }
}

export function useAppEffects(state: ReturnType<typeof useAppState>) {
  const {
    settings,
    setPetVisible,
    setSelectedPet,
    setPermissionIndicator,
    setToast,
    reportDiagnostic,
    setProfile,
    setIdentityConfigured,
    setConversations,
    setConversationId,
    setMessages,
    toast,
    setKeySavedForSession,
    request,
    companionStateTimer,
  } = state;
  useEffect(() => {
    if (!isTauri()) return;
    let current = true;
    let listenersDisposed = false;
    void (async () => {
      let recoveredFromIncompleteRun = false;
      try {
        recoveredFromIncompleteRun = await startupInvoke<boolean>(
          'get_startup_state',
          undefined,
          reportDiagnostic,
        );
      } catch {
        recoveredFromIncompleteRun = true;
      }
      if (!current) return;
      const provider = AI_PROVIDERS.find((item) => item.id === settings.activeProviderId);
      const results = await Promise.allSettled([
        startupInvoke<string>('get_selected_pet', undefined, reportDiagnostic),
        startupInvoke<{ mode: string; halted: boolean }>(
          'get_permission_settings',
          undefined,
          reportDiagnostic,
        ),
        startupInvoke<ProfileData>('get_profile', undefined, reportDiagnostic),
        startupInvoke<{ configured: boolean; state: string }>(
          'agent_home_status',
          undefined,
          reportDiagnostic,
        ),
        startupInvoke<Conversation[]>('list_conversations', undefined, reportDiagnostic),
        provider
          ? startupInvoke<boolean>(
              'has_provider_credential',
              { providerId: provider.id },
              reportDiagnostic,
            )
          : Promise.resolve(false),
      ]);
      if (!current) return;
      const [pet, permissions, savedProfile, home, history, credential] = results;
      const currentProfile = normalizeProfileData(
        savedProfile.status === 'fulfilled' ? savedProfile.value : undefined,
      );
      if (pet.status === 'fulfilled') setSelectedPet(pet.value);
      if (permissions.status === 'fulfilled') setPermissionIndicator(permissions.value);
      if (savedProfile.status === 'fulfilled') setProfile(currentProfile);
      if (home.status === 'fulfilled') setIdentityConfigured(home.value.configured);
      if (credential.status === 'fulfilled')
        setKeySavedForSession(credential.value || provider?.id === 'local');
      if (history.status === 'fulfilled') {
        setConversations(history.value);
        if (history.value[0]) {
          setConversationId(history.value[0].id);
          try {
            setMessages(
              await startupInvoke<ChatMessage[]>(
                'load_conversation',
                { conversationId: history.value[0].id },
                reportDiagnostic,
              ),
            );
          } catch {
            // O banner persistente identifica o comando que falhou.
          }
        }
      }
      if (
        home.status === 'fulfilled' &&
        home.value.state === 'integra' &&
        (!currentProfile.displayName.trim() || !currentProfile.nickname.trim())
      ) {
        try {
          const document = await startupInvoke<{ approved_content: string }>(
            'agent_home_read_core',
            undefined,
            reportDiagnostic,
          );
          const recovered = mergeApprovedProfile(currentProfile, document.approved_content);
          if (recovered) {
            setProfile(recovered);
            const saved = await startupInvoke<ProfileData>(
              'save_profile',
              { displayName: recovered.displayName, nickname: recovered.nickname },
              reportDiagnostic,
            );
            setProfile(normalizeProfileData(saved));
          }
        } catch {
          // O banner persistente identifica o comando que falhou.
        }
      }
      if (recoveredFromIncompleteRun) {
        reportDiagnostic(
          'modo_seguro',
          'A inicialização anterior não terminou; pet e ilha ficarão desligados nesta sessão.',
        );
      } else {
        const companion = settings.companion;
        if (companion.enabled) {
          const width = 280 * companion.size;
          const height = 150 * companion.size;
          const right = companion.position.endsWith('right');
          const bottom = companion.position.startsWith('bottom');
          const x =
            companion.coordinates?.x ?? (right ? Math.max(0, screen.availWidth - width - 28) : 28);
          const y =
            companion.coordinates?.y ??
            (bottom ? Math.max(0, screen.availHeight - height - 58) : 28);
          try {
            await timeout(
              (async () => {
                await startupInvoke<void>(
                  'set_pet_position',
                  { x, y, size: companion.size },
                  reportDiagnostic,
                );
                await startupInvoke<void>(
                  'set_pet_size',
                  {
                    width: (companion.freeMode ? 176 : 280) * companion.size,
                    height: (companion.freeMode ? 176 : 150) * companion.size,
                  },
                  reportDiagnostic,
                );
                await startupInvoke<void>(
                  'set_pet_position',
                  { x, y, size: companion.size },
                  reportDiagnostic,
                );
                await startupInvoke<void>(
                  'set_pet_topmost',
                  { enabled: companion.alwaysOnTop },
                  reportDiagnostic,
                );
                await startupInvoke<void>('set_pet_visible', { visible: true }, reportDiagnostic);
                setPetVisible(true);
                await emit('companion-state', { type: 'reveal' });
              })(),
              'restore_pet',
            );
          } catch {
            reportDiagnostic('set_pet_visible', 'Não foi possível restaurar o pet do assistente.');
          }
        }
      }
      try {
        await startupInvoke<void>('complete_startup', undefined, reportDiagnostic);
      } catch {
        // A falha fica registrada no banner e a janela principal continua utilizável.
      }
    })().catch((error: unknown) => {
      reportDiagnostic(
        'startup',
        'A inicialização foi interrompida por uma falha inesperada.',
        sanitizeDiagnosticDetails(error),
      );
    });
    let unlisten: (() => void)[] = [];
    void Promise.all([
      listen('security-halted', () => {
        stopLocalSpeech();
        setToast('O OdeKrai foi parado. Ações e respostas em andamento foram canceladas.');
        void playSound('halted', normalizeSoundPreferences(loadSettings().companion.sounds));
        void startupInvoke<{ mode: string; halted: boolean }>(
          'get_permission_settings',
          undefined,
          reportDiagnostic,
        )
          .then(setPermissionIndicator)
          .catch(() => setPermissionIndicator(null));
      }),
      listen<boolean>('pet-visibility-changed', ({ payload }) => setPetVisible(payload)),
      listen('security-resumed', () => {
        setToast('O OdeKrai foi retomado.');
        if (loadSettings().companion.enabled) {
          void startupInvoke<void>('set_pet_visible', { visible: true }, reportDiagnostic).then(
            () => setPetVisible(true),
          );
        }
        void startupInvoke<{ mode: string; halted: boolean }>(
          'get_permission_settings',
          undefined,
          reportDiagnostic,
        )
          .then(setPermissionIndicator)
          .catch(() => setPermissionIndicator(null));
      }),
      listen('permission-settings-updated', () => {
        void startupInvoke<{ mode: string; halted: boolean }>(
          'get_permission_settings',
          undefined,
          reportDiagnostic,
        )
          .then(setPermissionIndicator)
          .catch(() => setPermissionIndicator(null));
      }),
      listen<string>('security-error', ({ payload }) => setToast(payload)),
    ])
      .then((off) => {
        if (listenersDisposed) off.forEach((stop) => stop());
        else unlisten = off;
      })
      .catch(() =>
        reportDiagnostic('listen', 'Não foi possível iniciar alguns avisos de segurança.'),
      );
    const onError = (event: ErrorEvent) => {
      const id = correlationId();
      const details = sanitizeDiagnosticDetails(event.error ?? event.message);
      reportDiagnostic(
        'frontend.window.onerror',
        'A interface encontrou um erro.',
        `Referência ${id}: ${details || 'O WebView não informou o motivo do erro.'}`,
      );
      void invoke('record_diagnostic_event', {
        event: 'frontend_window_error',
        command: 'window.onerror',
        outcome: 'error',
        durationMs: 0,
        section: 'window',
        category: 'window_error',
        correlationId: id,
      }).catch(() =>
        console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.'),
      );
    };
    const onRejection = (event: PromiseRejectionEvent) => {
      const detail = sanitizeDiagnosticDetails(event.reason);
      const id = correlationId();
      reportDiagnostic(
        'frontend.unhandledrejection',
        'Uma ação falhou.',
        `Referência ${id}: ${detail || 'O WebView não informou o motivo da rejeição.'}`,
      );
      void invoke('record_diagnostic_event', {
        event: 'frontend_unhandled_rejection',
        command: 'unhandledrejection',
        outcome: 'error',
        durationMs: 0,
        section: 'window',
        category: 'promise_rejection',
        correlationId: id,
      }).catch(() =>
        console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.'),
      );
    };
    window.addEventListener('error', onError);
    window.addEventListener('unhandledrejection', onRejection);
    return () => {
      current = false;
      listenersDisposed = true;
      unlisten.forEach((off) => off());
      window.removeEventListener('error', onError);
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(''), 2600);
    return () => window.clearTimeout(timer);
  }, [toast]);
  useEffect(
    () => () => {
      if (request.current) void invoke('cancel_provider_request', { requestId: request.current });
      window.clearTimeout(companionStateTimer.current);
    },
    [],
  );
}
