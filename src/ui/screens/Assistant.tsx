import { useEffect, useState } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke, isTauri } from '@tauri-apps/api/core';
import {
  normalizeSoundPreferences,
  playSound,
  SOUND_EVENT_LABELS,
  SOUND_EVENT_GROUPS,
  SOUND_MANIFEST,
  soundSources,
} from '../../core/sounds';
import { PETS, PET_ANIMATION_LABELS, type PetAnimationState } from '../../core/pets';
import type { AppSettings } from '../../core/settings';
import { PageHeader, Toggle } from '../DesignSystem';
import { PetSprite } from '../PetSprite';

export function PetActionError({ message, onRetry }: { message: string; onRetry: () => void }) {
  if (!message) return null;
  return (
    <div className="diagnostic-banner" role="alert">
      <span>{message}</span>
      <button type="button" onClick={onRetry}>
        Tentar de novo
      </button>
    </div>
  );
}

function QuickShortcutSettings() {
  const [shortcut, setShortcut] = useState('Ctrl+Alt+Space');
  const [enabled, setEnabled] = useState(false);
  const [error, setError] = useState('');
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setLoaded(true);
      return;
    }
    let disposed = false;
    const refresh = () =>
      invoke<{ shortcut: string; enabled: boolean; error?: string | null }>(
        'get_island_quick_shortcut',
      )
        .then((value) => {
          if (disposed) return;
          setShortcut(value.shortcut);
          setEnabled(value.enabled);
          setError(value.error ?? '');
        })
        .catch(() => {
          if (!disposed) setError('Não foi possível carregar o atalho salvo.');
        })
        .finally(() => {
          if (!disposed) setLoaded(true);
        });
    let unlisten: (() => void) | undefined;
    void listen('island-quick-shortcut-updated', () => void refresh())
      .then((stop) => {
        unlisten = stop;
        void refresh();
      })
      .catch(() => void refresh());
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function save(nextEnabled = enabled) {
    setError('');
    if (!isTauri()) {
      setEnabled(nextEnabled);
      return;
    }
    try {
      await invoke('set_island_quick_shortcut', { shortcut, enabled: nextEnabled });
      setEnabled(nextEnabled);
    } catch (cause) {
      const current = await invoke<{ enabled: boolean }>('get_island_quick_shortcut').catch(
        () => null,
      );
      setEnabled(current?.enabled ?? false);
      setError(typeof cause === 'string' ? cause : 'Não foi possível registrar esta combinação.');
    }
  }

  return (
    <section id="assistant-quick-shortcut" className="settings-panel">
      <div className="panel-heading">
        <div>
          <h3>Atalho para o comando rápido</h3>
          <p>
            O atalho apenas abre a ilha e foca o campo. Ele não envia mensagens nem aprova ações.
          </p>
        </div>
      </div>
      <label className="settings-field">
        <span>Combinação</span>
        <input
          aria-label="Combinação do atalho para o comando rápido"
          value={shortcut}
          onChange={(event) => setShortcut(event.target.value)}
          placeholder="Ctrl+Alt+Espaço"
          autoComplete="off"
          disabled={!loaded}
        />
      </label>
      <label className="settings-toggle-row">
        <input
          type="checkbox"
          aria-label="Atalho para o comando rápido"
          checked={enabled}
          disabled={!loaded}
          onChange={(event) => void save(event.target.checked)}
        />
        <span>Atalho global (desligado por padrão)</span>
      </label>
      <button type="button" onClick={() => void save()} disabled={!loaded}>
        Salvar atalho
      </button>
      {error && (
        <p className="diagnostic-banner" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

export function AssistantScreen({
  settings,
  update,
  petVisible,
  onPetVisibilityChange,
  onError,
  selectedPet,
  onSelectPet,
  embedded = false,
}: {
  settings: AppSettings;
  update: (settings: AppSettings) => void;
  petVisible: boolean;
  onPetVisibilityChange: (visible: boolean) => void;
  onError: (message: string) => void;
  selectedPet: string;
  onSelectPet: (petId: string) => void;
  embedded?: boolean;
}) {
  const [petState, setPetState] = useState<PetAnimationState>('idle');
  const [petError, setPetError] = useState('');
  const [retryPetAction, setRetryPetAction] = useState<(() => void) | null>(null);
  async function applyPosition(next: AppSettings['companion']) {
    if (!isTauri() || !next.enabled) return;
    const width = (next.freeMode ? 176 : 280) * next.size;
    const height = (next.freeMode ? 176 : 150) * next.size;
    const right = next.position.endsWith('right');
    const bottom = next.position.startsWith('bottom');
    const x = next.coordinates?.x ?? (right ? Math.max(0, screen.availWidth - width - 28) : 28);
    const y = next.coordinates?.y ?? (bottom ? Math.max(0, screen.availHeight - height - 58) : 28);
    try {
      await invoke('set_pet_position', { x, y, size: next.size });
      setPetError('');
      setRetryPetAction(null);
      await invoke('set_pet_size', { width, height });
      await invoke('set_pet_position', { x, y, size: next.size });
    } catch {
      const message = 'Não foi possível atualizar a posição ou o tamanho do pet.';
      setPetError(message);
      setRetryPetAction(() => () => void applyPosition(next));
      throw new Error(message);
    }
  }
  async function setEnabled(enabled: boolean) {
    update({ ...settings, companion: { ...settings.companion, enabled } });
    if (!isTauri()) return;
    try {
      if (enabled) {
        await applyPosition({ ...settings.companion, enabled: true });
        await invoke('set_pet_topmost', { enabled: settings.companion.alwaysOnTop });
        await invoke('set_pet_visible', { visible: true });
        onPetVisibilityChange(true);
        await emit('companion-state', { type: 'reveal' });
        setPetError('');
        setRetryPetAction(null);
        void playSound('wake', normalizeSoundPreferences(settings.companion.sounds), {
          petVisible: true,
        });
      } else {
        await playSound('sleep', normalizeSoundPreferences(settings.companion.sounds), {
          petVisible: true,
        });
        await invoke('set_pet_visible', { visible: false });
        onPetVisibilityChange(false);
      }
    } catch {
      update({ ...settings, companion: { ...settings.companion, enabled: !enabled } });
      setPetError('Não foi possível alterar a janela do pet.');
      setRetryPetAction(() => () => void setEnabled(enabled));
      onError('Não foi possível alterar a janela do assistente.');
    }
  }
  useEffect(() => {
    if (!isTauri()) return;
    void invoke<'island' | 'free'>('get_pet_mode')
      .then((mode) => {
        update({ ...settings, companion: { ...settings.companion, freeMode: mode === 'free' } });
      })
      .catch(() => {
        setPetError('Não foi possível carregar o modo salvo do pet.');
        setRetryPetAction(
          () => () =>
            void invoke<'island' | 'free'>('get_pet_mode')
              .then((mode) =>
                update({
                  ...settings,
                  companion: { ...settings.companion, freeMode: mode === 'free' },
                }),
              )
              .catch(() => onError('Não foi possível carregar o modo salvo do pet.')),
        );
        onError('Não foi possível carregar o modo salvo do pet.');
      });
  }, []);
  async function setFreeMode(freeMode: boolean) {
    try {
      if (isTauri()) await invoke('set_pet_mode', { mode: freeMode ? 'free' : 'island' });
      const companion = { ...settings.companion, freeMode };
      update({ ...settings, companion });
      if (companion.enabled) {
        const x = companion.coordinates?.x ?? Math.max(0, screen.availWidth - 300 * companion.size);
        const y =
          companion.coordinates?.y ?? Math.max(0, screen.availHeight - 190 * companion.size);
        await invoke('set_pet_position', { x, y, size: companion.size });
        await invoke('set_pet_size', {
          width: (freeMode ? 176 : 280) * companion.size,
          height: (freeMode ? 176 : 150) * companion.size,
        });
        await invoke('set_pet_position', { x, y, size: companion.size });
      }
      setPetError('');
      setRetryPetAction(null);
    } catch {
      setPetError('Não foi possível trocar o modo do pet.');
      setRetryPetAction(() => () => void setFreeMode(freeMode));
      onError('Não foi possível trocar o modo do pet.');
    }
  }
  return (
    <div className={embedded ? 'settings-embedded-content' : 'page subpage'}>
      {!embedded && <QuickShortcutSettings />}
      <PetActionError message={petError} onRetry={retryPetAction ?? (() => undefined)} />
      {!embedded && (
        <PageHeader
          eyebrow="PERSONALIZAÇÃO"
          title="Assistente do OdeKrai"
          subtitle="Uma presença leve para acompanhar você na área de trabalho."
        />
      )}
      <section id="assistant-pet" className="settings-panel pet-picker-panel">
        <div className="panel-heading">
          <div>
            <h3>Escolha seu mascote</h3>
            <p>
              {'Escolha qual mascote aparece na área de trabalho, nas conversas e na tela inicial.'}
            </p>
          </div>
        </div>
        <div className="pet-picker-grid" role="group" aria-label="Mascote do OdeKrai">
          {PETS.map((pet) => (
            <button
              type="button"
              aria-pressed={selectedPet === pet.id}
              className={`pet-choice ${selectedPet === pet.id ? 'selected' : ''}`}
              key={pet.id}
              onClick={() => onSelectPet(pet.id)}
            >
              <PetSprite
                estado="idle"
                petId={pet.id}
                reduceMotion={settings.reduceMotion}
                staticImage
              />
              <span>{pet.name}</span>
              {selectedPet === pet.id && <small>Selecionado</small>}
            </button>
          ))}
        </div>
        <label className="pet-state-select">
          <span>Prévia de estado</span>
          <select
            value={petState}
            onChange={(event) => setPetState(event.target.value as PetAnimationState)}
          >
            <option value="idle">Em espera</option>
            <option value="thinking">Pensando</option>
            <option value="working">Trabalhando</option>
            <option value="awaiting_approval">Aguardando aprovação</option>
            <option value="success">Concluído</option>
            <option value="error">Erro</option>
            <option value="paused">Pausado</option>
          </select>
        </label>
      </section>
      <div className="companion-preview">
        <div className="preview-grid" />
        <div className="preview-island">
          <span className="island-light" />
          <PetSprite
            className="preview-mascot"
            estado={petState}
            petId={selectedPet}
            reduceMotion={settings.reduceMotion}
          />
          <span className="island-label">
            PET OdeKrai <i />
          </span>
        </div>
        <div className="preview-caption">
          <span className="eyebrow">PET DO OdeKrai</span>
          <strong>{PET_ANIMATION_LABELS[petState]}</strong>
          <p>OdeKrai pronto para acompanhar sua área de trabalho.</p>
        </div>
      </div>
      <section id="assistant-behavior" className="settings-panel compact-panel">
        <div className="panel-heading">
          <div>
            <h3>Pet na área de trabalho</h3>
            <p>
              Abre uma janela transparente do assistente. A posição pode ficar em qualquer canto da
              tela principal.
            </p>
          </div>
          <Toggle
            checked={settings.companion.enabled}
            label="Ativar pet do assistente"
            onClick={() => void setEnabled(!settings.companion.enabled)}
            disabled={!isTauri()}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Posição</strong>
            <span>Arraste a janela para personalizar; a posição é salva.</span>
          </div>
          <select
            value={settings.companion.position}
            onChange={(event) => {
              const companion = {
                ...settings.companion,
                position: event.target.value as AppSettings['companion']['position'],
                coordinates: null,
              };
              update({ ...settings, companion });
              void applyPosition(companion).catch(() =>
                onError('Não foi possível atualizar a posição do pet.'),
              );
            }}
          >
            <option value="bottom-right">Inferior direita</option>
            <option value="bottom-left">Inferior esquerda</option>
            <option value="top-right">Superior direita</option>
            <option value="top-left">Superior esquerda</option>
          </select>
        </div>
        <div className="preference-row">
          <div>
            <strong>Modo do pet</strong>
            <span>Escolha entre uma ilha junto à borda e o mascote livre.</span>
          </div>
          <select
            aria-label="Modo do pet"
            value={settings.companion.freeMode ? 'free' : 'island'}
            onChange={(event) => void setFreeMode(event.target.value === 'free')}
            disabled={!isTauri()}
          >
            <option value="island">Ilha</option>
            <option value="free">Livre</option>
          </select>
        </div>
        <div className="preference-row">
          <div>
            <strong>Tamanho</strong>
            <span>{Math.round(settings.companion.size * 100)}% da escala-base.</span>
          </div>
          <input
            aria-label="Tamanho do pet"
            type="range"
            min="0.75"
            max="1.5"
            step="0.05"
            value={settings.companion.size}
            onChange={(event) => {
              const companion = { ...settings.companion, size: Number(event.target.value) };
              update({ ...settings, companion });
              void applyPosition(companion).catch(() =>
                onError('Não foi possível atualizar o tamanho do pet.'),
              );
            }}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Manter sobre outras janelas</strong>
            <span>Define a prioridade de exibição do pet.</span>
          </div>
          <Toggle
            checked={settings.companion.alwaysOnTop}
            label="Manter o pet sobre outras janelas"
            onClick={() => {
              const enabled = !settings.companion.alwaysOnTop;
              update({ ...settings, companion: { ...settings.companion, alwaysOnTop: enabled } });
              if (isTauri())
                void invoke('set_pet_topmost', { enabled }).catch(() =>
                  onError('Não foi possível atualizar a prioridade da janela do pet.'),
                );
            }}
            disabled={!isTauri()}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Abrir ao passar o mouse</strong>
            <span>Expande o pet quando o ponteiro passa sobre ele.</span>
          </div>
          <Toggle
            checked={settings.companion.openOnHover}
            label="Expandir o pet ao passar o mouse"
            onClick={() =>
              update({
                ...settings,
                companion: { ...settings.companion, openOnHover: !settings.companion.openOnHover },
              })
            }
            disabled={!isTauri()}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Recolher após</strong>
            <span>Tempo sem interação enquanto expandida.</span>
          </div>
          <select
            value={settings.companion.collapseSeconds}
            onChange={(event) =>
              update({
                ...settings,
                companion: { ...settings.companion, collapseSeconds: Number(event.target.value) },
              })
            }
            disabled={!isTauri()}
          >
            <option value={8}>8 segundos</option>
            <option value={15}>15 segundos</option>
            <option value={30}>30 segundos</option>
            <option value={60}>1 minuto</option>
          </select>
        </div>
        <div className="preference-row">
          <div>
            <strong>Efeitos sonoros</strong>
            <span>Desative esta opção para manter o pet em silêncio.</span>
          </div>
          <Toggle
            checked={settings.companion.sounds.enabled}
            label="Ativar sons do pet"
            onClick={() =>
              update({
                ...settings,
                companion: {
                  ...settings.companion,
                  sounds: {
                    ...settings.companion.sounds,
                    enabled: !settings.companion.sounds.enabled,
                  },
                },
              })
            }
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Volume</strong>
            <span>{Math.round(settings.companion.sounds.volume * 100)}%</span>
          </div>
          <input
            aria-label="Volume dos efeitos sonoros"
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={settings.companion.sounds.volume}
            onChange={(event) =>
              update({
                ...settings,
                companion: {
                  ...settings.companion,
                  sounds: { ...settings.companion.sounds, volume: Number(event.target.value) },
                },
              })
            }
          />
        </div>
        <div className="sound-events-list">
          {SOUND_EVENT_GROUPS.map(({ label, events }) => (
            <section className="sound-event-group" key={label}>
              <h4>{label}</h4>
              {events.map((event) => {
                const replacement = settings.companion.sounds.replacements[event];
                const currentFile = soundSources(event, replacement)[0];
                return (
                  <div className="sound-event-row" key={event}>
                    <div className="sound-event-description">
                      <strong>{SOUND_EVENT_LABELS[event]}</strong>
                      <span>{currentFile ? currentFile.split('/').at(-1) : 'Sem arquivo'}</span>
                    </div>
                    <div className="sound-event-controls">
                      <select
                        aria-label={`Arquivo para ${SOUND_EVENT_LABELS[event]}`}
                        value={replacement ?? ''}
                        onChange={(change) => {
                          const replacements = { ...settings.companion.sounds.replacements };
                          if (change.target.value) replacements[event] = change.target.value;
                          else delete replacements[event];
                          update({
                            ...settings,
                            companion: {
                              ...settings.companion,
                              sounds: { ...settings.companion.sounds, replacements },
                            },
                          });
                        }}
                      >
                        <option value="">Automático (formato disponível)</option>
                        {SOUND_MANIFEST.map((file) => (
                          <option key={file} value={file}>
                            {file.split('/').at(-1)}
                          </option>
                        ))}
                      </select>
                      <button
                        className="ui-button ui-button--secondary"
                        type="button"
                        disabled={!currentFile || (label === 'Pet' && !petVisible)}
                        onClick={() =>
                          void playSound(
                            event,
                            normalizeSoundPreferences(settings.companion.sounds),
                            { petVisible },
                          )
                        }
                      >
                        Tocar prévia
                      </button>
                    </div>
                  </div>
                );
              })}
            </section>
          ))}
        </div>
      </section>
    </div>
  );
}
