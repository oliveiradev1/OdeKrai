import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ComponentType,
  type FormEvent,
} from 'react';
import { resolveConversationSelection } from '../core/chat';
import { setActionCardUndoError } from '../core/agent-card';
import type { ChatMessage } from '../core/provider-messages';
import type { AgentActionCardData } from './AgentControls';
import {
  ActivityPanelTrigger,
  ActivitySidePanel,
  shouldLoadPanelAudit,
  toggleActivityPanel,
  type ActivityAuditSummary,
  type UpcomingReminder,
} from './ActivitySidePanel';
import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import {
  normalizeSoundPreferences,
  playSound,
  SOUND_EVENTS,
  SoundTriggerGate,
  type SoundEvent,
} from '../core/sounds';
import { AI_PROVIDERS, type AiProvider } from '../core/providers';
import { stopLocalSpeech } from '../core/speech';
import {
  createHomeChecklist,
  createHomeSuggestions,
  getTodayBounds,
  type HomeEvent,
} from '../core/home';
import { loadFirstUseProgress, shouldAutoOpenFirstUse } from '../core/first-use';
import { usePerformanceMode } from './usePerformanceMode';
import { decideReleasePrompt, RELEASE_NOTES_LAST_SEEN_KEY } from '../core/release-notes';
import { releaseNotesManifest } from '../core/release-notes-manifest';
import {
  islandReviewDestination,
  islandSuggestions,
  summarizeIslandResponse,
  type IslandQuickRequest,
  type IslandReviewState,
} from '../core/island-quick';
import type { CatalogSkill } from '../core/skills-catalog';
const Overview = lazy(() =>
  import('./screens/Overview').then((module) => ({ default: withScreenBoundary(module.Overview) })),
);
const DevDesignSystemGallery = import.meta.env.DEV
  ? lazy(() =>
      import('./DesignSystemGallery').then((module) => ({ default: module.DesignSystemGallery })),
    )
  : null;
const AssistantScreen = lazy(() =>
  import('./screens/Assistant').then((module) => ({
    default: withScreenBoundary(module.AssistantScreen),
  })),
);
const SettingsScreen = lazy(() =>
  import('./screens/Settings').then((module) => ({
    default: withScreenBoundary(module.SettingsScreen),
  })),
);
const ConversationsScreen = lazy(() =>
  import('./screens/Conversations').then((module) => ({
    default: withScreenBoundary(module.ConversationsScreen),
  })),
);
const ProvidersScreen = lazy(() =>
  import('./screens/Providers').then((module) => ({
    default: withScreenBoundary(module.ProvidersScreen),
  })),
);
const ComputerScreen = lazy(() =>
  import('./screens/Computer').then((module) => ({
    default: withScreenBoundary(module.ComputerScreen),
  })),
);
const CalendarScreen = lazy(() =>
  import('./screens/Calendar').then((module) => ({
    default: withScreenBoundary(module.CalendarScreen),
  })),
);
const IdentityScreen = lazy(() =>
  import('./screens/Identity').then((module) => ({
    default: withScreenBoundary(module.IdentityScreen),
  })),
);
const PermissionsScreen = lazy(() =>
  import('./screens/Permissions').then((module) => ({
    default: withScreenBoundary(module.PermissionsScreen),
  })),
);
const ActivityScreen = lazy(() =>
  import('./screens/Activity').then((module) => ({
    default: withScreenBoundary(module.ActivityScreen),
  })),
);
import { ApprovalWindow, isApprovalWindow, SecurityCenter } from './SecurityCenter';
import type { AuditEntry } from './SecurityCenter';
import { SkillsScreen } from './SkillsScreen';
import { PETS, petAnimationForAssistantActivity, type PetAnimationState } from '../core/pets';
import {
  navigationItems,
  SidebarNavigation,
  SidebarProfileMenu,
  shouldShowStopAll,
} from './SidebarNavigation';
import {
  loadSavedCredentialProviderIds,
  setPrimaryProviderSettings,
  toggleProviderSettings,
} from '../core/provider-directory';
import { loadSettings, saveSettings, type ProviderSettings } from '../core/settings';
import { useAppEffects } from './hooks/useAppEffects';
import { externalLinkDomain, safeMarkdownUrl } from '../core/markdown';
import { shouldAutomaticallyCheckForUpdates } from '../core/update-check';

import { useAppState } from './hooks/useAppState';
import { useNavigation } from './hooks/useNavigation';
import { SettingsModal } from './SettingsModal';
import { UpdateAvailableBanner, type UpdateCheckResult } from './UpdateAvailableBanner';
import { FirstUseAssistant, type FirstUseStepContent } from './FirstUseAssistant';
import { ReleaseNotesModal } from './ReleaseNotesModal';
import { useConfirmation } from './ConfirmationProvider';
import { resolveAgentReason, resolveConfigurationReason } from '../core/navigation';
import { ScreenErrorBoundary } from './ScreenErrorBoundary';
import { ResizeHandle } from './ResizeHandle';
import { CommandPalette } from './CommandPalette';
import {
  buildPaletteItems,
  dispatchPaletteAction,
  updateRecentCommands,
  type PaletteItem,
} from '../core/command-palette';
import type { SkillFamily } from '../core/skills-catalog';
const screenLoading = (
  <div className="screen-loading" role="status">
    Carregando tela…
  </div>
);
const withScreenBoundary = <Props extends object>(
  Screen: ComponentType<Props>,
): ComponentType<Props> => {
  function BoundedScreen(props: Props) {
    return (
      <ScreenErrorBoundary>
        <Screen {...props} />
      </ScreenErrorBoundary>
    );
  }
  return BoundedScreen;
};
const currentReleaseNotesUrl = `https://github.com/Anceryx/OdeKrai/blob/main/docs/releases/v${releaseNotesManifest.currentVersion}.md`;

export function App() {
  if (isApprovalWindow()) return <ApprovalWindow />;
  if (
    import.meta.env.DEV &&
    location.pathname === '/__dev/design-system' &&
    DevDesignSystemGallery
  ) {
    return (
      <Suspense fallback={screenLoading}>
        <DevDesignSystemGallery />
      </Suspense>
    );
  }
  return <MainApp />;
}

