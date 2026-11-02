# Auditoria autônoma v0.18 — checkpoints

## Baseline

- Data da auditoria: 2026-11-02.
- Repositório: `oliveiradev1/OdeKrai`.
- Branch de trabalho local: `audit-v018-codex`, criada de `dev-mcp`.
- HEAD inicial de `dev-mcp`: `3765b148be0985af0df645c112de9488ab3d4abf`.
- HEAD inicial de `main` remoto: `57db155a0ca278038ba1e5b2a85144f0e6a9de47`.
- Base comum: `2cda8b55afdf09d62976b12e82c1ed76d9bad496`.
- Comparação remota via API GitHub: `main...dev-mcp` = 36 commits à frente e 1 atrás. O único commit exclusivo de `main` é `57db155`, merge assinado do lote MCP (`Merge lote-mcp na main...`), cujo segundo pai é a base comum e primeiro pai `22cd426`; não foi feito merge local.
- `git fetch origin` não concluiu: proxy indisponível (`Failed to connect to proxy port 8080`). As referências locais `main` e `origin/*` não são atualizadas e não foram usadas para afirmar o estado remoto. A comparação acima veio do conector da API do GitHub.
- `git status --short` antes de editar: limpo. `git log -5 --oneline`: `3765b14`, `0734df8`, `17d59e4`, `08f92ca`, `316296e`.
- `git diff main...HEAD --stat` local: 120 arquivos, 18.801 inserções e 1.139 remoções, mas a ref local `main` está em `371cbd6`, atrás do HEAD e diferente da ref remota; esse número não é a comparação válida com a `main` atual.
- Versão do pacote permanece `0.17.0`; `docs/releases/v0.18.0.md` é nota em preparação.

## Registro de alterações

| ID      | ESTADO           | REPRODUÇÃO                                                                                                                                                      | CORREÇÃO                                                                                             | TESTE/COMANDO                                                                                              | COMMIT                | PENDÊNCIA                                                            |
| ------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------- | -------------------------------------------------------------------- |
| G00     | FEITO            | Conflito entre item 6 do `AGENTS.md` e autorização contínua identificado.                                                                                       | Regra passou a admitir apenas subfases autorizadas, preservando paradas obrigatórias e proibições.   | `npm run check:encoding` passou; `git diff --check` passou.                                                | `8d735e9`             | Nenhuma.                                                             |
| G01     | PARCIAL          | API GitHub comparou as refs atuais; fetch local falhou pelo proxy. Estado de implementação dos 75 itens anteriores e F0–F9 ainda não foi inventariado.          | Registro do baseline e da divergência atualizado nesta tabela; sem merge.                            | GitHub compare API; `git status`, `git log -5`, `git diff main...HEAD --stat`.                             | incluído em `8d735e9` | Completar a matriz rastreável por arquivo/commit.                    |
| G02     | FEITO LOCALMENTE | `verify.mjs` não incluía o guarda de comandos bloqueantes nem o teste mock de janelas; CI não executava `check:release-features`.                               | Adicionadas essas etapas em `verify`; CI agora executa `check:release-features`.                     | `npx vitest run scripts/verify.test.mjs` (3 testes passaram); Prettier, codificação e diff check passaram. | `94bd0a0`             | Verify completo não executado; Cargo não está disponível localmente. |
| G03     | PARCIAL          | A suíte frontend foi executada; testes de componente não iniciam WebView2. Duas suítes Rust de integração estão presentes, mas Cargo não existe neste ambiente. | Matriz de níveis e limites adicionada em `docs/qa/MATRIZ-DE-TESTES-V018.md`; jsdom não é E2E nativo. | `npm test -- --pool=threads --maxWorkers=1`: 87 arquivos, 512 testes passaram; Cargo não encontrado.       | `73ccdd0`             | Executar Rust e WebView2/Windows no ambiente correspondente.         |
| G04     | PARCIAL          | Nota conferida contra referências de commits, testes e limites; pacote permanece 0.17.0.                                                                        | Promessa de delegação funcional removida; nota registra recurso desativado.                          | Leitura de código/docs; Vitest 512/512 no HEAD `73ccdd0`.                                                  | `73ccdd0`             | Continuar checando claims; validar CI no HEAD final.                 |
| G05–Q05 | NÃO AVALIADOS    | Inventário detalhado será registrado conforme cada subfase for inspecionada.                                                                                    | —                                                                                                    | —                                                                                                          | —                     | 61 itens aguardam auditoria.                                         |

