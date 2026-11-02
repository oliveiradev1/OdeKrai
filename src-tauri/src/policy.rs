use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    SemprePerguntar,
    AprovarPorMim,
    ControleTotal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Leitura,
    EscritaReversivel,
    ComunicacaoExterna,
    DestrutivoFinanceiro,
    ExecucaoDeCodigo,
    CapturaSensivel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "source")]
pub enum Origin {
    User,
    Model,
    ExternalContent(String),
}

impl Origin {
    pub fn is_tainted(&self) -> bool {
        matches!(self, Self::ExternalContent(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow(&'static str),
    AskUser(&'static str),
    Deny(&'static str),
}

pub fn decide(
    mode: Mode,
    risk: Risk,
    origin: &Origin,
    inside_workspace: bool,
    fixed_block: Option<&'static str>,
    permissive_override: bool,
) -> Decision {
    if let Some(rule) = fixed_block {
        return Decision::Deny(rule);
    }
    if risk == Risk::ExecucaoDeCodigo {
        return Decision::AskUser("code_execution_requires_approval");
    }
    if origin.is_tainted() && risk != Risk::Leitura {
        return Decision::AskUser("origin_tainted");
    }
    if risk == Risk::CapturaSensivel {
        return Decision::AskUser("sensitive_capture_requires_approval");
    }
    let base = match risk {
        Risk::Leitura if inside_workspace => Decision::Allow("workspace_read"),
        Risk::Leitura => Decision::Deny("outside_workspace"),
        Risk::EscritaReversivel => Decision::AskUser("write_requires_approval"),
        Risk::ComunicacaoExterna => {
            Decision::AskUser("external_communication_requires_approval")
        }
        Risk::DestrutivoFinanceiro => Decision::AskUser("destructive_action_requires_approval"),
        Risk::ExecucaoDeCodigo => Decision::AskUser("code_execution_requires_approval"),
        Risk::CapturaSensivel => Decision::AskUser("sensitive_capture_requires_approval"),
    };
    // All writes, external actions and sensitive operations require the approval window.
    // The selected permission mode and per-tool overrides cannot bypass this boundary.
    let _ = (mode, permissive_override);
    base
}

pub fn decide_with_taint(
    mode: Mode,
    risk: Risk,
    origin: &Origin,
    inside_workspace: bool,
    fixed_block: Option<&'static str>,
    permissive_override: bool,
    tainted: bool,
) -> Decision {
    if tainted && risk != Risk::Leitura && fixed_block.is_none() {
        return Decision::AskUser("origin_tainted");
    }
    decide(
        mode,
        risk,
        origin,
        inside_workspace,
        fixed_block,
        permissive_override,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_mode_risk_origin_and_path_matrix() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            for risk in [
                Risk::Leitura,
                Risk::EscritaReversivel,
                Risk::ComunicacaoExterna,
                Risk::DestrutivoFinanceiro,
                Risk::ExecucaoDeCodigo,
                Risk::CapturaSensivel,
            ] {
                for origin in [
                    Origin::User,
                    Origin::Model,
                    Origin::ExternalContent("mail".into()),
                ] {
                    for inside in [false, true] {
                        let decision = decide(mode, risk, &origin, inside, None, false);
                        let expected = if risk == Risk::ExecucaoDeCodigo {
                            Decision::AskUser("code_execution_requires_approval")
                        } else if risk != Risk::Leitura && origin.is_tainted() {
                            Decision::AskUser("origin_tainted")
                        } else if risk == Risk::CapturaSensivel {
                            Decision::AskUser("sensitive_capture_requires_approval")
                        } else if risk == Risk::Leitura {
                            if inside {
                                Decision::Allow("workspace_read")
                            } else {
                                Decision::Deny("outside_workspace")
                            }
                        } else {
                            Decision::AskUser(match risk {
                                Risk::EscritaReversivel => "write_requires_approval",
                                Risk::ComunicacaoExterna => "external_communication_requires_approval",
                                Risk::DestrutivoFinanceiro => "destructive_action_requires_approval",
                                Risk::ExecucaoDeCodigo => "code_execution_requires_approval",
                                Risk::CapturaSensivel => "sensitive_capture_requires_approval",
                                Risk::Leitura => unreachable!(),
                            })
                        };
                        assert_eq!(
                            decision, expected,
                            "{mode:?} {risk:?} {origin:?} inside={inside}: {decision:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(
            decide(
                Mode::SemprePerguntar,
                Risk::EscritaReversivel,
                &Origin::User,
                true,
                None,
                false
            ),
            Decision::AskUser("write_requires_approval")
        );
        assert_eq!(
            decide(
                Mode::AprovarPorMim,
                Risk::EscritaReversivel,
                &Origin::User,
                true,
                None,
                false
            ),
            Decision::AskUser("write_requires_approval")
        );
        assert_eq!(
            decide(
                Mode::ControleTotal,
                Risk::DestrutivoFinanceiro,
                &Origin::User,
                true,
                None,
                false
            ),
            Decision::AskUser("destructive_action_requires_approval")
        );
        assert_eq!(
            decide(
                Mode::ControleTotal,
                Risk::DestrutivoFinanceiro,
                &Origin::User,
                false,
                None,
                false
            ),
            Decision::AskUser("destructive_action_requires_approval")
        );
    }

    #[test]
    fn external_communication_requires_approval_in_every_mode_even_with_override() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            assert!(
                matches!(
                    decide(
                        mode,
                        Risk::ComunicacaoExterna,
                        &Origin::User,
                        false,
                        None,
                        true,
                    ),
                    Decision::AskUser(_)
                ),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn blocks_override_every_mode_and_tainted_actions_always_ask() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            assert_eq!(
                decide(
                    mode,
                    Risk::EscritaReversivel,
                    &Origin::User,
                    true,
                    Some("fixed_no"),
                    true
                ),
                Decision::Deny("fixed_no")
            );
            assert_eq!(
                decide(
                    mode,
                    Risk::ComunicacaoExterna,
                    &Origin::ExternalContent("web".into()),
                    true,
                    None,
                    true
                ),
                Decision::AskUser("origin_tainted")
            );
        }
    }

    #[test]
    fn permissive_override_cannot_skip_write_approval() {
        assert_eq!(
            decide(
                Mode::SemprePerguntar,
                Risk::ComunicacaoExterna,
                &Origin::User,
                true,
                None,
                true
            ),
            Decision::AskUser("external_communication_requires_approval")
        );
        assert_eq!(
            decide(
                Mode::SemprePerguntar,
                Risk::Leitura,
                &Origin::User,
                false,
                None,
                true
            ),
            Decision::Deny("outside_workspace")
        );
        assert_eq!(
            decide(
                Mode::SemprePerguntar,
                Risk::DestrutivoFinanceiro,
                &Origin::User,
                false,
                None,
                true
            ),
            Decision::AskUser("destructive_action_requires_approval")
        );
    }

    #[test]
    fn every_write_risk_requires_the_approval_window_in_all_modes() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            for risk in [
                Risk::EscritaReversivel,
                Risk::ComunicacaoExterna,
                Risk::DestrutivoFinanceiro,
                Risk::ExecucaoDeCodigo,
                Risk::CapturaSensivel,
            ] {
                assert!(
                    matches!(
                        decide(mode, risk, &Origin::Model, true, None, true),
                        Decision::AskUser(_)
                    ),
                    "{mode:?} {risk:?}"
                );
            }
        }
    }

    #[test]
    fn tainted_tool_result_requires_approval_even_in_total_mode_and_with_override() {
        assert_eq!(
            decide_with_taint(
                Mode::ControleTotal,
                Risk::EscritaReversivel,
                &Origin::Model,
                true,
                None,
                true,
                true,
            ),
            Decision::AskUser("origin_tainted")
        );
    }

    #[test]
    fn code_execution_always_requires_approval_even_with_permissive_override() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            assert_eq!(
                decide(
                    mode,
                    Risk::ExecucaoDeCodigo,
                    &Origin::User,
                    true,
                    None,
                    true,
                ),
                Decision::AskUser("code_execution_requires_approval")
            );
        }
    }

    #[test]
    fn sensitive_capture_always_requires_approval_in_every_mode() {
        for mode in [
            Mode::SemprePerguntar,
            Mode::AprovarPorMim,
            Mode::ControleTotal,
        ] {
            assert_eq!(
                decide(mode, Risk::CapturaSensivel, &Origin::User, true, None, true,),
                Decision::AskUser("sensitive_capture_requires_approval")
            );
            assert_eq!(
                decide(
                    mode,
                    Risk::CapturaSensivel,
                    &Origin::User,
                    false,
                    None,
                    true,
                ),
                Decision::AskUser("sensitive_capture_requires_approval")
            );
            assert_eq!(
                decide(
                    mode,
                    Risk::CapturaSensivel,
                    &Origin::ExternalContent("janela de conversa".into()),
                    true,
                    None,
                    true,
                ),
                Decision::AskUser("origin_tainted")
            );
        }
    }
}
