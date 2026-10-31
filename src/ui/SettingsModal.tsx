import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppSettings } from '../core/settings';
import { providerById } from '../core/providers';
import type { CatalogSkill } from '../core/skills-catalog';
import {
  filterSettingsIndex,
  filterSettingsMenu,
  SETTINGS_MENU,
} from '../core/settings-navigation';
import type { SettingsSection } from '../core/navigation';
import type { ProfileData } from './Profile';
import type { Section } from './screens/types';
import type { PerformanceModeView } from './usePerformanceMode';
import { Button } from './DesignSystem';
import { Modal } from './Modal';
import { ScreenErrorBoundary } from './ScreenErrorBoundary';

const SettingsScreen = lazy(() =>
  import('./screens/Settings').then((module) => ({ default: module.SettingsScreen })),
);
const AboutSettings = lazy(() =>
  import('./screens/AboutSettings').then((module) => ({ default: module.AboutSettings })),
);
const IdentityScreen = lazy(() =>
  import('./screens/Identity').then((module) => ({ default: module.IdentityScreen })),
);
const AssistantScreen = lazy(() =>
  import('./screens/Assistant').then((module) => ({ default: module.AssistantScreen })),
);
const ComputerScreen = lazy(() =>
  import('./screens/Computer').then((module) => ({ default: module.ComputerScreen })),
);
const SecurityCenter = lazy(() =>
  import('./SecurityCenter').then((module) => ({ default: module.SecurityCenter })),
);
import type { AuditEntry } from './SecurityCenter';
import { CloudSettings } from './CloudSettings';