## Checkpoints

### G00 — governança contínua

Leitura integral de `AGENTS.md` e `docs/CODEX-CLOUD-INSTRUCTIONS.md`. As instruções agora permitem atravessar apenas as subfases expressamente autorizadas na solicitação atual; preservam casos de parada obrigatória, gates de segurança, aprovação, testes e a exigência independente de autorização para push/merge/tag/release/publicação. Não há afrouxamento de segurança.

Checkpoint `8d735e9`; `npm run check:encoding` e `git diff --check` passaram antes do commit.

### G01 — refs e baseline remoto

A confirmação remota por API é contemporânea a 2026-11-02: `main` (`57db155`) tem como único commit exclusivo o merge “Merge lote-mcp na main...” (pais `22cd426` e `2cda8b5`); `dev-mcp` tem 36 commits desde a base comum. Isso explica 36 à frente/1 atrás. A ref local `main` (`371cbd6`) é antiga; seu diff local com `HEAD` não serve como comparação remota. A matriz de resultados dos prompts antigos ainda está em levantamento e não deve ser interpretada como já validada.

### G02 — runner de validação

O runner executa as guardas e suítes sequencialmente e interrompe no primeiro erro. Foram acrescentados `check:blocking-commands` e `cargo test --test mock_window --features mock-window-tests`; o CI ganhou `check:release-features`, que já era parte do runner. A suíte do runner confirmou ordem e parada no primeiro erro (3 testes). O teste completo do runner não foi iniciado neste checkpoint.

### G03 — níveis efetivos de teste

O mapeamento por teste e comando está em [`MATRIZ-DE-TESTES-V018.md`](MATRIZ-DE-TESTES-V018.md). No HEAD `73ccdd0`, Vitest passou: 87 arquivos e 512 testes, executado com um worker para evitar o travamento observado no pool padrão. Os 18 casos de `src/ui/rendering-smoke.test.tsx` montam componentes em jsdom com comandos Tauri simulados; verificam renderização e algumas interações DOM, mas não iniciam Tauri nem WebView2. `src-tauri/tests/mock_window.rs` usa MockRuntime Tauri; `mcp_stdio.rs` usa processo de servidor falso. Nenhum foi executado aqui por falta de Cargo. Não há E2E de janela nativa comprovado nesta suíte. Windows, instalação, WebView2, DPI, permissões físicas e serviços com conta real continuam no roteiro manual.

### G04 — nota v0.18.0

A nota cita commits/testes para as mudanças listadas e não altera o número do pacote (permanece 0.17.0). A afirmação de delegação funcional foi removida; o recurso agora aparece em limitações como desativado, com fundamento e evidências de S01. Os resultados antigos de CI e `verify` estão identificados pelos SHAs históricos em que foram executados; não representam a validação do HEAD desta auditoria.

### S01 — isolamento de delegação de código

**Falha reproduzível por inspeção:** `code_delegate::command_args` usava `--full-auto` no Codex e `--dangerously-skip-permissions` no Claude; os próprios textos de aprovação reconheciam que o processo herda os privilégios da conta e que o worktree não é sandbox. A aprovação não impedia acesso do processo a arquivos, rede e credenciais do usuário.

**Correção aplicada:** delegação agora é bloqueada na função Rust, no estado de disponibilidade da habilidade e na prévia; a tela de configuração explica o motivo e não oferece ativação. Removi os flags de bypass da construção de argumentos e atualizei Segurança, Orquestrador e nota v0.18.0. Não afirmei isolamento por prompt ou worktree.

**Verificação:** testes de interface (3 arquivos, 52 testes) passaram, incluindo a mensagem e ausência de controles para iniciar. O teste Rust novo está escrito, mas `cargo test` não executou: `cargo: command not found`. Execução de CLI adversarial fora do diretório não foi tentada, pois a função está bloqueada e não devemos expor arquivos reais.
