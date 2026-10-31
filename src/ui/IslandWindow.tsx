import { useEffect, useRef, useState, type FormEvent } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  initialIslandSnapshot,
  islandLabels,
  transitionIsland,
  type IslandEvent,
  type IslandSnapshot,
} from '../core/island';
import { loadSettings, saveSettings } from '../core/settings';
import { normalizeSoundPreferences, playSound } from '../core/sounds';
import { DEFAULT_PET_ID, PETS, petAnimationForIslandState, petGreeting } from '../core/pets';
import { PetSprite } from './PetSprite';
import { Icon } from './Icon';
import { petWindowLayout } from '../core/pet-layout';
import {
  createIslandQuickRequest,
  canSubmitIslandQuickCommand,
  islandKeyboardFocus,
  isIslandReviewState,
  islandSuggestions,
  type IslandReviewState,
  type IslandQuickRequest,
} from '../core/island-quick';
import type { HomeSuggestion } from '../core/home';
import type { PerformanceModeStatus } from '../core/performance';

type IslandQuickResult = {
  requestId: string;
  conversationId: string;
  title: string;
  summary: string;
  success: boolean;
};

type IslandAlertSummary = {
  alertId: string;
  state: IslandReviewState;
  message: string;
};

type IslandAlertPayload = {
  alertId?: string;
  state: IslandSnapshot['state'];
  message: string;
  count: number;
  pinned: boolean;
  alerts?: IslandAlertSummary[];
};