export function SettingsModal({
  open,
  section,
  onClose,
  onSelectSection,
  settings,
  update,
  performanceStatus,
  profile,
  setProfile,
  petVisible,
  setPetVisible,
  selectedPet,
  onSelectPet,
  onError,
  onDeleteAllHistory,
  onOpenPage,
  onOpenActivityFullPage,
  onAgentStatus,
  releaseNotesUrl,
  onOpenReleaseNotes,
  onUpdateFound,
  sharedAudit,
  requestedEntry,
}: {
  open: boolean;
  section: SettingsSection;
  onClose: () => void;
  onSelectSection: (section: SettingsSection) => void;
  settings: AppSettings;
  update: (settings: AppSettings) => void;
  performanceStatus: PerformanceModeView;
  profile: ProfileData;
  setProfile: (profile: ProfileData) => void;
  petVisible: boolean;
  setPetVisible: (visible: boolean) => void;
  selectedPet: string;
  onSelectPet: (petId: string) => void;
  onError: (message: string) => void;
  onDeleteAllHistory: () => Promise<boolean>;
  onOpenPage: (page: Extract<Section, 'skills' | 'providers'>) => void;
  onOpenActivityFullPage: () => void;
  onAgentStatus: () => void;
  releaseNotesUrl: string;
  onOpenReleaseNotes: () => void;
  onUpdateFound: (result: {
    current_version: string;
    available_version: string | null;
    release_url: string | null;
    notes_summary: string | null;
    check_succeeded: boolean;
  }) => void;
  sharedAudit?: {
    entries: AuditEntry[];
    verified: boolean | null;
    loading: boolean;
    unavailable?: boolean;
  };
  requestedEntry?: { section: SettingsSection; anchor: string; requestId: number } | null;
}) {
  const [query, setQuery] = useState('');
  const [pendingAnchor, setPendingAnchor] = useState<string | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const lastHandledPaletteRequest = useRef(0);
  const menu = useMemo(() => filterSettingsMenu(query), [query]);
  const results = useMemo(() => filterSettingsIndex(query), [query]);
  const activeTitle = SETTINGS_MENU.find((item) => item.id === section)?.title ?? 'Configurações';

  useEffect(() => {
    if (!pendingAnchor || !contentRef.current) return;
    const container = contentRef.current;
    const reveal = () => {
      const anchor = container.querySelector<HTMLElement>(`#${CSS.escape(pendingAnchor)}`);
      if (!anchor) return false;
      anchor.scrollIntoView({ block: 'start', behavior: 'auto' });
      setPendingAnchor(null);
      return true;
    };
    if (reveal()) return;
    const observer = new MutationObserver(() => {
      if (reveal()) observer.disconnect();
    });
    observer.observe(container, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [pendingAnchor, section]);

  useEffect(() => {
    if (
      open &&
      requestedEntry?.section === section &&
      requestedEntry.requestId > lastHandledPaletteRequest.current
    ) {
      lastHandledPaletteRequest.current = requestedEntry.requestId;
      setPendingAnchor(requestedEntry.anchor);
    }
  }, [open, requestedEntry, section]);

  function chooseSection(next: SettingsSection, anchor?: string) {
    setPendingAnchor(anchor ?? null);
    onSelectSection(next);
  }

  const content = (() => {
    switch (section) {
      case 'geral':
        return (
          <SettingsScreen
            settings={settings}
            update={update}
            performanceStatus={performanceStatus}
            onOpenPermissions={() => chooseSection('privacidade')}
            embedded
          />
        );
      case 'sobre':
        return (
          <AboutSettings
            releaseNotesUrl={releaseNotesUrl}
            onOpenReleaseNotes={onOpenReleaseNotes}
            settings={settings}
            update={update}
            onUpdateFound={onUpdateFound}
          />
        );
      case 'perfil-memoria':
        return (
          <IdentityScreen
            initialName={profile.displayName}
            initialNickname={profile.nickname}
            profileValue={profile}
            onProfileChange={setProfile}
            onStatus={onAgentStatus}
            embedded
          />
        );
      case 'assistente':
        return (
          <AssistantScreen
            settings={settings}
            update={update}
            petVisible={petVisible}
            onPetVisibilityChange={setPetVisible}
            onError={onError}
            selectedPet={selectedPet}
            onSelectPet={onSelectPet}
            embedded
          />
        );
      case 'computador':
        return <ComputerScreen embedded />;
      case 'privacidade':
        return (
          <SecurityCenter
            page="permissions"
            embedded
            onDeleteAllHistory={onDeleteAllHistory}
            appSettings={settings}
            updateAppSettings={update}
          />
        );
      case 'atividade':
        return (
          <SecurityCenter
            page="activity"
            embedded
            onOpenFullPage={onOpenActivityFullPage}
            sharedAudit={sharedAudit}
          />
        );
      case 'nuvem':
        return <CloudSettings />;
      case 'habilidades':
        return <SkillsSettingsSummary onOpenPage={() => onOpenPage('skills')} />;
      case 'conectores':
        return (
          <ConnectorsSettingsSummary
            settings={settings}
            onOpenPage={() => onOpenPage('providers')}
          />
        );
    }
  })();

  return (
    <Modal open={open} title="Configurações" onClose={onClose}>
      <div className="settings-modal-content">
        <label className="settings-modal-search">
          <span>Pesquisar configurações</span>
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Ex.: microfone, memória, permissões"
            autoFocus
          />
        </label>
        {query.trim() && (
          <div className="settings-modal-results" aria-label="Resultados da pesquisa">
            {results.length ? (
              results.map((result) => (
                <button
                  type="button"
                  key={result.anchor}
                  onClick={() => chooseSection(result.section, result.anchor)}
                >
                  <span>{result.title}</span>
                  <small>{SETTINGS_MENU.find((item) => item.id === result.section)?.title}</small>
                </button>
              ))
            ) : (
              <p role="status">Nenhuma opção encontrada.</p>
            )}
          </div>
        )}
        <div className="settings-modal-layout">
          <nav className="settings-modal-menu" aria-label="Seções de configurações">
            <div className="settings-modal-menu-wide">
              {menu.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  className={item.id === section ? 'active' : ''}
                  aria-current={item.id === section ? 'page' : undefined}
                  onClick={() => chooseSection(item.id)}
                >
                  {item.title}
                </button>
              ))}
            </div>
            <label className="settings-modal-menu-narrow">
              <span>Seção</span>
              <select
                value={section}
                onChange={(event) => chooseSection(event.target.value as SettingsSection)}
              >
                {!menu.some((item) => item.id === section) && (
                  <option value={section}>{activeTitle}</option>
                )}
                {menu.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.title}
                  </option>
                ))}
              </select>
            </label>
          </nav>
          <section className="settings-modal-panel" aria-label={activeTitle} ref={contentRef}>
            <ScreenErrorBoundary key={section}>
              <Suspense fallback={<p role="status">Carregando configurações…</p>}>
                {content}
              </Suspense>
            </ScreenErrorBoundary>
          </section>
        </div>
      </div>
    </Modal>
  );
}

