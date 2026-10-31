import { useRef, type FormEvent } from 'react';
import type { ConversationSummary } from '../../core/chat';
import {
  applyHomeSuggestion,
  submitHomeMessage,
  type HomeChecklistItem,
  type HomeSuggestion,
} from '../../core/home';
import { MessageComposer } from '../MessageComposer';
import type { ProfileData } from '../Profile';
import { PetSprite } from '../PetSprite';
import type { PetAnimationState } from '../../core/pets';
import { normalizeProfileData } from '../../core/identity';

export function Overview({
  draft,
  setDraft,
  busy,
  keyReady,
  profile,
  selectedPet,
  petState,
  reduceMotion,
  reduceMascot,
  suggestions,
  conversations,
  checklist,
  pendingApprovalCount,
  skillsLoadError,
  onStartConversation,
  onSendMessage,
  onResumeConversation,
  onResolveChecklistItem,
  onOpenApprovals,
  onRetrySkills,
}: {
  draft: string;
  setDraft: (value: string) => void;
  busy: boolean;
  keyReady: boolean;
  profile: ProfileData;
  selectedPet: string;
  petState: PetAnimationState;
  reduceMotion: boolean;
  reduceMascot: boolean;
  suggestions: HomeSuggestion[];
  conversations: ConversationSummary[];
  checklist: HomeChecklistItem[];
  pendingApprovalCount: number;
  skillsLoadError?: string;
  onStartConversation: () => void;
  onSendMessage: (text: string) => Promise<boolean>;
  onResumeConversation: (id: string) => void;
  onResolveChecklistItem: (item: HomeChecklistItem) => void;
  onOpenApprovals: () => void;
  onRetrySkills?: () => void;
}) {
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const recentConversations = conversations.slice(0, 3);
  const safeProfile = normalizeProfileData(profile);
  const name = safeProfile.nickname.trim() || safeProfile.displayName.trim();

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await submitHomeMessage(draft, busy, onStartConversation, onSendMessage, keyReady);
  }

  function selectSuggestion(suggestion: HomeSuggestion) {
    applyHomeSuggestion(suggestion, setDraft);
    window.requestAnimationFrame(() => composerRef.current?.focus());
  }

  return (
    <main className="page overview-page">
      {skillsLoadError && (
        <div className="error-banner" role="alert">
          <p>{skillsLoadError}</p>
          <button type="button" className="secondary-button" onClick={onRetrySkills}>
            Tentar de novo
          </button>
        </div>
      )}
      {pendingApprovalCount > 0 && (
        <button
          className="home-approval-banner"
          type="button"
          onClick={onOpenApprovals}
          aria-label={`${pendingApprovalCount} ${pendingApprovalCount === 1 ? 'aprovação pendente' : 'aprovações pendentes'}; abrir janela de aprovação`}
        >
          <span aria-hidden="true">{pendingApprovalCount}</span>
          {pendingApprovalCount === 1
            ? 'Ação aguardando sua aprovação'
            : 'Ações aguardando sua aprovação'}
          <span className="home-approval-open">Abrir aprovações</span>
        </button>
      )}

      <section className="home-focus" aria-labelledby="home-title">
        <PetSprite
          estado={petState}
          petId={selectedPet}
          reduceMotion={reduceMotion}
          staticImage={reduceMascot}
          className="home-pet"
        />
        <h1 id="home-title">O que vamos fazer hoje?</h1>
        <p className="home-greeting">{name ? `Olá, ${name}.` : 'Olá.'}</p>
        <MessageComposer
          draft={draft}
          setDraft={setDraft}
          busy={busy}
          canSend={keyReady}
          onSubmit={submit}
          textareaRef={composerRef}
          className="composer home-message-composer"
          sendButtonClassName="primary-button home-send-button"
        />
        {suggestions.length > 0 && (
          <div className="home-suggestions" aria-label="Sugestões baseadas no que está disponível">
            {suggestions.map((suggestion) => (
              <button
                key={suggestion.id}
                type="button"
                className="home-suggestion"
                onClick={() => selectSuggestion(suggestion)}
                title="Preencher a mensagem para você revisar"
              >
                {suggestion.label}
              </button>
            ))}
          </div>
        )}
      </section>

      <div className="home-panels home-panels--centered">
        {checklist.length > 0 && (
          <section className="home-checklist" aria-labelledby="home-checklist-title">
            <h2 id="home-checklist-title">Para começar</h2>
            <ul>
              {checklist.map((item) => (
                <li key={item.id}>
                  <div>
                    <strong>{item.title}</strong>
                    <p>{item.detail}</p>
                  </div>
                  <button type="button" onClick={() => onResolveChecklistItem(item)}>
                    Resolver
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
        <section className="home-resume" aria-labelledby="home-resume-title">
          <h2 id="home-resume-title">Retomar</h2>
          {recentConversations.length > 0 ? (
            <ul className="home-resume-grid">
              {recentConversations.map((conversation) => (
                <li key={conversation.id}>
                  <button
                    type="button"
                    onClick={() => onResumeConversation(conversation.id)}
                    aria-label={`Retomar ${conversation.title}`}
                  >
                    <strong>{conversation.title}</strong>
                    {conversation.preview && <span>{conversation.preview}</span>}
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="home-resume-empty">As conversas recentes aparecerão aqui.</p>
          )}
        </section>
      </div>
    </main>
  );
}