export function IslandWindow() {
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(initialIslandSnapshot);
  const [settings, setSettings] = useState(loadSettings);
  const [selectedPetId, setSelectedPetId] = useState(DEFAULT_PET_ID);
  const [quickDraft, setQuickDraft] = useState('');
  const [quickSuggestions, setQuickSuggestions] = useState<HomeSuggestion[]>([]);
  const [quickBusy, setQuickBusy] = useState(false);
  const [quickResult, setQuickResult] = useState<IslandQuickResult | null>(null);
  const [quickError, setQuickError] = useState('');
  const [queuedAlerts, setQueuedAlerts] = useState<IslandAlertSummary[]>([]);
  const quickInput = useRef<HTMLInputElement>(null);
  const focusEnabled = useRef(false);
  const quickRequestId = useRef<string | null>(null);
  const moveTimer = useRef<number | undefined>(undefined);
  const initialLayout = petWindowLayout(
    settings.companion.freeMode,
    false,
    settings.companion.size,
  );
  const layoutRef = useRef(initialLayout);
  useEffect(() => {
    document.body.classList.add('island-window-body');
    document.documentElement.classList.add('island-window-html');
    const refreshSettings = () => setSettings(loadSettings());
    window.addEventListener('storage', refreshSettings);
    let unlisten: (() => void)[] = [];
    void Promise.all([
      listen<IslandEvent>('companion-state', ({ payload }) => {
        if (payload.type !== 'activity') {
          setSnapshot((current) => transitionIsland(current, payload));
          return;
        }
        if (payload.state === 'awaiting_approval') return;
        void invoke('enqueue_pet_activity', { state: payload.state }).catch(() =>
          console.warn(
            '[OdeKrai diagnóstico] Falha no comando enqueue_pet_activity da janela do pet.',
          ),
        );
      }),
      listen<IslandAlertPayload>('pet-alert-state', ({ payload }) => {
        setQueuedAlerts((payload.alerts ?? []).filter((alert) => isIslandReviewState(alert.state)));
        setSnapshot((current) =>
          transitionIsland(current, {
            type: 'sync-alert',
            alert: {
              alertId: payload.alertId,
              state: payload.state,
              message: payload.message,
              alertCount: payload.count,
              pinned: payload.pinned,
            },
          }),
        );
      }),
      listen<[number, number]>('pet-moved', ({ payload: [x, y] }) => {
        window.clearTimeout(moveTimer.current);
        moveTimer.current = window.setTimeout(() => {
          const current = loadSettings();
          saveSettings({ ...current, companion: { ...current.companion, coordinates: { x, y } } });
        }, 180);
      }),
      listen<string>('selected-pet-updated', ({ payload }) => {
        if (PETS.some((pet) => pet.id === payload)) setSelectedPetId(payload);
      }),
      listen<PerformanceModeStatus>('performance-mode-updated', ({ payload }) => {
        document.documentElement.dataset.performance = payload.enabled ? 'on' : 'off';
      }),
      listen<HomeSuggestion[]>('pet-quick-suggestions', ({ payload }) =>
        setQuickSuggestions(islandSuggestions(payload)),
      ),
      listen<IslandQuickResult>('pet-quick-result', ({ payload }) => {
        if (payload.requestId !== quickRequestId.current) return;
        setQuickBusy(false);
        setQuickResult(payload);
        setQuickDraft('');
        if (!payload.success) setQuickError(payload.summary);
      }),
    ])
      .then((off) => {
        unlisten = off;
        void emit('pet-window-ready');
      })
      .catch(() => {
        console.error('[OdeKrai diagnóstico] Falha ao registrar eventos da janela do pet.');
      });
    void invoke<string>('get_selected_pet')
      .then((petId) => {
        if (PETS.some((pet) => pet.id === petId)) setSelectedPetId(petId);
      })
      .catch(() =>
        console.warn('[OdeKrai diagnóstico] Falha no comando get_selected_pet da janela do pet.'),
      );
    void invoke<IslandAlertPayload>('get_pet_alert_state')
      .then((payload) => {
        setQueuedAlerts((payload.alerts ?? []).filter((alert) => isIslandReviewState(alert.state)));
        setSnapshot((current) =>
          transitionIsland(current, {
            type: 'sync-alert',
            alert: {
              alertId: payload.alertId,
              state: payload.state,
              message: payload.message,
              alertCount: payload.count,
              pinned: payload.pinned,
            },
          }),
        );
      })
      .catch(() =>
        console.warn(
          '[OdeKrai diagnóstico] Falha no comando get_pet_alert_state da janela do pet.',
        ),
      );
    return () => {
      document.body.classList.remove('island-window-body');
      document.documentElement.classList.remove('island-window-html');
      window.removeEventListener('storage', refreshSettings);
      window.clearTimeout(moveTimer.current);
      unlisten.forEach((off) => off());
      if (focusEnabled.current) void invoke('set_pet_keyboard_focus', { focused: false });
    };
  }, []);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused || !focusEnabled.current) return;
        focusEnabled.current = islandKeyboardFocus(focusEnabled.current, 'blur');
        void invoke('set_pet_keyboard_focus', { focused: false }).catch(() => undefined);
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch(() => setQuickError('Não foi possível acompanhar o foco do campo rápido.'));
    return () => unlisten?.();
  }, []);
  useEffect(() => {
    let active = true;
    const scale = settings.companion.size;
    const start = layoutRef.current;
    const end = petWindowLayout(settings.companion.freeMode, snapshot.state === 'expanded', scale);
    void (async () => {
      for (let step = 1; step <= 8 && active; step++) {
        const width = start.width + ((end.width - start.width) * step) / 8;
        const height = start.height + ((end.height - start.height) * step) / 8;
        await invoke('set_pet_size', {
          width,
          height,
        });
        layoutRef.current = { width, height };
        await new Promise((resolve) => window.setTimeout(resolve, 18));
      }
    })().catch(() => {
      console.error('[OdeKrai diagnóstico] Falha no comando set_pet_size da janela do pet.');
    });
    return () => {
      active = false;
    };
  }, [snapshot.state, settings.companion.freeMode, settings.companion.size]);
  useEffect(() => {
    if (snapshot.state === 'expanded') {
      const timer = window.setTimeout(
        () => setSnapshot((current) => transitionIsland(current, { type: 'timeout' })),
        settings.companion.collapseSeconds * 1000,
      );
      return () => window.clearTimeout(timer);
    }
  }, [snapshot.state, settings.companion.collapseSeconds]);
  useEffect(() => {
    if (
      snapshot.pinned ||
      !snapshot.alertId ||
      !['completed', 'error', 'paused', 'notification'].includes(snapshot.state)
    )
      return;
    const id = snapshot.alertId;
    const timer = window.setTimeout(
      () => {
        void invoke('dismiss_pet_alert', { alertId: id }).catch(() =>
          console.warn(
            '[OdeKrai diagnóstico] Falha no comando dismiss_pet_alert da janela do pet.',
          ),
        );
      },
      snapshot.state === 'notification' ? 5000 : 2400,
    );
    return () => window.clearTimeout(timer);
  }, [snapshot.alertId, snapshot.pinned, snapshot.state]);
  const send = async (event: IslandEvent, sound?: Parameters<typeof playSound>[0]) => {
    setSnapshot((current) => transitionIsland(current, event));
    try {
      if (sound) await playSound(sound, normalizeSoundPreferences(settings.companion.sounds));
      await emit('companion-state', event);
    } catch {
      console.error(
        `[OdeKrai diagnóstico] Falha ao emitir companion-state (${event.type}) na janela pet.`,
      );
    }
  };
  const requestQuickFocus = async () => {
    if (focusEnabled.current) {
      quickInput.current?.focus();
      return;
    }
    focusEnabled.current = islandKeyboardFocus(focusEnabled.current, 'request');
    setQuickError('');
    try {
      await invoke('set_pet_keyboard_focus', { focused: true });
      quickInput.current?.focus();
    } catch {
      focusEnabled.current = false;
      setQuickError('O Windows não permitiu focar o campo rápido. Clique novamente para tentar.');
    }
  };
  const releaseQuickFocus = async (event: 'send' | 'escape' | 'blur') => {
    focusEnabled.current = islandKeyboardFocus(focusEnabled.current, event);
    if (event !== 'blur') quickInput.current?.blur();
    try {
      await invoke('set_pet_keyboard_focus', { focused: false });
    } catch {
      setQuickError('Não foi possível devolver a ilha ao modo sem foco.');
    }
  };
  useEffect(() => {
    let disposed = false;
    const focusIfRequested = async () => {
      try {
        const requested = await invoke<boolean>('take_pet_quick_focus_request');
        if (!requested || disposed) return;
        await send({ type: 'open' });
        window.requestAnimationFrame(() => {
          window.requestAnimationFrame(() => void requestQuickFocus());
        });
      } catch {
        if (!disposed) setQuickError('Não foi possível abrir o campo de comando rápido.');
      }
    };
    let stopFocus: (() => void) | undefined;
    let stopError: (() => void) | undefined;
    void listen('pet-quick-focus-request', () => void focusIfRequested())
      .then((stop) => {
        stopFocus = stop;
        void focusIfRequested();
      })
      .catch(() => setQuickError('Não foi possível acompanhar o atalho do comando rápido.'));
    void listen<string>('pet-quick-focus-error', ({ payload }) => setQuickError(payload))
      .then((stop) => {
        stopError = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      stopFocus?.();
      stopError?.();
    };
  }, []);
  const submitQuickCommand = async (event: FormEvent) => {
    event.preventDefault();
    if (!canSubmitIslandQuickCommand(quickDraft, quickBusy)) return;
    const request: IslandQuickRequest | null = createIslandQuickRequest(quickDraft);
    if (!request) return;
    quickRequestId.current = request.requestId;
    setQuickBusy(true);
    setQuickResult(null);
    setQuickError('');
    await releaseQuickFocus('send');
    try {
      await emit('island-quick-command', request);
    } catch {
      setQuickBusy(false);
      setQuickError('Não foi possível enviar a mensagem ao OdeKrai.');
    }
  };
  const state = snapshot.state;
  return (
    <main
      className={`floating-island ${settings.companion.freeMode ? 'free-mode' : 'island-mode'} state-${state} ${settings.reduceMotion ? 'reduce-motion' : ''}`}
    >
      <div
        className="island-shell"
        onMouseEnter={() => {
          void playSound('hover', normalizeSoundPreferences(settings.companion.sounds));
          if (settings.companion.openOnHover && state !== 'expanded') {
            void send({ type: 'open' }, 'island_open');
          }
        }}
      >
        <div className="island-drag-handle" data-tauri-drag-region aria-hidden="true" />
        <button
          className="island-mascot"
          data-tauri-drag-region={settings.companion.freeMode ? '' : undefined}
          aria-label="Abrir o assistente do OdeKrai"
          onClick={() => {
            const preferences = normalizeSoundPreferences(settings.companion.sounds);
            void playSound('click', preferences, {
              petVisible: true,
            });
            if (settings.companion.freeMode) {
              void playSound('happy', preferences, { petVisible: true });
            }
            void send({ type: state === 'expanded' ? 'close' : 'open' });
          }}
        >
          <PetSprite
            className="island-mascot-renderer"
            estado={petAnimationForIslandState(state)}
            reduceMotion={settings.reduceMotion}
          />
        </button>
        {!settings.companion.freeMode && (
          <span className="island-copy">
            <strong>{petGreeting(selectedPetId)}</strong>
            <span>{snapshot.message || islandLabels[state]}</span>
            {(snapshot.alertCount ?? 0) > 1 && <small>{snapshot.alertCount} alertas na fila</small>}
          </span>
        )}
        {!settings.companion.freeMode && (
          <button
            className="island-action"
            aria-label={
              state === 'expanded' ? 'Recolher pet do OdeKrai' : 'Expandir pet do OdeKrai'
            }
            onClick={() =>
              void send(
                { type: state === 'expanded' ? 'close' : 'open' },
                state === 'expanded' ? 'island_close' : 'island_open',
              )
            }
          >
            <Icon name={state === 'expanded' ? 'minus' : 'plus'} />
          </button>
        )}
        {state === 'expanded' && (
          <div
            className="island-expanded"
            onKeyDown={(event) => {
              if (event.key === 'Escape' && focusEnabled.current) {
                event.preventDefault();
                void releaseQuickFocus('escape');
              }
            }}
          >
            <form
              className="island-quick-command"
              onSubmit={(event) => void submitQuickCommand(event)}
            >
              <input
                ref={quickInput}
                aria-label="Comando rápido"
                placeholder="O que você precisa?"
                value={quickDraft}
                disabled={quickBusy}
                onPointerDown={() => void requestQuickFocus()}
              />
              <button type="submit" disabled={quickBusy || !quickDraft.trim()}>
                {quickBusy ? 'Enviando…' : 'Enviar'}
              </button>
            </form>
            {quickError && (
              <p className="island-quick-error" role="status">
                {quickError}
              </p>
            )}
            {quickSuggestions.length > 0 && (
              <div className="island-quick-suggestions" aria-label="Sugestões reais">
                {quickSuggestions.map((suggestion) => (
                  <button
                    type="button"
                    key={suggestion.id}
                    onClick={() => {
                      setQuickDraft(suggestion.prompt);
                      void requestQuickFocus();
                    }}
                  >
                    {suggestion.label}
                  </button>
                ))}
              </div>
            )}
            {quickResult && (
              <div className="island-quick-result" role="status">
                <strong>
                  {quickResult.success ? quickResult.title : 'A resposta não ficou pronta'}
                </strong>
                <span>{quickResult.summary}</span>
                {quickResult.success && (
                  <button
                    type="button"
                    onClick={() => void emit('pet-open-conversation', quickResult.conversationId)}
                  >
                    Abrir no OdeKrai
                  </button>
                )}
              </div>
            )}
            {queuedAlerts.length > 0 && (
              <ul className="island-alerts" aria-label="Alertas da ilha">
                {queuedAlerts.map((alert) => (
                  <li key={alert.alertId}>
                    <span>{alert.message}</span>
                    <button
                      type="button"
                      onClick={() => void emit('pet-review-alert', alert.state)}
                    >
                      Revisar
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <button
              className="island-action-open"
              onClick={() => {
                void playSound('click', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                void emit('pet-action', 'open').catch(() =>
                  console.error('[OdeKrai diagnóstico] Falha ao emitir pet-action/open.'),
                );
              }}
            >
              <Icon name="home" />
              <span>Abrir o OdeKrai</span>
            </button>
            <button
              className="island-action-chat"
              onClick={() => {
                void playSound('click', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                void emit('pet-action', 'new-chat').catch(() =>
                  console.error('[OdeKrai diagnóstico] Falha ao emitir pet-action/new-chat.'),
                );
              }}
            >
              <Icon name="chat" />
              <span>Nova conversa</span>
            </button>
            <button
              className="island-action-stop"
              onClick={() => {
                void playSound('click', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                void emit('pet-action', 'stop').catch(() =>
                  console.error('[OdeKrai diagnóstico] Falha ao emitir pet-action/stop.'),
                );
              }}
            >
              <Icon name="stop" />
              <span>Parar tudo</span>
            </button>
            <button
              className="island-action-mute"
              onClick={() => {
                void playSound('click', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                void emit('pet-action', 'mute').catch(() =>
                  console.error('[OdeKrai diagnóstico] Falha ao emitir pet-action/mute.'),
                );
              }}
            >
              <Icon name="mute" />
              <span>Silenciar</span>
            </button>
            <button
              className="island-action-hide"
              onClick={async () => {
                const current = loadSettings();
                void playSound('click', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                await playSound('sleep', normalizeSoundPreferences(settings.companion.sounds), {
                  petVisible: true,
                });
                saveSettings({ ...current, companion: { ...current.companion, enabled: false } });
                await send({ type: 'hide' });
                await invoke('set_pet_visible', { visible: false });
                await emit('pet-visibility-changed', false);
              }}
            >
              <Icon name="close" />
              <span>Ocultar</span>
            </button>
          </div>
        )}
      </div>
    </main>
  );
}