export function summarizeSkillsStatus(skills: Pick<CatalogSkill, 'status'>[]) {
  return {
    total: skills.length,
    active: skills.filter((skill) => skill.status === 'installed').length,
  };
}

export function summarizeConnectors(settings: AppSettings) {
  const providerId = settings.activeProviderId;
  const configuration = settings.providers[providerId];
  return {
    providerName: providerId
      ? (providerById(providerId)?.name ?? 'Provedor selecionado')
      : 'Nenhum provedor principal selecionado',
    model: configuration?.model.trim() || null,
    enabled: configuration?.enabled !== false,
    fallbackCount: settings.fallbackProviderIds.filter(
      (fallbackId) =>
        settings.providers[fallbackId]?.enabled !== false &&
        Boolean(settings.providers[fallbackId]?.model.trim()),
    ).length,
  };
}

function SettingsDestinationSummary({
  id,
  title,
  description,
  status,
  actionLabel,
  onOpenPage,
}: {
  id: string;
  title: string;
  description: string;
  status: string;
  actionLabel: string;
  onOpenPage: () => void;
}) {
  return (
    <div className="settings-embedded-content">
      <section id={id} className="settings-panel settings-section">
        <h2>{title}</h2>
        <p>{description}</p>
        <p role="status">{status}</p>
        <Button type="button" variant="secondary" onClick={onOpenPage}>
          {actionLabel}
        </Button>
      </section>
    </div>
  );
}

export function SkillsSettingsSummary({ onOpenPage }: { onOpenPage: () => void }) {
  const [summary, setSummary] = useState<
    | { state: 'loading' }
    | { state: 'ready'; total: number; active: number }
    | { state: 'unavailable' }
    | { state: 'error' }
  >({ state: 'loading' });

  useEffect(() => {
    let current = true;
    if (!isTauri()) {
      setSummary({ state: 'unavailable' });
      return () => {
        current = false;
      };
    }
    setSummary({ state: 'loading' });
    void invoke<CatalogSkill[]>('get_skills')
      .then((skills) => {
        if (current) setSummary({ state: 'ready', ...summarizeSkillsStatus(skills) });
      })
      .catch(() => {
        if (current) setSummary({ state: 'error' });
      });
    return () => {
      current = false;
    };
  }, []);

  const status =
    summary.state === 'loading'
      ? 'Consultando o catálogo de habilidades…'
      : summary.state === 'unavailable'
        ? 'O catálogo está disponível no aplicativo OdeKrai para desktop.'
        : summary.state === 'error'
          ? 'Não foi possível consultar o catálogo. Abra a central para verificar novamente.'
          : summary.total === 0
            ? 'Nenhuma habilidade está registrada nesta versão.'
            : `${summary.active} de ${summary.total} habilidades estão ativas no catálogo.`;

  return (
    <SettingsDestinationSummary
      id="skills-summary"
      title="Habilidades"
      description="A busca, a ativação por família e a configuração das habilidades ficam na central de Habilidades. A disponibilidade no agente também depende dos requisitos de cada ferramenta."
      status={status}
      actionLabel="Abrir central de habilidades"
      onOpenPage={onOpenPage}
    />
  );
}

export function ConnectorsSettingsSummary({
  settings,
  onOpenPage,
}: {
  settings: AppSettings;
  onOpenPage: () => void;
}) {
  const summary = summarizeConnectors(settings);
  const status = !settings.activeProviderId
    ? 'Nenhum provedor principal foi selecionado.'
    : `${summary.providerName} · ${summary.enabled ? 'habilitado' : 'desabilitado'} · ${summary.model ? `modelo ${summary.model}` : 'modelo não selecionado'} · ${summary.fallbackCount} provedores reserva configurados.`;

  return (
    <SettingsDestinationSummary
      id="connectors-summary"
      title="Conectores de IA"
      description="A configuração de credenciais, modelos, testes de conexão e provedores reserva é feita na central de Conectores de IA."
      status={status}
      actionLabel="Abrir central de conectores"
      onOpenPage={onOpenPage}
    />
  );
}
