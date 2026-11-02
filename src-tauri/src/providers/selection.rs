#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TaskKind {
    Conversation,
    Research,
    Code,
    Files,
    Agenda,
    Vision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TaskNeeds {
    pub tools: bool,
    pub vision: bool,
    pub context_tokens: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CandidateCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub context_tokens: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CandidateAvailability {
    pub credential: bool,
    pub quota: bool,
    pub recent_failure: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProviderCandidate {
    pub id: String,
    pub capabilities: CandidateCapabilities,
    pub availability: CandidateAvailability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProviderChoice {
    pub index: usize,
    pub reason: &'static str,
}

fn failure_states() -> &'static Mutex<HashMap<String, FailureState>> {
    PROVIDER_FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn failure_key(provider_id: &str, model: &str) -> String {
    format!("{provider_id}\0{model}")
}

pub(super) fn is_recent_failure(provider_id: &str, model: &str) -> bool {
    failure_states()
        .lock()
        .ok()
        .and_then(|states| states.get(&failure_key(provider_id, model)).copied())
        .is_some_and(|state| Instant::now() < state.recent_failure_until)
}

pub(super) fn is_quota_available(provider_id: &str, model: &str) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    failure_states()
        .lock()
        .ok()
        .and_then(|states| states.get(&failure_key(provider_id, model)).copied())
        .and_then(|state| state.quota_blocked_until_utc)
        .is_none_or(|until| now >= until)
}

pub(super) fn record_result(provider_id: &str, model: &str, result: Result<(), &str>) {
    let key = failure_key(provider_id, model);
    let Ok(mut states) = failure_states().lock() else {
        return;
    };
    let instant_now = Instant::now();
    let utc_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    states.retain(|_, state| {
        instant_now < state.recent_failure_until
            || state
                .quota_blocked_until_utc
                .is_some_and(|until| utc_now < until)
    });
    match result {
        Ok(()) => {
            states.remove(&key);
        }
        Err(error) if super::failover::transient_failure_reason(error).is_some() => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let quota_blocked_until_utc = error
                .to_ascii_lowercase()
                .contains("cota ou o saldo")
                .then_some((now / 86_400 + 1) * 86_400);
            states.insert(
                key,
                FailureState {
                    recent_failure_until: Instant::now() + RECENT_FAILURE_COOLDOWN,
                    quota_blocked_until_utc,
                },
            );
        }
        Err(_) => {}
    }
}

pub(super) fn classify_task(text: &str, has_image: bool) -> TaskKind {
    if has_image {
        return TaskKind::Vision;
    }
    let normalized = text.to_lowercase();
    if normalized.contains("programar")
        && contains_any(
            &normalized,
            &[
                "reunião",
                "reuniao",
                "compromisso",
                "calendário",
                "calendario",
            ],
        )
    {
        return TaskKind::Agenda;
    }
    if contains_any(
        &normalized,
        &[
            "código",
            "codigo",
            "programar",
            "implementar",
            "corrigir bug",
            "refatorar",
            "script",
            "codex",
            "claude code",
        ],
    ) {
        TaskKind::Code
    } else if contains_any(
        &normalized,
        &[
            "agenda",
            "reunião",
            "reuniao",
            "compromisso",
            "calendário",
            "calendario",
            "lembrete",
        ],
    ) {
        TaskKind::Agenda
    } else if contains_any(
        &normalized,
        &[
            "arquivo",
            "pasta",
            "documento",
            "planilha",
            "ler o arquivo",
            "buscar no arquivo",
        ],
    ) {
        TaskKind::Files
    } else if contains_any(
        &normalized,
        &[
            "pesquise",
            "pesquisar",
            "pesquisa",
            "buscar na web",
            "fontes",
            "referências",
            "referencias",
        ],
    ) {
        TaskKind::Research
    } else {
        TaskKind::Conversation
    }
}

pub(super) fn task_needs(kind: TaskKind, context_tokens: Option<u32>) -> TaskNeeds {
    TaskNeeds {
        tools: kind != TaskKind::Conversation,
        vision: kind == TaskKind::Vision,
        context_tokens,
    }
}

pub(super) fn choose_provider(
    primary_id: &str,
    candidates: &[ProviderCandidate],
    needs: TaskNeeds,
) -> Option<ProviderChoice> {
    let primary_index = candidates
        .iter()
        .position(|candidate| candidate.id == primary_id)?;

    // Requests without a task-specific requirement preserve the selected provider.
    if !needs.tools && !needs.vision && needs.context_tokens.is_none() {
        return Some(ProviderChoice {
            index: primary_index,
            reason:
                "nenhum requisito específico de capacidade se aplica; mantendo o provedor principal",
        });
    }

    let eligible = |candidate: &ProviderCandidate| {
        candidate.availability.credential
            && candidate.availability.quota
            && !candidate.availability.recent_failure
            && (!needs.tools || candidate.capabilities.tools)
            && (!needs.vision || candidate.capabilities.vision)
            && needs.context_tokens.is_none_or(|required| {
                candidate
                    .capabilities
                    .context_tokens
                    .is_some_and(|available| available >= required)
            })
    };

    if eligible(&candidates[primary_index]) {
        return Some(ProviderChoice {
            index: primary_index,
            reason: "o provedor principal atende aos requisitos e está disponível",
        });
    }

    if let Some((index, _)) = candidates
        .iter()
        .enumerate()
        .find(|(index, candidate)| *index != primary_index && eligible(candidate))
    {
        return Some(ProviderChoice {
            index,
            reason: "o provedor principal não atende aos requisitos ou está indisponível; esta alternativa tem a capacidade necessária e está disponível",
        });
    }

    Some(ProviderChoice {
        index: primary_index,
        reason: "nenhuma alternativa elegível está disponível; mantendo o provedor principal",
    })
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, tools: bool, credential: bool) -> ProviderCandidate {
        ProviderCandidate {
            id: id.into(),
            capabilities: CandidateCapabilities {
                tools,
                vision: false,
                context_tokens: None,
            },
            availability: CandidateAvailability {
                credential,
                quota: true,
                recent_failure: false,
            },
        }
    }

    #[test]
    fn code_task_prefers_available_provider_with_tools_when_primary_lacks_them() {
        let candidates = [
            candidate("primary", false, true),
            candidate("tools", true, true),
        ];
        let choice = choose_provider("primary", &candidates, task_needs(TaskKind::Code, None))
            .expect("primary exists");
        assert_eq!(candidates[choice.index].id, "tools");
        assert!(choice.reason.contains("capacidade necessária"));
    }

    #[test]
    fn recently_failed_provider_is_skipped_when_an_alternative_is_eligible() {
        let mut primary = candidate("primary", true, true);
        primary.availability.recent_failure = true;
        let candidates = [primary, candidate("backup", true, true)];
        let choice = choose_provider("primary", &candidates, task_needs(TaskKind::Code, None))
            .expect("primary exists");
        assert_eq!(candidates[choice.index].id, "backup");
    }

    #[test]
    fn primary_is_kept_when_no_alternative_is_available() {
        let candidates = [
            candidate("primary", true, true),
            candidate("backup", true, false),
        ];
        let choice = choose_provider("primary", &candidates, task_needs(TaskKind::Code, None))
            .expect("primary exists");
        assert_eq!(candidates[choice.index].id, "primary");
        assert!(choice.reason.contains("provedor principal"));
    }

    #[test]
    fn no_specific_requirement_preserves_primary_even_when_an_alternative_is_healthier() {
        let mut primary = candidate("primary", false, false);
        primary.availability.quota = false;
        let candidates = [primary, candidate("backup", true, true)];
        let choice =
            choose_provider("primary", &candidates, TaskNeeds::default()).expect("primary exists");
        assert_eq!(candidates[choice.index].id, "primary");
    }

    #[test]
    fn context_requirement_uses_only_declared_capacity() {
        let mut unknown = candidate("unknown", true, true);
        unknown.capabilities.context_tokens = None;
        let mut enough = candidate("enough", true, true);
        enough.capabilities.context_tokens = Some(64_000);
        let candidates = [unknown, enough];
        let choice = choose_provider(
            "unknown",
            &candidates,
            TaskNeeds {
                tools: true,
                vision: false,
                context_tokens: Some(32_000),
            },
        )
        .expect("primary exists");
        assert_eq!(candidates[choice.index].id, "enough");
    }

    #[test]
    fn quota_and_vision_are_hard_requirements_for_alternatives() {
        let mut no_quota = candidate("no-quota", true, true);
        no_quota.availability.quota = false;
        let mut vision = candidate("vision", true, true);
        vision.capabilities.vision = true;
        let candidates = [candidate("primary", false, true), no_quota, vision];
        let choice = choose_provider("primary", &candidates, task_needs(TaskKind::Vision, None))
            .expect("primary exists");
        assert_eq!(candidates[choice.index].id, "vision");
    }

    #[test]
    fn task_classifier_uses_explicit_content_type_hints() {
        assert_eq!(classify_task("Implemente um script", false), TaskKind::Code);
        assert_eq!(
            classify_task("Pesquise fontes atuais", false),
            TaskKind::Research
        );
        assert_eq!(classify_task("Marque uma reunião", false), TaskKind::Agenda);
        assert_eq!(
            classify_task("Programar uma reunião amanhã", false),
            TaskKind::Agenda
        );
        assert_eq!(classify_task("Leia este arquivo", false), TaskKind::Files);
        assert_eq!(
            classify_task("O que aparece nesta imagem?", true),
            TaskKind::Vision
        );
        assert_eq!(classify_task("Olá", false), TaskKind::Conversation);
    }

    #[test]
    fn recent_transient_failure_is_recorded_and_success_clears_it() {
        let provider = format!("test-provider-{}", std::process::id());
        let model = "selection-health-test";
        record_result(&provider, model, Err("HTTP 503"));
        assert!(is_recent_failure(&provider, model));
        record_result(&provider, model, Ok(()));
        assert!(!is_recent_failure(&provider, model));
    }

    #[test]
    fn quota_failure_is_unavailable_until_the_next_utc_day() {
        let provider = format!("test-quota-provider-{}", std::process::id());
        let model = "selection-quota-test";
        record_result(
            &provider,
            model,
            Err("A cota ou o saldo da conta acabou (HTTP 429)."),
        );
        assert!(!is_quota_available(&provider, model));
    }
}
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const RECENT_FAILURE_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
struct FailureState {
    recent_failure_until: Instant,
    quota_blocked_until_utc: Option<u64>,
}

static PROVIDER_FAILURES: OnceLock<Mutex<HashMap<String, FailureState>>> = OnceLock::new();