function MainApp() {
  const confirm = useConfirmation();
  const [hadSettingsAtStartup] = useState(() => {
    try {
      return Boolean(
        localStorage.getItem('odekrai.settings.v2') || localStorage.getItem('odekrai.settings.v1'),
      );
    } catch {
      return false;
    }
  });
  const appState = useAppState();
  const {
    section,
    route,
    navigate,
    openLegacyScreen,
    settingsOpen,
    closeSettings,
    selectSettingsSection,
  } = useNavigation();
  const [usedProviderId, setUsedProviderId] = useState<string | null>(null);
  const [agentAvailability, setAgentAvailability] = useState<{
    available: boolean;
    reason: string | null;
    workspacePath: string | null;
  }>({ available: false, reason: 'Verificando requisitos do agente…', workspacePath: null });
  const [agentEnabled, setAgentEnabled] = useState(false);
  const [agentRunning, setAgentRunning] = useState(false);
  const [agentErrorDetails, setAgentErrorDetails] = useState('');
  const [agentAwaitingApproval, setAgentAwaitingApproval] = useState(false);
  const [agentActionCards, setAgentActionCards] = useState<AgentActionCardData[]>([]);
  const [assistantPetState, setAssistantPetState] = useState<PetAnimationState>('idle');
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [recentCommands, setRecentCommands] = useState<string[]>([]);
  const [requestedSkillFamily, setRequestedSkillFamily] = useState<{
    family: SkillFamily;
    requestId: number;
  } | null>(null);
  const skillFamilyRequestId = useRef(0);
  const quickRequestHandler = useRef<(request: IslandQuickRequest) => void>(() => undefined);
  const openConversationHandler = useRef<(id: string) => void>(() => undefined);
  const reviewAlertHandler = useRef<(state: IslandReviewState) => void>(() => undefined);
  const quickSuggestionsRef = useRef<ReturnType<typeof islandSuggestions>>([]);
  const activeConversationRef = useRef<string | null>(null);
  const [requestedSettingsEntry, setRequestedSettingsEntry] = useState<{
    section: import('../core/navigation').SettingsSection;
    anchor: string;
    requestId: number;
  } | null>(null);
  const settingsEntryRequestId = useRef(0);
  const [undoToast, setUndoToast] = useState<{ cardId: string } | null>(null);
  const [firstUseOpen, setFirstUseOpen] = useState(false);
  const [releaseNotesPending, setReleaseNotesPending] = useState(false);
  const [releaseNotesOpen, setReleaseNotesOpen] = useState(false);
  const [showAllReleaseNotes, setShowAllReleaseNotes] = useState(false);
  const [availableUpdate, setAvailableUpdate] = useState<UpdateCheckResult | null>(null);
  useEffect(() => {
    if (!undoToast) return;
    const timer = window.setTimeout(() => setUndoToast(null), 8000);
    return () => window.clearTimeout(timer);
  }, [undoToast]);
  const [pendingApprovalCount, setPendingApprovalCount] = useState(0);
  const [activityPanelOpen, setActivityPanelOpen] = useState(false);
  const activityPanelTriggerRef = useRef<HTMLButtonElement>(null);
  const [activityAuditRecords, setActivityAuditRecords] = useState<AuditEntry[]>([]);
  const [activityAuditVerified, setActivityAuditVerified] = useState<boolean | null>(null);
  const [activityAuditLoading, setActivityAuditLoading] = useState(false);
  const [activityAuditUnavailable, setActivityAuditUnavailable] = useState(false);
  const activityAuditLoadedAt = useRef(0);
  const [homeSkills, setHomeSkills] = useState<CatalogSkill[] | null>(null);
  const [homeSkillsLoadError, setHomeSkillsLoadError] = useState('');
  const [homeContextRetry, setHomeContextRetry] = useState(0);
  const [todayEvents, setTodayEvents] = useState<HomeEvent[]>([]);
  const [scheduledEvents, setScheduledEvents] = useState<HomeEvent[]>([]);
  const [reminderCount, setReminderCount] = useState(0);
  const [upcomingReminders, setUpcomingReminders] = useState<UpcomingReminder[]>([]);
  const [upcomingRemindersLoading, setUpcomingRemindersLoading] = useState(false);
  const [upcomingRemindersUnavailable, setUpcomingRemindersUnavailable] = useState(false);
  const [scheduleUnavailable, setScheduleUnavailable] = useState(false);
  const [reminderUnavailable, setReminderUnavailable] = useState(false);
  const homeContextLoadedAt = useRef(0);
  const agentRunConversation = useRef<string | null>(null);
  const thinkingSoundGate = useRef(new SoundTriggerGate());
  const lastThinkingSoundKey = useRef<string | null>(null);
  const diagnosticsSoundPlayed = useRef(false);
  const {
    settings,
    setSettings,
    identityConfigured,
    setIdentityConfigured,
    petVisible,
    setPetVisible,
    permissionIndicator,
    profile,
    setProfile,
    keySavedForSession,
    setKeySavedForSession,
    selectedProviderId,
    setSelectedProviderId,
    activeProvider,
    messages,
    setMessages,
    selectedPet,
    setSelectedPet,
    conversations,
    setConversations,
    conversationId,
    setConversationId,
    draft,
    setDraft,
    busy,
    setBusy,
    error,
    setError,
    lastFailedPrompt,
    setLastFailedPrompt,
    toast,
    setToast,
    diagnostics,
    dismissDiagnostic,
    dismissDiagnostics,
    reportDiagnostic,
    chatMode,
    setChatMode,
    request,
    companionStateTimer,
  } = appState;
  const performanceStatus = usePerformanceMode(settings.performanceMode);
  useEffect(() => {
    if (!shouldAutomaticallyCheckForUpdates(settings.checkForUpdatesAutomatically, isTauri()))
      return;
    void invoke<UpdateCheckResult>('check_for_update')
      .then((result) => {
        setAvailableUpdate(result.check_succeeded && result.available_version ? result : null);
      })
      .catch(() => {
        // A consulta automática não deve interromper a experiência do app.
      });
  }, [settings.checkForUpdatesAutomatically]);
  const performanceStatusRef = useRef(performanceStatus);
  useEffect(() => {
    performanceStatusRef.current = performanceStatus;
  }, [performanceStatus.enabled, performanceStatus.loading, performanceStatus.reason]);
  useEffect(() => {
    if (!isTauri()) return;
    const storage = typeof localStorage === 'undefined' ? undefined : localStorage;
    const progress = loadFirstUseProgress(storage);
    if (!shouldAutoOpenFirstUse(progress)) return;
    setFirstUseOpen(true);
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    const current = releaseNotesManifest.releases.find(
      (release) => release.version === releaseNotesManifest.currentVersion,
    );
    if (!current) return;
    try {
      const previousVersion = localStorage.getItem(RELEASE_NOTES_LAST_SEEN_KEY);
      const decision = decideReleasePrompt({
        currentVersion: releaseNotesManifest.currentVersion,
        previousVersion,
        notesAvailable: true,
        hasExistingPreferences: hadSettingsAtStartup,
        enabled: settings.showReleaseNotes,
      });
      if (decision.markSeen && !decision.show) {
        localStorage.setItem(RELEASE_NOTES_LAST_SEEN_KEY, releaseNotesManifest.currentVersion);
      }
      if (decision.show) setReleaseNotesPending(true);
    } catch {
      // Sem armazenamento local não é possível garantir a exibição única por versão.
    }
  }, []);
  useEffect(() => {
    if (!releaseNotesPending || firstUseOpen) return;
    try {
      localStorage.setItem(RELEASE_NOTES_LAST_SEEN_KEY, releaseNotesManifest.currentVersion);
    } catch {
      setReleaseNotesPending(false);
      return;
    }
    setReleaseNotesPending(false);
    setShowAllReleaseNotes(false);
    setReleaseNotesOpen(true);
  }, [firstUseOpen, releaseNotesPending]);
  const emitCompanionState = (payload: { type: 'activity'; state: string; message: string }) => {
    setAssistantPetState(petAnimationForAssistantActivity(payload.state));
    void emit('companion-state', payload).catch(() =>
      reportDiagnostic('companion-state', 'Não foi possível atualizar o estado visual do pet.'),
    );
  };
  const usedProvider = usedProviderId
    ? AI_PROVIDERS.find((item) => item.id === usedProviderId)
    : undefined;
  const shownProviderSettings = usedProviderId ? settings.providers[usedProviderId] : undefined;
  const shownProviderName = usedProvider?.name ?? activeProvider.name;
  const shownProviderId = usedProviderId ?? settings.activeProviderId;
  const shownProviderModel = shownProviderSettings?.model ?? activeProvider.model;
  const activeHomeSkills = useMemo(
    () =>
      (homeSkills ?? [])
        .filter((skill) => skill.enabled && skill.status === 'installed')
        .map(({ id, family, name }) => ({ id, family, name })),
    [homeSkills],
  );
  const homeSuggestions = useMemo(
    () =>
      createHomeSuggestions({
        activeSkills: activeHomeSkills,
        agentAvailable: agentAvailability.available,
        todayEvents,
        reminderCount,
        pendingApprovalCount,
        recentConversations: conversations.slice(0, 5),
      }),
    [
      activeHomeSkills,
      agentAvailability.available,
      todayEvents,
      reminderCount,
      pendingApprovalCount,
      conversations,
    ],
  );
  quickSuggestionsRef.current = islandSuggestions(homeSuggestions);
  activeConversationRef.current = conversationId;
  useEffect(() => {
    if (isTauri()) void emit('pet-quick-suggestions', quickSuggestionsRef.current);
  }, [homeSuggestions]);
  const homeChecklist = useMemo(
    () =>
      createHomeChecklist({
        providerReady: keySavedForSession,
        workspaceAvailable: Boolean(agentAvailability.workspacePath),
        agentAvailable: agentAvailability.available,
        agentReason: agentAvailability.reason,
      }),
    [
      keySavedForSession,
      agentAvailability.available,
      agentAvailability.reason,
      agentAvailability.workspacePath,
    ],
  );
  useEffect(() => {
    if (isTauri())
      void invoke('set_microphone_opt_in', { enabled: settings.microphoneEnabled }).catch(() =>
        setToast('Não foi possível atualizar a permissão do microfone.'),
      );
  }, [settings.microphoneEnabled]);

  useEffect(() => {
    if (permissionIndicator?.halted) stopLocalSpeech();
  }, [permissionIndicator?.halted]);

  useEffect(() => {
    if (!settings.companion.sounds.enabled) stopLocalSpeech();
  }, [settings.companion.sounds.enabled]);

  useEffect(() => {
    if (!isTauri()) {
      setAgentAvailability({
        available: false,
        reason: 'O modo agente está disponível somente no aplicativo do OdeKrai.',
        workspacePath: null,
      });
      return;
    }
    const checkAvailability = () =>
      invoke<{ available: boolean; reason: string | null; workspacePath: string | null }>(
        'agent_availability',
        { providerId: activeProvider.providerId, model: activeProvider.model },
      )
        .then((availability) => {
          setAgentAvailability(availability);
          if (!availability.available) setAgentEnabled(false);
          else if (!conversationId) setAgentEnabled(true);
        })
        .catch(() =>
          setAgentAvailability({
            available: false,
            reason: 'Não foi possível verificar a pasta, o provedor e as habilidades do agente.',
            workspacePath: null,
          }),
        );
    void checkAvailability();
    let disposed = false;
    let stopSkillsUpdated: (() => void) | undefined;
    void listen('skills-updated', () => void checkAvailability())
      .then((stop) => {
        if (disposed) stop();
        else stopSkillsUpdated = stop;
      })
      .catch(() => setToast('Não foi possível acompanhar as alterações das habilidades.'));
    const onFocus = () => {
      void checkAvailability();
    };
    window.addEventListener('focus', onFocus);
    return () => {
      disposed = true;
      stopSkillsUpdated?.();
      window.removeEventListener('focus', onFocus);
    };
  }, [
    activeProvider.providerId,
    activeProvider.model,
    conversationId,
    permissionIndicator?.halted,
  ]);

  useEffect(() => {
    if (section !== 'overview' && !activityPanelOpen) return;
    let disposed = false;
    let stopSkillsUpdated: (() => void) | undefined;

    const refreshHomeContext = async (force = false) => {
      if (!force && Date.now() - homeContextLoadedAt.current < 30_000 && homeSkills !== null)
        return;
      setHomeSkills(null);
      setTodayEvents([]);
      setScheduledEvents([]);
      setReminderCount(0);
      setScheduleUnavailable(true);
      setReminderUnavailable(true);
      if (!isTauri()) {
        setHomeSkills([]);
        homeContextLoadedAt.current = Date.now();
        return;
      }
      try {
        const skills = await invoke<CatalogSkill[]>('get_skills');
        if (disposed) return;
        setHomeSkills(skills);
        setHomeSkillsLoadError('');
        const active = skills.filter((skill) => skill.enabled && skill.status === 'installed');
        const canReadCalendar = active.some(
          (skill) => skill.id === 'calendar.list_events' && skill.family === 'agenda_lembretes',
        );
        const canReadReminders = active.some(
          (skill) => skill.id === 'reminder.list' && skill.family === 'agenda_lembretes',
        );
        const now = new Date();
        const todayBounds = getTodayBounds(now);
        const next24Hours = new Date(now.getTime() + 24 * 60 * 60 * 1000);
        const bounds = {
          from: todayBounds.from,
          to: new Date(
            Math.max(new Date(todayBounds.to).getTime(), next24Hours.getTime()),
          ).toISOString(),
        };
        const [events, reminders] = await Promise.allSettled([
          canReadCalendar
            ? invoke<HomeEvent[]>('calendar_list_events', bounds)
            : Promise.resolve([] as HomeEvent[]),
          canReadReminders ? invoke<number>('get_home_reminder_count') : Promise.resolve(0),
        ]);
        if (disposed) return;
        const availableEvents = events.status === 'fulfilled' ? events.value : [];
        const todayStart = new Date(todayBounds.from).getTime();
        const todayEnd = new Date(todayBounds.to).getTime();
        setTodayEvents(
          availableEvents.filter((event) => {
            const startsAt = new Date(event.start).getTime();
            return startsAt >= todayStart && startsAt < todayEnd;
          }),
        );
        setScheduledEvents(
          availableEvents.filter((event) => {
            const startsAt = new Date(event.start).getTime();
            return startsAt >= now.getTime() && startsAt <= next24Hours.getTime();
          }),
        );
        setReminderCount(reminders.status === 'fulfilled' ? reminders.value : 0);
        setScheduleUnavailable(events.status === 'rejected' || !canReadCalendar);
        setReminderUnavailable(reminders.status === 'rejected' || !canReadReminders);
        homeContextLoadedAt.current = Date.now();
      } catch {
        if (!disposed) {
          setHomeSkills(null);
          setHomeSkillsLoadError(
            'Não foi possível carregar as habilidades para atualizar o Início.',
          );
        }
      }
    };

    void refreshHomeContext();
    void listen('skills-updated', () => void refreshHomeContext(true))
      .then((stop) => {
        if (disposed) stop();
        else stopSkillsUpdated = stop;
      })
      .catch(() => setToast('Não foi possível atualizar o contexto do Início.'));
    const refreshOnFocus = () => void refreshHomeContext(true);
    window.addEventListener('focus', refreshOnFocus);
    return () => {
      disposed = true;
      stopSkillsUpdated?.();
      window.removeEventListener('focus', refreshOnFocus);
    };
  }, [section, activityPanelOpen, homeContextRetry]);

  useEffect(() => {
    if (!activityPanelOpen || !isTauri() || homeSkills === null) {
      setUpcomingReminders([]);
      setUpcomingRemindersLoading(false);
      return;
    }
    const enabled = homeSkills.some(
      (skill) =>
        skill.id === 'reminder.list' &&
        skill.family === 'agenda_lembretes' &&
        skill.enabled &&
        skill.status === 'installed',
    );
    if (!enabled) {
      setUpcomingReminders([]);
      setUpcomingRemindersUnavailable(true);
      return;
    }
    let disposed = false;
    setUpcomingRemindersLoading(true);
    setUpcomingRemindersUnavailable(false);
    void invoke<UpcomingReminder[]>('get_upcoming_reminders', { windowHours: 24 })
      .then((items) => {
        if (!disposed) setUpcomingReminders(items);
      })
      .catch(() => {
        if (!disposed) {
          setUpcomingReminders([]);
          setUpcomingRemindersUnavailable(true);
        }
      })
      .finally(() => {
        if (!disposed) setUpcomingRemindersLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [activityPanelOpen, homeSkills]);

  useEffect(() => {
    if (
      !shouldLoadPanelAudit(
        activityPanelOpen ||
          section === 'activity' ||
          (settingsOpen && route.settingsSection === 'atividade'),
      )
    )
      return;
    if (Date.now() - activityAuditLoadedAt.current < 10_000) return;
    let disposed = false;
    setActivityAuditLoading(true);
    setActivityAuditUnavailable(false);
    void Promise.all([invoke<AuditEntry[]>('list_audit'), invoke<boolean>('verify_audit')])
      .then(([entries, verified]) => {
        if (!disposed) {
          setActivityAuditRecords(entries);
          setActivityAuditVerified(verified);
          activityAuditLoadedAt.current = Date.now();
        }
      })
      .catch(() => {
        if (!disposed) {
          setActivityAuditRecords([]);
          setActivityAuditVerified(null);
          setActivityAuditUnavailable(true);
          activityAuditLoadedAt.current = Date.now();
        }
      })
      .finally(() => {
        if (!disposed) setActivityAuditLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [activityPanelOpen, section, settingsOpen, route.settingsSection]);

  const sharedAudit = useMemo(
    () => ({
      entries: activityAuditRecords,
      verified: activityAuditVerified,
      loading: activityAuditLoading,
      unavailable: activityAuditUnavailable,
    }),
    [activityAuditRecords, activityAuditVerified, activityAuditLoading, activityAuditUnavailable],
  );
  const activityAudit = useMemo<ActivityAuditSummary[]>(
    () =>
      activityAuditRecords.slice(0, 20).map(({ id, timestamp, tool, status, decision }) => ({
        id,
        timestamp,
        tool,
        status,
        decision,
      })),
    [activityAuditRecords],
  );

  useEffect(() => {
    const onShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'b') return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
      )
        return;
      event.preventDefault();
      setSettings((current) => ({
        ...current,
        layout: { ...current.layout, sidebarCollapsed: !current.layout.sidebarCollapsed },
      }));
    };
    window.addEventListener('keydown', onShortcut);
    return () => window.removeEventListener('keydown', onShortcut);
  }, [setSettings]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<string>('pet-action', ({ payload }) => {
      if (payload === 'open') {
        void invoke('show_main_window').catch((error: unknown) =>
          setToast(`Não foi possível abrir a janela principal: ${String(error)}`),
        );
      } else if (payload === 'new-chat') {
        navigate('chat');
        setConversationId(null);
        setMessages([]);
        setDraft('');
      } else if (payload === 'stop') {
        void invoke('halt_all').catch(() => setToast('Não foi possível parar as ações.'));
      } else if (payload === 'mute') {
        stopLocalSpeech();
        setSettings((current) => ({
          ...current,
          companion: {
            ...current.companion,
            sounds: { ...current.companion.sounds, enabled: false },
          },
        }));
        setToast('O pet foi silenciado.');
      }
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => setToast('Não foi possível iniciar os comandos do pet.'));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setDraft, setMessages, navigate, setSettings, setConversationId, setToast]);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const stops: Array<() => void> = [];
    void Promise.all([
      listen('pet-window-ready', () => {
        void emit('pet-quick-suggestions', quickSuggestionsRef.current);
        void emit('performance-mode-updated', performanceStatusRef.current).catch(() => undefined);
      }),
      listen<IslandQuickRequest>('island-quick-command', ({ payload }) => {
        quickRequestHandler.current(payload);
      }),
      listen<string>('pet-open-conversation', ({ payload }) => {
        void invoke('show_main_window')
          .then(() => openConversationHandler.current(payload))
          .catch(() => setToast('Não foi possível abrir essa conversa no OdeKrai.'));
      }),
      listen<IslandReviewState>('pet-review-alert', ({ payload }) => {
        reviewAlertHandler.current(payload);
      }),
    ])
      .then((off) => {
        if (disposed) off.forEach((stop) => stop());
        else stops.push(...off);
      })
      .catch(() => setToast('Não foi possível conectar os comandos rápidos da ilha.'));
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const refresh = () => {
      void invoke<number>('get_pending_approval_count')
        .then((count) => {
          if (!disposed) setPendingApprovalCount(count);
        })
        .catch(() =>
          reportDiagnostic(
            'get_pending_approval_count',
            'Não foi possível atualizar a contagem de aprovações pendentes.',
          ),
        );
    };
    void listen('approval-updated', refresh)
      .then((stop) => {
        if (disposed) stop();
        else {
          unlisten = stop;
          refresh();
        }
      })
      .catch(() => setToast('Não foi possível acompanhar as aprovações pendentes.'));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (section !== 'skills') setRequestedSkillFamily(null);
  }, [section]);

  useEffect(() => {
    let unlistenHalted: (() => void) | undefined;
    let unlistenResumed: (() => void) | undefined;
    let disposed = false;
    void Promise.all([
      listen('security-halted', () => setAssistantPetState('paused')),
      listen('security-resumed', () => setAssistantPetState('idle')),
    ])
      .then(([halted, resumed]) => {
        if (disposed) {
          halted();
          resumed();
        } else {
          unlistenHalted = halted;
          unlistenResumed = resumed;
        }
      })
      .catch(() =>
        reportDiagnostic(
          'pet-security-state',
          'Não foi possível acompanhar a parada de emergência do mascote.',
        ),
      );
    return () => {
      disposed = true;
      unlistenHalted?.();
      unlistenResumed?.();
    };
  }, [reportDiagnostic]);

  useEffect(() => {
    if (permissionIndicator?.halted) setAssistantPetState('paused');
  }, [permissionIndicator?.halted]);

  async function choosePet(petId: string) {
    if (!PETS.some((pet) => pet.id === petId)) return;
    try {
      await invoke('set_selected_pet', { petId });
      setSelectedPet(petId);
      await emit('selected-pet-updated', petId);
    } catch {
      setToast('Não foi possível salvar o personagem selecionado.');
    }
  }

  useAppEffects(appState);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<{ event: string; petVisible?: boolean }>('sound-request', ({ payload }) => {
      if (!SOUND_EVENTS.includes(payload.event as SoundEvent)) return;
      void playSound(
        payload.event as SoundEvent,
        normalizeSoundPreferences(loadSettings().companion.sounds),
        { petVisible: payload.petVisible },
      );
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => setToast('Não foi possível iniciar os avisos de áudio.'));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    if (diagnostics.length === 0) {
      diagnosticsSoundPlayed.current = false;
      return;
    }
    if (diagnosticsSoundPlayed.current) return;
    diagnosticsSoundPlayed.current = true;
    void playSound('notification', normalizeSoundPreferences(settings.companion.sounds));
  }, [diagnostics.length, settings.companion.sounds]);
  const currentNav = useMemo(() => navigationItems.find((item) => item.id === section), [section]);
  const paletteItems = useMemo(
    () => buildPaletteItems({ conversations, currentConversationId: conversationId, agentEnabled }),
    [conversations, conversationId, agentEnabled],
  );

  useEffect(() => {
    const onShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'k') return;
      event.preventDefault();
      setCommandPaletteOpen((open) => !open);
    };
    window.addEventListener('keydown', onShortcut);
    return () => window.removeEventListener('keydown', onShortcut);
  }, []);

  async function executePaletteItem(item: PaletteItem) {
    setRecentCommands((recent) => updateRecentCommands(recent, item.id));
    setCommandPaletteOpen(false);
    dispatchPaletteAction(item.action, {
      page: (page) => {
        navigate(page);
        if (page === 'chat') setChatMode(true);
      },
      settings: (section, anchor) => {
        if (anchor) {
          settingsEntryRequestId.current += 1;
          setRequestedSettingsEntry({ section, anchor, requestId: settingsEntryRequestId.current });
        }
        selectSettingsSection(section);
      },
      newConversation: startHomeConversation,
      conversation: (id) => openConversation(id),
      agentMode: (enabled) => void setConversationAgentMode(enabled),
      skillFamily: (family) => {
        skillFamilyRequestId.current += 1;
        setRequestedSkillFamily({ family, requestId: skillFamilyRequestId.current });
        navigate('skills');
      },
      pet: (id) => void choosePet(id),
      permissionMode: (mode) => {
        try {
          // This is the same native-confirmation command used by SecurityCenter.
          void invoke('set_permission_mode', { mode }).catch((cause: unknown) => {
            setToast(typeof cause === 'string' ? cause : 'O modo não foi alterado.');
          });
        } catch (cause) {
          setToast(typeof cause === 'string' ? cause : 'O modo não foi alterado.');
        }
      },
      stopAll: () => void stopAll(),
      mute: () => {
        stopLocalSpeech();
        setSettings((current) => ({
          ...current,
          companion: {
            ...current.companion,
            sounds: { ...current.companion.sounds, enabled: false },
          },
        }));
        setToast('O pet foi silenciado.');
      },
    });
  }

  function selectProvider(provider: AiProvider) {
    setSelectedProviderId(provider.id);
  }

  function setPrimaryProvider(providerId: string) {
    setSettings((current) => setPrimaryProviderSettings(current, providerId));
    setSelectedProviderId(providerId);
    setUsedProviderId(null);
  }

  function setProviderEnabled(providerId: string, enabled: boolean) {
    setSettings((current) => toggleProviderSettings(current, providerId, enabled));
    if (!enabled && settings.activeProviderId === providerId) setKeySavedForSession(false);
  }

  function saveProviderSettings(provider: ProviderSettings) {
    setSettings((current) => ({
      ...current,
      providers: { ...current.providers, [provider.providerId]: provider },
    }));
    if (provider.providerId === settings.activeProviderId)
      setKeySavedForSession(provider.providerId !== 'local');
  }

  function handleCredentialRemoved(providerId: string) {
    if (settings.activeProviderId === providerId) setKeySavedForSession(false);
  }

  async function setConversationAgentMode(enabled: boolean) {
    if (enabled && !agentAvailability.available) return;
    if (conversationId) {
      try {
        await invoke('set_conversation_agent_mode', { conversationId, enabled });
        setConversations((items) =>
          items.map((item) =>
            item.id === conversationId ? { ...item, agent_enabled: enabled } : item,
          ),
        );
      } catch (cause) {
        setToast(
          cause instanceof Error ? cause.message : 'Não foi possível alterar o modo agente.',
        );
        return;
      }
    }
    setAgentEnabled(enabled);
  }

  async function hasConfiguredChatProvider() {
    const candidates = [settings.activeProviderId, ...settings.fallbackProviderIds].filter(
      (providerId, index, all) =>
        all.indexOf(providerId) === index &&
        settings.providers[providerId]?.enabled !== false &&
        Boolean(settings.providers[providerId]?.model),
    );
    const { credentialIds } = await loadSavedCredentialProviderIds(candidates, (providerId) =>
      invoke<boolean>('has_provider_credential', { providerId }),
    );
    const available = candidates.find(
      (providerId) => providerId === 'local' || credentialIds.has(providerId),
    );
    if (available) setUsedProviderId(available);
    setKeySavedForSession(Boolean(available));
    return Boolean(available);
  }

  useEffect(() => {
    void hasConfiguredChatProvider().catch(() => setKeySavedForSession(false));
  }, [settings.activeProviderId, settings.fallbackProviderIds, settings.providers]);

  async function beginChat() {
    if (!isTauri() || !(await hasConfiguredChatProvider())) {
      navigate('providers');
      setChatMode(false);
      return;
    }
    setChatMode(true);
  }

  function startHomeConversation() {
    setConversationId(null);
    setMessages([]);
    setAgentActionCards([]);
    setAgentEnabled(agentAvailability.available);
    setError('');
    setAgentErrorDetails('');
    setLastFailedPrompt('');
    setChatMode(true);
    navigate('chat');
  }

  function sendHomeFirstMessage(text: string) {
    return sendMessage(text, false, {
      newConversation: true,
      agentEnabled: agentAvailability.available,
    });
  }

  function reportUndoFailure(actionId: string, error: unknown) {
    const message =
      typeof error === 'string'
        ? error
        : error instanceof Error
          ? error.message
          : 'Não foi possível desfazer a ação.';
    setAgentActionCards((items) => setActionCardUndoError(items, actionId, message));
    setUndoToast(null);
  }

  async function sendMessage(
    rawText: string,
    reuseLastUserMessage = false,
    options: {
      newConversation?: boolean;
      agentEnabled?: boolean;
      conversationId?: string;
      onResponse?: (text: string) => void;
    } = {},
  ): Promise<boolean> {
    const text = rawText.trim();
    if (!text || busy) return false;
    const isNewConversation = options.newConversation ?? !conversationId;
    const useAgent = options.agentEnabled ?? agentEnabled;
    if (useAgent && !agentAvailability.available) {
      setError(
        `Modo agente indisponível: ${agentAvailability.reason ?? 'verifique as configurações do agente.'}`,
      );
      setLastFailedPrompt('');
      return false;
    }
    if (!isTauri()) {
      setError('Configure um provedor de IA em Conectores para enviar mensagens.');
      setLastFailedPrompt('');
      return false;
    }
    let hasProvider = false;
    try {
      hasProvider = await hasConfiguredChatProvider();
    } catch {
      setError('Não foi possível verificar a credencial do provedor.');
      setLastFailedPrompt('');
      return false;
    }
    if (!hasProvider) {
      setError('Configure um provedor de IA em Conectores para enviar mensagens.');
      setLastFailedPrompt('');
      return false;
    }
    const activeId =
      options.conversationId ?? (isNewConversation ? crypto.randomUUID() : conversationId!);
    const sentAt = Math.floor(Date.now() / 1000);
    const next = reuseLastUserMessage
      ? messages
      : [
          ...(isNewConversation ? [] : messages),
          { role: 'user' as const, content: text, created_at: sentAt },
        ];
    setMessages(next);
    setDraft('');
    setError('');
    setAgentErrorDetails('');
    setLastFailedPrompt('');
    setBusy(true);
    const thinkingSoundKey = reuseLastUserMessage
      ? lastThinkingSoundKey.current
      : crypto.randomUUID();
    if (!reuseLastUserMessage) lastThinkingSoundKey.current = thinkingSoundKey;
    if (thinkingSoundKey && thinkingSoundGate.current.claim(thinkingSoundKey)) {
      void playSound('thinking', normalizeSoundPreferences(settings.companion.sounds));
    }
    if (isTauri())
      emitCompanionState({
        type: 'activity',
        state: 'thinking',
        message: 'Aguardando resposta do provedor',
      });
    const requestId = crypto.randomUUID();
    request.current = requestId;
    try {
      const provider = AI_PROVIDERS.find((item) => item.id === settings.activeProviderId);
      if (!provider) throw new Error('O provedor selecionado não possui adaptador conhecido.');
      const agentProviderTargets = [
        { providerId: provider.id, baseUrl: activeProvider.baseUrl, model: activeProvider.model },
        ...settings.fallbackProviderIds.flatMap((fallbackId) => {
          const config = settings.providers[fallbackId];
          return config?.enabled !== false && config?.model
            ? [{ providerId: fallbackId, baseUrl: config.baseUrl, model: config.model }]
            : [];
        }),
      ];
      setUsedProviderId(provider.id);
      if (!reuseLastUserMessage) {
        await invoke('append_conversation_message', {
          conversationId: activeId,
          role: 'user',
          content: text,
          providerId: provider.id,
          model: activeProvider.model,
          providerTargets: agentProviderTargets,
        });
        if (isNewConversation && !useAgent && agentAvailability.available) {
          await invoke('set_conversation_agent_mode', {
            conversationId: activeId,
            enabled: false,
          });
        }
        setConversations((items) => {
          const existing = items.find((item) => item.id === activeId);
          if (!existing) return items;
          return [{ ...existing, preview: text }, ...items.filter((item) => item.id !== activeId)];
        });
      }
      setConversationId(activeId);
      if (isNewConversation)
        setConversations((items) => [
          {
            id: activeId,
            title: text.slice(0, 80),
            preview: text,
            created_at: sentAt,
            provider_id: provider.id,
            model: activeProvider.model,
            agent_enabled: useAgent && agentAvailability.available,
          },
          ...items,
        ]);
      if (useAgent && agentAvailability.available) {
        const onEvent = new Channel<{
          type:
            | 'step'
            | 'toolCall'
            | 'awaitingApproval'
            | 'toolResult'
            | 'finalResponse'
            | 'error'
            | 'stopped'
            | 'actionCard';
          turn?: number;
          id?: string;
          name?: string;
          auditId?: string | null;
          preview?: string;
          expiresAt?: number;
          isError?: boolean;
          content?: string;
          kind?: string;
          message?: string;
          technicalDetails?: string | null;
          card?: AgentActionCardData & { createdAt: number };
        }>();
        agentRunConversation.current = activeId;
        setAgentRunning(true);
        setAgentAwaitingApproval(false);
        onEvent.onmessage = (event) => {
          if (event.type === 'actionCard' && event.card) {
            setAgentActionCards((items) => {
              const next = items.filter((item) => item.id !== event.card!.id);
              return [...next, event.card!].slice(-100);
            });
            if (event.card.status === 'Concluída' && event.card.undoDescriptor) {
              setUndoToast({ cardId: event.card.id });
            }
            setAgentAwaitingApproval(event.card.status === 'Aguardando aprovação');
          } else if (event.type === 'awaitingApproval') {
            setAgentAwaitingApproval(true);
            if (event.preview || event.expiresAt) {
              setAgentActionCards((items) => {
                let index = -1;
                for (let cursor = items.length - 1; cursor >= 0; cursor -= 1) {
                  if (items[cursor].status === 'Aguardando aprovação') {
                    index = cursor;
                    break;
                  }
                }
                if (index < 0) return items;
                return items.map((item, itemIndex) =>
                  itemIndex === index
                    ? {
                        ...item,
                        preview: item.preview || event.preview,
                        approvalExpiresAt: item.approvalExpiresAt || event.expiresAt,
                      }
                    : item,
                );
              });
            }
            void playSound('approval_needed', normalizeSoundPreferences(settings.companion.sounds));
            emitCompanionState({
              type: 'activity',
              state: 'awaiting_approval',
              message: 'Aguardando aprovação na janela nativa',
            });
          } else if (event.type === 'toolCall' || event.type === 'step') {
            setAgentAwaitingApproval(false);
            emitCompanionState({
              type: 'activity',
              state: event.type === 'toolCall' ? 'task' : 'thinking',
              message: event.type === 'toolCall' ? 'Executando habilidade' : 'Pensando',
            });
          } else if (event.type === 'toolResult') {
            setAgentAwaitingApproval(false);
            if (event.isError) {
              void playSound('error', normalizeSoundPreferences(settings.companion.sounds));
            } else {
              void playSound('action_done', normalizeSoundPreferences(settings.companion.sounds));
            }
            emitCompanionState({
              type: 'activity',
              state: event.isError ? 'error' : 'task',
              message: event.isError ? 'A ação não foi concluída' : 'Ação concluída',
            });
          } else if (event.type === 'finalResponse') {
            const content = event.content ?? '';
            setMessages([
              ...next,
              { role: 'assistant', content, created_at: Math.floor(Date.now() / 1000) },
            ]);
            void playSound('success', normalizeSoundPreferences(settings.companion.sounds));
            emitCompanionState({
              type: 'activity',
              state: 'completed',
              message: 'Resposta pronta',
            });
          } else if (event.type === 'error') {
            setError(event.message ?? 'O agente não conseguiu concluir a execução.');
            setAgentErrorDetails(event.technicalDetails ?? '');
            void playSound('error', normalizeSoundPreferences(settings.companion.sounds));
            emitCompanionState({
              type: 'activity',
              state: 'error',
              message: 'Falha no agente',
            });
          } else if (event.type === 'stopped') {
            setAgentAwaitingApproval(false);
            void playSound('halted', normalizeSoundPreferences(settings.companion.sounds));
            emitCompanionState({
              type: 'activity',
              state: 'paused',
              message: 'Agente parado',
            });
          }
        };
        const answer = await invoke<string>('agent_run', {
          conversationId: activeId,
          message: text,
          dailyTokenLimit: settings.dailyTokenLimit,
          onEvent,
        });
        options.onResponse?.(answer);
        setMessages([
          ...next,
          { role: 'assistant', content: answer, created_at: Math.floor(Date.now() / 1000) },
        ]);
        setConversations((items) =>
          items.map((item) => (item.id === activeId ? { ...item, preview: answer } : item)),
        );
        void invoke<AgentActionCardData[]>('load_agent_action_cards', {
          conversationId: activeId,
        })
          .then(setAgentActionCards)
          .catch(() => setAgentActionCards([]));
        return true;
      }
      const onChunk = new Channel<
        | { type: 'textDelta'; text: string }
        | { type: 'toolCallComplete'; id: string; name: string; args: unknown }
        | { type: 'usage'; input: number; output: number }
        | { type: 'providerSelected'; providerId: string }
        | { type: 'done'; stopReason: string | null }
        | { type: 'error'; kind: string; status: number | null; message: string }
      >();
      let responseProviderId = provider.id;
      let streamed = '';
      onChunk.onmessage = (event) => {
        if (event.type === 'providerSelected') {
          responseProviderId = event.providerId;
          setUsedProviderId(event.providerId);
          return;
        }
        if (event.type !== 'textDelta') return;
        streamed += event.text;
        setMessages([
          ...next,
          { role: 'assistant', content: streamed, created_at: Math.floor(Date.now() / 1000) },
        ]);
      };
      const answer = await invoke<{ content: string; citations: { title: string; url: string }[] }>(
        'stream_provider_message',
        {
          providerId: provider.id,
          baseUrl: activeProvider.baseUrl,
          model: activeProvider.model,
          messages: next.map((message) => ({
            role: message.role,
            blocks: [{ type: 'text' as const, text: message.content }],
          })),
          tools: [],
          fallbacks: settings.fallbackProviderIds.flatMap((fallbackId) => {
            const config = settings.providers[fallbackId];
            return config?.enabled !== false && config?.model
              ? [{ providerId: fallbackId, baseUrl: config.baseUrl, model: config.model }]
              : [];
          }),
          dailyTokenLimit: settings.dailyTokenLimit,
          requestId,
          onChunk,
        },
      );
      options.onResponse?.(answer.content);
      await invoke('append_conversation_message', {
        conversationId: activeId,
        role: 'assistant',
        content: answer.content,
        providerId: responseProviderId,
        model:
          settings.providers[responseProviderId]?.model ??
          (responseProviderId === provider.id ? activeProvider.model : ''),
        providerTargets: agentProviderTargets,
      });
      const responseModel =
        settings.providers[responseProviderId]?.model ??
        (responseProviderId === provider.id ? activeProvider.model : '');
      setConversations((items) =>
        items.map((item) =>
          item.id === activeId
            ? {
                ...item,
                preview: answer.content,
                provider_id: responseProviderId,
                model: responseModel,
              }
            : item,
        ),
      );
      setMessages([
        ...next,
        {
          role: 'assistant',
          content: answer.content,
          citations: answer.citations,
          created_at: Math.floor(Date.now() / 1000),
        },
      ]);
      lastThinkingSoundKey.current = null;
      void playSound('success', normalizeSoundPreferences(settings.companion.sounds));
      window.clearTimeout(companionStateTimer.current);
      if (isTauri()) {
        emitCompanionState({
          type: 'activity',
          state: 'responding',
          message: 'Resposta pronta',
        });
        companionStateTimer.current = window.setTimeout(
          () =>
            emitCompanionState({
              type: 'activity',
              state: 'completed',
              message: 'Resposta recebida',
            }),
          1100,
        );
      }
      return true;
    } catch (cause) {
      const stopped =
        typeof cause === 'string' &&
        (cause.toLocaleLowerCase('pt-BR').includes('parad') ||
          cause.toLocaleLowerCase('pt-BR').includes('cancel'));
      if (!stopped) {
        setMessages(next);
        setLastFailedPrompt(text);
        setError(
          typeof cause === 'string'
            ? cause
            : cause instanceof Error
              ? cause.message
              : 'Não foi possível concluir a solicitação.',
        );
        void playSound('error', normalizeSoundPreferences(settings.companion.sounds));
        if (isTauri())
          emitCompanionState({
            type: 'activity',
            state: 'error',
            message: 'Não foi possível concluir a solicitação',
          });
      } else {
        setMessages(next);
        setLastFailedPrompt('');
      }
      return false;
    } finally {
      setBusy(false);
      request.current = null;
      setAgentRunning(false);
      setAgentAwaitingApproval(false);
      agentRunConversation.current = null;
    }
  }

  quickRequestHandler.current = (request) => {
    let summary = '';
    void (async () => {
      const success = await sendMessage(request.text, false, {
        newConversation: true,
        agentEnabled: agentAvailability.available,
        conversationId: request.conversationId,
        onResponse: (response) => {
          summary = summarizeIslandResponse(response);
        },
      });
      if (!summary) {
        summary = success
          ? 'A conversa foi concluída. Abra no OdeKrai para ver a resposta completa.'
          : 'Não foi possível concluir. Revise a conversa no OdeKrai.';
      }
      void emit('pet-quick-result', {
        requestId: request.requestId,
        conversationId: request.conversationId,
        title: request.text.slice(0, 80),
        summary,
        success,
      });
    })();
  };

  function submitMessage(event: FormEvent): Promise<boolean> {
    event.preventDefault();
    return sendMessage(draft);
  }

  function openConversation(id: string) {
    if (busy) return;
    const selectedId = resolveConversationSelection(conversations, id);
    if (!selectedId) return;
    setConversationId(selectedId);
    setConversations((items) => {
      const selected = items.find((item) => item.id === selectedId);
      return selected ? [selected, ...items.filter((item) => item.id !== selectedId)] : items;
    });
    setError('');
    setChatMode(true);
    navigate('chat');
    void Promise.all([
      invoke<ChatMessage[]>('load_conversation', { conversationId: selectedId }),
      invoke<boolean>('get_conversation_agent_mode', { conversationId: selectedId }),
      invoke<AgentActionCardData[]>('load_agent_action_cards', { conversationId: selectedId }),
    ])
      .then(([loaded, enabled, cards]) => {
        setMessages(loaded);
        setAgentEnabled(enabled);
        setAgentActionCards(cards);
      })
      .catch(() => setError('Não foi possível abrir a conversa.'));
  }

  openConversationHandler.current = openConversation;
  reviewAlertHandler.current = (state) => {
    void (async () => {
      try {
        await invoke('show_main_window');
        const destination = islandReviewDestination(state, Boolean(activeConversationRef.current));
        if (destination === 'approval-window') {
          await invoke('show_pending_approval_window');
        } else if (destination === 'calendar') {
          navigate('calendar');
        } else if (destination === 'conversation' && activeConversationRef.current) {
          openConversationHandler.current(activeConversationRef.current);
        } else {
          navigate('chat');
        }
      } catch {
        setToast('Não foi possível abrir o local para revisar este alerta.');
      }
    })();
  };

  function cancelRequest() {
    if (agentRunConversation.current) {
      void invoke('agent_cancel', { conversationId: agentRunConversation.current }).catch(() =>
        setError('Não foi possível parar a execução do agente.'),
      );
      return;
    }
    if (request.current)
      void invoke('cancel_provider_request', { requestId: request.current }).catch(() =>
        console.warn('[OdeKrai diagnóstico] Não foi possível cancelar a solicitação do provedor.'),
      );
  }

  async function stopAll() {
    setAssistantPetState('paused');
    stopLocalSpeech();
    try {
      await invoke('halt_all');
    } catch {
      setToast('Não foi possível parar as ações.');
    }
  }

  async function deleteConversation(id: string) {
    if (busy) return;
    try {
      if (
        !(await confirm({
          title: 'Apagar conversa?',
          message: 'Apagar esta conversa e suas mensagens?',
          destructive: true,
        }))
      )
        return;
    } catch {
      setToast('Não foi possível mostrar a confirmação. A conversa não foi apagada.');
      return;
    }
    try {
      await invoke('delete_conversation', { conversationId: id });
      setConversations((items) => items.filter((item) => item.id !== id));
      if (conversationId === id) {
        setConversationId(null);
        setMessages([]);
      }
    } catch {
      setError('Não foi possível apagar a conversa.');
    }
  }

  async function deleteAllConversations(): Promise<boolean> {
    if (busy) {
      setToast('Aguarde a conclusão da resposta antes de apagar o histórico.');
      return false;
    }
    try {
      await invoke('delete_all_conversations');
      setConversations([]);
      setConversationId(null);
      setMessages([]);
      setDraft('');
      setError('');
      setLastFailedPrompt('');
      setToast('Todo o histórico foi apagado.');
      return true;
    } catch {
      setToast('Não foi possível apagar o histórico.');
      return false;
    }
  }

  async function renameConversation(id: string, rawTitle: string) {
    const title = rawTitle.trim();
    if (!title || title.length > 120) return;
    try {
      await invoke('rename_conversation', { conversationId: id, title });
      setConversations((items) =>
        items.map((item) => (item.id === id ? { ...item, title } : item)),
      );
    } catch {
      setToast('Não foi possível renomear a conversa.');
    }
  }

  async function openResponseLink(rawUrl: string) {
    const url = safeMarkdownUrl(rawUrl);
    const domain = url ? externalLinkDomain(url) : null;
    if (!url || !domain) return;
    const needsConfirmation = !settings.trustedLinkDomains.includes(domain);
    if (needsConfirmation) {
      let accepted = false;
      try {
        accepted = await confirm({
          title: 'Abrir link externo?',
          message: `Este link abrirá ${domain} no navegador padrão.\n\n${url}\n\nAbrir e lembrar este domínio?`,
        });
      } catch {
        setToast('Não foi possível mostrar a confirmação. O link não foi aberto.');
        return;
      }
      if (!accepted) return;
    }
    try {
      if (!isTauri()) throw new Error('Abra links nas respostas pelo aplicativo OdeKrai.');
      await invoke('trust_external_domain', { domain });
      if (needsConfirmation) {
        setSettings((current) => ({
          ...current,
          trustedLinkDomains: [...new Set([...current.trustedLinkDomains, domain])].slice(0, 100),
        }));
      }
      await invoke('open_external_link', { url });
    } catch (cause) {
      setToast(cause instanceof Error ? cause.message : 'Não foi possível abrir este link.');
    }
  }

  async function openUpdateRelease() {
    if (!availableUpdate?.release_url) return;
    try {
      await invoke('open_update_release', { url: availableUpdate.release_url });
    } catch {
      setToast('Não foi possível abrir a página oficial da release.');
    }
  }

  const firstUseSteps: FirstUseStepContent[] = [
    {
      id: 0,
      title: 'Conecte um provedor',
      description:
        'Escolha um provedor, salve a chave no Gerenciador de Credenciais do Windows e teste a conexão quando quiser.',
      complete:
        (keySavedForSession || settings.activeProviderId === 'local') &&
        settings.providers[settings.activeProviderId]?.lastTest?.status === 'passed',
      content: (
        <Suspense fallback={screenLoading}>
          <ProvidersScreen
            settings={settings}
            selectedId={selectedProviderId}
            onSelect={selectProvider}
            onSetPrimary={setPrimaryProvider}
            onToggleEnabled={setProviderEnabled}
            onSave={saveProviderSettings}
            onChat={beginChat}
            onCredentialRemoved={handleCredentialRemoved}
            initialTab="free"
            onboardingMode
          />
        </Suspense>
      ),
    },
    {
      id: 1,
      title: 'Escolha a pasta do OdeKrai',
      description:
        'Use o fluxo existente para criar uma pasta nova, localizar a atual ou escolher uma pasta existente.',
      complete: identityConfigured === true,
      content: (
        <Suspense fallback={screenLoading}>
          <IdentityScreen
            initialName={profile.displayName}
            initialNickname={profile.nickname}
            profileValue={profile}
            onProfileChange={setProfile}
            onStatus={(status) => setIdentityConfigured(status.configured)}
            embedded
          />
        </Suspense>
      ),
    },
    {
      id: 2,
      title: 'Escolha seu mascote',
      description:
        'A grade mostra os mascotes disponíveis pelo nome. Você pode trocar a escolha depois.',
      complete: PETS.some((pet) => pet.id === selectedPet),
      content: (
        <Suspense fallback={screenLoading}>
          <AssistantScreen
            settings={settings}
            update={setSettings}
            petVisible={petVisible}
            onPetVisibilityChange={setPetVisible}
            onError={setToast}
            selectedPet={selectedPet}
            onSelectPet={(petId) => void choosePet(petId)}
            embedded
          />
        </Suspense>
      ),
    },
    {
      id: 3,
      title: 'Revise as permissões',
      description:
        'O padrão é “Sempre perguntar”. “Controle total” mantém bloqueios fixos e só pode ser ativado após a confirmação nativa do Windows.',
      complete: Boolean(
        permissionIndicator &&
        ['sempre_perguntar', 'aprovar_por_mim', 'controle_total'].includes(
          permissionIndicator.mode,
        ),
      ),
      content: (
        <SecurityCenter
          page="permissions"
          embedded
          appSettings={settings}
          updateAppSettings={setSettings}
        />
      ),
    },
  ];

  const currentActivityAction =
    [...agentActionCards]
      .reverse()
      .find(
        (action) => action.status === 'Executando' || action.status === 'Aguardando aprovação',
      ) ?? null;

  return (
    <div
      className={`app-shell ${settings.reduceMotion ? 'reduce-motion' : ''} ${chatMode ? 'chat-mode' : ''} ${activityPanelOpen ? 'has-activity-panel' : ''}`}
      style={
        {
          '--activity-sidebar-width': `${settings.layout.sidebarCollapsed ? 72 : settings.layout.sidebarWidth}px`,
        } as CSSProperties
      }
    >
      <aside
        className={`sidebar ${settings.layout.sidebarCollapsed ? 'sidebar-collapsed' : ''}`}
        style={{ width: settings.layout.sidebarCollapsed ? 72 : settings.layout.sidebarWidth }}
      >
        <SidebarNavigation
          section={section}
          onSelectSection={navigate}
          onSetChatMode={setChatMode}
          keySavedForSession={keySavedForSession}
          compact={settings.layout.sidebarCollapsed}
        />
        {!settings.layout.sidebarCollapsed && (
          <ResizeHandle
            label="Redimensionar barra lateral"
            value={settings.layout.sidebarWidth}
            min={190}
            max={340}
            onResize={(sidebarWidth) =>
              setSettings((current) => ({
                ...current,
                layout: { ...current.layout, sidebarWidth },
              }))
            }
          />
        )}
        <SidebarProfileMenu
          profile={profile}
          compact={settings.layout.sidebarCollapsed}
          section={section}
          onMyProfile={() => navigate('agent')}
          onSettings={() => navigate('settings')}
          onPermissions={() => navigate('permissions')}
          onHelp={() => setFirstUseOpen(true)}
          onAbout={() => selectSettingsSection('sobre')}
        />
      </aside>

      <main className="main-area">
        <header className="topbar">
          <button
            type="button"
            className="sidebar-toggle"
            aria-label={
              settings.layout.sidebarCollapsed ? 'Expandir barra lateral' : 'Recolher barra lateral'
            }
            title="Alternar barra lateral (Ctrl+B)"
            onClick={() =>
              setSettings((current) => ({
                ...current,
                layout: { ...current.layout, sidebarCollapsed: !current.layout.sidebarCollapsed },
              }))
            }
          >
            {settings.layout.sidebarCollapsed ? '»' : '«'}
          </button>
          <div className="breadcrumbs">
            <span>OdeKrai</span>
            <b>/</b>
            <strong>
              {chatMode
                ? (conversations.find((item) => item.id === conversationId)?.title ??
                  'Nova conversa')
                : currentNav?.label}
            </strong>
          </div>
          <div className="top-actions">
            <ActivityPanelTrigger
              count={pendingApprovalCount}
              expanded={activityPanelOpen}
              buttonRef={activityPanelTriggerRef}
              onClick={() => setActivityPanelOpen(toggleActivityPanel)}
            />
            {shouldShowStopAll(busy, agentRunning) && (
              <button type="button" className="stop-all-button" onClick={() => void stopAll()}>
                Parar tudo
              </button>
            )}
            <button
              type="button"
              className="provider-chip"
              onClick={() => {
                navigate('providers');
                setChatMode(false);
              }}
              aria-label="Abrir conectores de IA"
            >
              {keySavedForSession
                ? `${shownProviderName}${shownProviderModel ? ` · ${shownProviderModel}` : ''}`
                : 'Sem provedor'}
            </button>
            {permissionIndicator?.halted && (
              <button
                className="security-state halted"
                onClick={() => {
                  navigate('permissions');
                  setChatMode(false);
                }}
              >
                PARADO · PERMISSÕES
              </button>
            )}
            {!permissionIndicator?.halted && permissionIndicator?.mode === 'controle_total' && (
              <button
                className="security-state total"
                onClick={() => {
                  navigate('permissions');
                  setChatMode(false);
                }}
              >
                CONTROLE TOTAL
              </button>
            )}
          </div>
        </header>
        {availableUpdate && (
          <UpdateAvailableBanner
            update={availableUpdate}
            onOpen={() => void openUpdateRelease()}
            onDismiss={() => setAvailableUpdate(null)}
          />
        )}
        {diagnostics.length > 0 && (
          <aside className="diagnostic-banner" aria-label="Diagnóstico" role="status">
            <div className="diagnostic-banner-heading">
              <strong>Diagnóstico</strong>
              <button type="button" aria-label="Fechar diagnóstico" onClick={dismissDiagnostics}>
                ×
              </button>
            </div>
            {diagnostics.map((item) => (
              <div className="diagnostic-item" key={item.key}>
                <div className="diagnostic-item-heading">
                  <p>
                    {item.message}
                    {item.count > 1 ? ` x${item.count}` : ''}
                  </p>
                  <button
                    type="button"
                    aria-label={`Fechar aviso: ${item.message}`}
                    onClick={() => dismissDiagnostic(item.key)}
                  >
                    ×
                  </button>
                </div>
                {item.details && (
                  <details>
                    <summary>Detalhes técnicos</summary>
                    <code>{item.details}</code>
                  </details>
                )}
                {item.command === 'agent_home_status' && (
                  <button
                    type="button"
                    className="secondary-button small-button"
                    onClick={() => {
                      navigate('agent');
                      setChatMode(false);
                    }}
                  >
                    Resolver pasta do agente
                  </button>
                )}
              </div>
            ))}
          </aside>
        )}
        <Suspense fallback={screenLoading}>
          {chatMode || section === 'chat' ? (
            <ConversationsScreen
              messages={messages}
              conversations={conversations}
              conversationId={conversationId}
              onConversation={openConversation}
              onNewConversation={() => {
                if (!busy) {
                  setConversationId(null);
                  setMessages([]);
                  setAgentActionCards([]);
                  setAgentEnabled(agentAvailability.available);
                  setError('');
                  setLastFailedPrompt('');
                }
              }}
              onDeleteConversation={(id) => void deleteConversation(id)}
              onRenameConversation={(id, title) => void renameConversation(id, title)}
              busy={busy}
              draft={draft}
              setDraft={setDraft}
              error={error}
              errorDetails={agentErrorDetails}
              canFixConfiguration={
                !/rede|conexão|limitou as solicitações|tempo limite|demorou para responder|serviço do provedor está indisponível/i.test(
                  error,
                )
              }
              submit={submitMessage}
              cancel={cancelRequest}
              providerModel={shownProviderModel}
              providerId={shownProviderId}
              microphoneEnabled={settings.microphoneEnabled}
              onMicrophonePreferenceChange={async (enabled) => {
                const nextSettings = { ...settings, microphoneEnabled: enabled };
                if (!saveSettings(nextSettings))
                  throw new Error('Não foi possível salvar a preferência.');
                try {
                  if (isTauri()) await invoke('set_microphone_opt_in', { enabled });
                } catch (error) {
                  saveSettings(settings);
                  throw error;
                }
                setSettings(nextSettings);
              }}
              readRepliesAutomatically={settings.readRepliesAutomatically}
              speechVoiceURI={settings.speechVoiceURI}
              speechRate={settings.speechRate}
              speechVolume={settings.speechVolume}
              conversationSidebarCollapsed={settings.layout.conversationSidebarCollapsed}
              conversationSidebarWidth={settings.layout.conversationSidebarWidth}
              onToggleConversationSidebar={() =>
                setSettings((current) => ({
                  ...current,
                  layout: {
                    ...current.layout,
                    conversationSidebarCollapsed: !current.layout.conversationSidebarCollapsed,
                  },
                }))
              }
              onResizeConversationSidebar={(conversationSidebarWidth) =>
                setSettings((current) => ({
                  ...current,
                  layout: { ...current.layout, conversationSidebarWidth },
                }))
              }
              onOpenLink={(url) => void openResponseLink(url)}
              providerConfigured={keySavedForSession}
              assistantUsesMascot={settings.useMascotAsAssistantAvatar}
              petState={assistantPetState}
              reduceMotion={settings.reduceMotion}
              reduceMascot={settings.reduceMascot}
              onFixConfiguration={() => {
                navigate(resolveConfigurationReason(error));
                setChatMode(false);
              }}
              onRetry={() => void sendMessage(lastFailedPrompt, true)}
              canRetry={
                Boolean(lastFailedPrompt) &&
                /rede|conexão|limitou as solicitações|tempo limite|demorou para responder|serviço do provedor está indisponível/i.test(
                  error,
                )
              }
              profile={profile}
              selectedPet={selectedPet}
              agentEnabled={agentEnabled}
              agentUnavailableReason={agentAvailability.available ? null : agentAvailability.reason}
              agentWorkspacePath={agentAvailability.workspacePath}
              agentRunning={agentRunning}
              agentAwaitingApproval={agentAwaitingApproval}
              agentActionCards={agentActionCards}
              onReviewApproval={() =>
                void invoke('show_pending_approval_window').catch(() =>
                  setToast('Não foi possível abrir a janela de aprovação.'),
                )
              }
              onConfigureDiscord={() => {
                settingsEntryRequestId.current += 1;
                setRequestedSettingsEntry({
                  section: 'privacidade',
                  anchor: 'discord-integration-heading',
                  requestId: settingsEntryRequestId.current,
                });
                selectSettingsSection('privacidade');
              }}
              onUndo={(action) => {
                const descriptor = action.undoDescriptor;
                if (!descriptor) return;
                const requestUndo = (confirmModified: boolean) =>
                  invoke<{ message: string; confirmationRequired: boolean; status: string }>(
                    'undo_agent_action',
                    { descriptor, confirmModified },
                  )
                    .then(async (result) => {
                      if (result.confirmationRequired) {
                        try {
                          if (
                            await confirm({
                              title: 'Confirmar restauração?',
                              message: result.message,
                              destructive: true,
                            })
                          )
                            requestUndo(true);
                        } catch {
                          setToast(
                            'Não foi possível mostrar a confirmação. O arquivo não foi restaurado.',
                          );
                        }
                        return;
                      }
                      setAgentActionCards((items) =>
                        items.map((item) =>
                          item.id === action.id
                            ? {
                                ...item,
                                undoDescriptor: null,
                                undoError: null,
                                resultSummary: `${item.resultSummary ?? ''}\n${result.message}`,
                              }
                            : item,
                        ),
                      );
                      setUndoToast(null);
                      setToast(result.message);
                    })
                    .catch((error) => reportUndoFailure(action.id, error));
                void requestUndo(false);
              }}
              onOpenArtifact={(path) =>
                void invoke('open_agent_artifact', { path }).catch((error) =>
                  setToast(
                    error instanceof Error ? error.message : 'Não foi possível abrir o arquivo.',
                  ),
                )
              }
              onRevealArtifact={(path) =>
                void invoke('reveal_agent_artifact', { path }).catch((error) =>
                  setToast(
                    error instanceof Error ? error.message : 'Não foi possível mostrar o arquivo.',
                  ),
                )
              }
              onToggleAgent={(enabled) => void setConversationAgentMode(enabled)}
              onStopAgent={cancelRequest}
              onConfigureAgent={() => {
                const reason = agentAvailability.reason ?? '';
                setChatMode(false);
                navigate(resolveAgentReason(reason));
              }}
            />
          ) : (
            <div className="content-scroll">
              {section === 'overview' && (
                <Overview
                  draft={draft}
                  setDraft={setDraft}
                  busy={busy}
                  keyReady={keySavedForSession}
                  profile={profile}
                  selectedPet={selectedPet}
                  petState={assistantPetState}
                  reduceMotion={settings.reduceMotion}
                  reduceMascot={settings.reduceMascot}
                  suggestions={homeSuggestions}
                  conversations={conversations.slice(0, 3)}
                  checklist={homeChecklist}
                  pendingApprovalCount={pendingApprovalCount}
                  skillsLoadError={homeSkillsLoadError}
                  onStartConversation={startHomeConversation}
                  onSendMessage={sendHomeFirstMessage}
                  onResumeConversation={openConversation}
                  onResolveChecklistItem={(item) => navigate(item.destination)}
                  onOpenApprovals={() =>
                    void invoke('show_pending_approval_window').catch(() =>
                      setToast('Não foi possível abrir a janela de aprovação.'),
                    )
                  }
                  onRetrySkills={() => setHomeContextRetry((retry) => retry + 1)}
                />
              )}
              {section === 'providers' && (
                <ProvidersScreen
                  settings={settings}
                  selectedId={selectedProviderId}
                  onSelect={selectProvider}
                  onSetPrimary={setPrimaryProvider}
                  onToggleEnabled={setProviderEnabled}
                  onSave={saveProviderSettings}
                  onChat={beginChat}
                  onCredentialRemoved={handleCredentialRemoved}
                />
              )}
              {section === 'computer' && <ComputerScreen />}
              {section === 'calendar' && (
                <CalendarScreen
                  selectedPet={selectedPet}
                  petState={assistantPetState}
                  reduceMotion={settings.reduceMotion}
                  reduceMascot={settings.reduceMascot}
                />
              )}
              {section === 'skills' && (
                <SkillsScreen
                  key={requestedSkillFamily?.requestId ?? 'all-families'}
                  initialFamily={requestedSkillFamily?.family}
                  onOpenPrivacy={() => navigate('permissions')}
                  agentRuntime={{
                    modeEnabled: agentEnabled,
                    available: agentAvailability.available,
                    reason: agentAvailability.reason,
                  }}
                />
              )}
              {section === 'companion' && (
                <AssistantScreen
                  settings={settings}
                  update={setSettings}
                  petVisible={petVisible}
                  onPetVisibilityChange={setPetVisible}
                  onError={setToast}
                  selectedPet={selectedPet}
                  onSelectPet={(petId) => void choosePet(petId)}
                />
              )}
              {section === 'settings' && (
                <SettingsScreen
                  settings={settings}
                  update={setSettings}
                  onOpenPermissions={() => navigate('permissions')}
                  performanceStatus={performanceStatus}
                />
              )}
              {section === 'permissions' && <PermissionsScreen />}
              {section === 'activity' && <ActivityScreen sharedAudit={sharedAudit} />}
              {section === 'agent' && (
                <IdentityScreen
                  initialName={profile.displayName}
                  initialNickname={profile.nickname}
                  profileValue={profile}
                  onProfileChange={setProfile}
                  onStatus={() => reportDiagnostic('agent_home_status', '')}
                />
              )}
            </div>
          )}
        </Suspense>
      </main>
      <ActivitySidePanel
        open={activityPanelOpen}
        triggerRef={activityPanelTriggerRef}
        pendingApprovalCount={pendingApprovalCount}
        running={agentRunning}
        awaitingApproval={agentAwaitingApproval}
        currentAction={currentActivityAction}
        events={scheduledEvents}
        reminderCount={reminderCount}
        reminders={upcomingReminders}
        remindersLoading={upcomingRemindersLoading}
        upcomingReminderUnavailable={upcomingRemindersUnavailable}
        scheduleUnavailable={scheduleUnavailable}
        reminderUnavailable={reminderUnavailable}
        actions={agentActionCards}
        artifactHistoryAvailable={Boolean(conversationId)}
        audit={activityAudit}
        scheduleLoading={homeSkills === null}
        auditLoading={activityAuditLoading}
        auditUnavailable={activityAuditUnavailable}
        onClose={() => setActivityPanelOpen(false)}
        onStop={cancelRequest}
        onReview={() =>
          void invoke('show_pending_approval_window').catch(() =>
            setToast('Não foi possível abrir a janela de aprovação.'),
          )
        }
        onOpenArtifact={(path) =>
          void invoke('open_agent_artifact', { path }).catch(() =>
            setToast('Não foi possível abrir o arquivo.'),
          )
        }
        onRevealArtifact={(path) =>
          void invoke('reveal_agent_artifact', { path }).catch(() =>
            setToast('Não foi possível mostrar o arquivo.'),
          )
        }
        onViewAll={() => {
          setActivityPanelOpen(false);
          setChatMode(false);
          navigate('activity');
        }}
      />
      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}
      {undoToast && (
        <div className="toast" role="status">
          Ação reversível concluída.
          <button
            type="button"
            onClick={() => {
              const action = agentActionCards.find((item) => item.id === undoToast.cardId);
              if (!action?.undoDescriptor) return;
              const requestUndo = (confirmModified: boolean) =>
                invoke<{ message: string; confirmationRequired: boolean; status: string }>(
                  'undo_agent_action',
                  { descriptor: action.undoDescriptor, confirmModified },
                )
                  .then(async (result) => {
                    if (result.confirmationRequired) {
                      try {
                        if (
                          await confirm({
                            title: 'Confirmar restauração?',
                            message: result.message,
                            destructive: true,
                          })
                        )
                          requestUndo(true);
                      } catch {
                        setToast(
                          'Não foi possível mostrar a confirmação. O arquivo não foi restaurado.',
                        );
                      }
                      return;
                    }
                    setAgentActionCards((items) =>
                      items.map((item) =>
                        item.id === action.id
                          ? {
                              ...item,
                              undoDescriptor: null,
                              undoError: null,
                              resultSummary: `${item.resultSummary ?? ''}\n${result.message}`,
                            }
                          : item,
                      ),
                    );
                    setUndoToast(null);
                    setToast(result.message);
                  })
                  .catch((error) => reportUndoFailure(action.id, error));
              void requestUndo(false);
            }}
          >
            Desfazer
          </button>
        </div>
      )}
      <FirstUseAssistant
        open={firstUseOpen}
        steps={firstUseSteps}
        onClose={() => setFirstUseOpen(false)}
        onOpenDocumentation={() =>
          void openResponseLink('https://github.com/Anceryx/OdeKrai/tree/main/docs')
        }
      />
      <ReleaseNotesModal
        open={releaseNotesOpen}
        showAllVersions={showAllReleaseNotes}
        onShowAllVersions={() => setShowAllReleaseNotes(true)}
        onShowCurrent={() => setShowAllReleaseNotes(false)}
        onClose={() => setReleaseNotesOpen(false)}
      />
      <SettingsModal
        open={settingsOpen}
        section={route.settingsSection ?? 'geral'}
        onClose={closeSettings}
        onSelectSection={selectSettingsSection}
        settings={settings}
        update={setSettings}
        performanceStatus={performanceStatus}
        profile={profile}
        setProfile={setProfile}
        petVisible={petVisible}
        setPetVisible={setPetVisible}
        selectedPet={selectedPet}
        onSelectPet={(petId) => void choosePet(petId)}
        onError={setToast}
        onDeleteAllHistory={deleteAllConversations}
        onOpenPage={(page) => navigate(page)}
        onOpenActivityFullPage={() => openLegacyScreen('activity')}
        onAgentStatus={() => reportDiagnostic('agent_home_status', '')}
        releaseNotesUrl={currentReleaseNotesUrl}
        onOpenReleaseNotes={() => void openResponseLink(currentReleaseNotesUrl)}
        onUpdateFound={(result) =>
          setAvailableUpdate(result.check_succeeded && result.available_version ? result : null)
        }
        sharedAudit={sharedAudit}
        requestedEntry={requestedSettingsEntry}
      />
      <CommandPalette
        open={commandPaletteOpen}
        items={paletteItems}
        recentIds={recentCommands}
        onClose={() => setCommandPaletteOpen(false)}
        onExecute={(item) => void executePaletteItem(item)}
      />
    </div>
  );
}
