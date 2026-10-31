# Testes e validação

## L11 — modo Desempenho

- `performance.test.ts` cobre os modos fixos e Automático com perfis de hardware simulados, incluindo os limites de 8 GiB e 4 núcleos lógicos. `performance-mode.test.ts` verifica os efeitos CSS e a sincronização da janela do pet.
- A medição de CPU em idle e das chamadas IPC por 60 s exige execução e instrumentação do aplicativo Tauri no Windows; permanece pendente neste ambiente.

## L10 — notas de versão no app

- `scripts/generate-release-notes.test.mjs` verifica extração, ordenação, validação de Markdown e ausência da nota atual; `release-notes.test.ts` cobre nova versão, versão já vista, primeira instalação, opção desligada e notas ausentes.
- `ReleaseNotesModal.test.tsx` verifica que a tela usa resumo e mudanças do manifesto local e lista todas as versões embutidas. `Settings.test.tsx` e `settings.test.ts` cobrem o toggle e sua persistência.
- A suíte completa e a build devem ser executadas em validação final; teste visual no Windows continua pendente.

## Paleta de comandos

`src/core/command-palette.test.ts` verifica busca aproximada, recência, destinos canônicos, despacho
dos tipos de ação, navegação com setas, contrato ARIA do combobox/listbox e reutilização do comando
Rust de modo de permissão com confirmação nativa. Os testes são locais; validação visual e de foco
no executável Windows permanece pendente.

## Fase 4 — cobertura parcial do agente

O loop de agente e o motor de políticas têm cobertura unitária, mas continuam pendentes cenários
integrados de multi-turno, failover sem repetição de ferramentas, propagação tainted de ToolResult,
cancelamento, concorrência por conversa e UI completa. A matriz detalhada está em
[`TRACEABILITY.md`](TRACEABILITY.md).

Após implementação, todos esses testes devem usar servidor/mock local e zero chamadas a APIs reais.
Validação visual no Windows deverá cobrir estados do chat, aprovação, parada e integração com
island/mascote.

## Regressões do adaptador OpenAI (Lote 6 — E3)

Os testes Rust cobrem mapeamento reversível de nomes internos de habilidades, nomes em histórico de
tool calls, contrato de nomes OpenAI, preservação do schema, chat sem ferramentas, classificações
HTTP simuladas (400, 401, 403, 429 e 500) sem ecoar corpo privado e rejeição de respostas vazias ou
inválidas. São testes locais; não usam credenciais nem APIs reais. Compatibilidade de um modelo de
terceiro pode depender de sua configuração e permanece sujeita à resposta real do endpoint.

## Lote 6 — E10: diagnósticos e avisos do WebView

`password-forms.test.ts` verifica campos de credenciais em formulários HTML; `favicon.test.ts`
confere a declaração e o cabeçalho ICO do recurso público copiado pelo Vite; `diagnostics.test.ts`
verifica sanitização de stack trace. `diagnostics.rs` testa redação de dados pessoais e gravações
concorrentes em linhas completas. Falhas de registro de listeners e chamadas assíncronas conhecidas
são tratadas nos componentes, e o listener global continua relatando erros sem cancelar o evento.

A busca no código não encontrou emissão própria de `OPTIONS` ou evento chamado `preflight`. O
workflow mantém DevTools fora das features padrão; o recurso `diagnostics` apenas habilita o DevTools
do Tauri. “Simulação” aparece no centro de Segurança e nas tools locais para descrever uma operação
que não foi executada. Portanto, esses termos não justificam alterar transporte ou remover os avisos
do inspetor. A requisição específica vista anteriormente não pôde ser capturada neste ambiente.

O favicon nativo listado em `tauri.conf.json` é ícone do pacote; o WebView usa o `favicon.ico` em
`public/`, copiado para `dist/` pelo Vite. O código de diagnóstico não registra payloads de erro;
referências locais apresentam seção, comando, categoria, resultado e ID de correlação, enquanto
detalhes sanitizados de stack ficam limitados ao painel local da interface.

## Verificações automatizadas

- `npm test`: chat, estados da ilha, persistência, carregador de sons, catálogo de sprites, mapeamento de estados e movimento reduzido.
- `npm run build`: TypeScript e bundle Vite.
- `npm run format:check`: estilo Prettier.
- `npm run check:debug-command`: confirma que declaração e registro do disparo manual estão limitados a `debug_assertions`.
- O workflow `CI` valida testes, formatação e build frontend no push/PR, mas não gera nem armazena instaladores Windows como artefatos do GitHub Actions.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: formatação Rust.
- `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`: compilação Rust para Windows.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: testes Rust de lógica e cabeçalhos de autenticação com servidor local simulado.
- `cargo test --manifest-path src-tauri/Cargo.toml --test mock_window --features mock-window-tests`: teste de integração Tauri com `MockRuntime`; no Windows MSVC, o harness recebe um manifesto Common Controls v6 próprio.
- `window_config_tests::main_window_uses_requested_default_and_minimum_size`: dimensões, mínimos e centralização da janela principal em `tauri.conf.json`.
- Testes Rust da validação do endpoint, mapeamento de status, recusa de formatos de imagem e normalização de avatar; sem chamadas às APIs reais.
- Testes Vitest da migração v1 e isolamento de endpoint/modelo entre provedores.
- `provider-directory.test.ts`: filtros das abas, ativação/desativação e remoção da credencial após sucesso.
- `providers.test.ts`: Cerebras no catálogo e provedores marcados com plano gratuito.
- `providers::failover::tests::offline_detection_is_limited_to_connection_and_timeout_errors`: diferencia erros de conexão/timeout de HTTP 401, 403, 429 e 5xx.
- `npm run tauri:build`: compila o aplicativo Tauri. A ausência de assets proprietários no bundle deve ser conferida no resultado do build.
- `git diff --check`: whitespace e marcadores de conflito.

### UI-2 — sistema visual e cópia

- `DesignSystem.test.tsx`: variantes de botão, nome e estado acessíveis de alternância e cartão selecionável, estrutura de cabeçalho, aviso, selo, cartão, campo e seletor.
- `npm run build`: verifica TypeScript e bundle após a adoção dos componentes e tokens.
- Validação visual pendente no Windows: Visão geral, Conversas, Agenda, Habilidades, Conectores de IA e Configurações em 1250 × 700 e 1000 × 600; conferir foco, contraste e escala de texto.
- `assets-src/iconly/` contém SVGs locais ainda não rastreados, mas não há arquivo de licença nem atribuição informada. Os ícones não foram incorporados; a atribuição e o uso dependem de confirmar os direitos.

### Painéis da tela Conversas

- `Conversations.test.tsx` verifica os estados expandido e recolhido da lista; `settings.test.ts` verifica a persistência e os limites das larguras.
- A barra lateral alterna com `Ctrl+B`; as alças dos painéis aceitam arraste e setas do teclado. As preferências ficam nas configurações locais da interface.
- Validação visual pendente no executável Windows em 1000 × 600, incluindo foco, redimensionamento por ponteiro e legibilidade no modo compacto.

### L0 — Início

- `home.test.ts` verifica disponibilidade, ordem e limite das sugestões, requisitos do checklist, dados ausentes, preenchimento sem envio automático e limites do dia local.
- `MessageComposer.test.tsx`, `Conversations.test.tsx` e `Overview.test.tsx` cobrem o campo compartilhado, Enter/Shift+Enter, estado de envio, envio inicial, retomar conversas e ausência de conteúdo fictício.
- Validação visual no Windows pendente em 1000 × 600: conferir posição do mascote, sugestões, checklist, faixa de aprovação e acesso ao histórico sem cortes ou rolagem horizontal.

### L8 — comando rápido da ilha

- `island-quick.test.ts` cobre a solicitação de foco e sua liberação, envio sem duplicação, criação de conversa com texto revisável, limite de três sugestões reais, resumo e encaminhamento de cada tipo de alerta sem ação de aprovação na ilha.
- Build, testes de frontend, codificação e ACL foram executados nesta sessão. A suíte completa tem três testes de publicação bloqueados pelo `spawnSync git EPERM` do sandbox; Rust não pôde ser compilado porque `cargo`/`rustfmt` não estão instalados no ambiente.
- Validação visual e de foco no Windows pendente, incluindo monitores múltiplos e DPI misto. A confirmação documental Microsoft do comportamento de foco também está pendente por bloqueio HTTP 403 neste ambiente.

### L9 — assistente de primeiro uso

- `first-use.test.ts` usa armazenamento simulado para verificar abrir uma vez, persistir e retomar, pular, voltar e concluir apenas etapas atendidas. `FirstUseAssistant.test.tsx` verifica as etapas como botões com nome acessível e estado atual anunciado. `Modal.test.tsx` cobre foco por teclado e Escape no Modal reutilizado.
- A grade de provedores, a pasta, a seleção de mascote e os modos de permissão reutilizam componentes/fluxos já testados; o modo do assistente suprime descoberta/listagem e teste rápido de modelos, e nenhum teste usa credenciais reais ou chama o provedor automaticamente.
- Validação visual no Windows pendente, incluindo tamanho do Modal, foco e leitura das etapas em telas estreitas.

### L2 — Cartão de ação do agente

- `AgentControls.test.tsx` cobre os estados apresentados, prévia como texto, timeline, parada, revisão pela janela nativa e controles de artefato sem botões de decisão.
- `agent-card.test.ts` cobre normalização de estado, prazo restante e rejeição de travessia/extensões executáveis.
- Teste Rust em `history.rs` cobre migração aditiva de colunas e leitura retrocompatível de cartões antigos; requer `cargo test`.
- Comandos para abrir/mostrar artefatos revalidam caminho existente, pasta de trabalho ou permitida e extensões bloqueadas no Rust.
- **VALIDAÇÃO VISUAL NO WINDOWS PENDENTE** para foco da janela nativa, leitores de tela e interação com o Explorador.

### Links em respostas

- `markdown.test.ts` cobre links Markdown e URLs soltas, esquemas proibidos, caracteres de controle e normalização de domínios; `MessageMarkdown.test.tsx` verifica o rótulo do domínio de destino.
- `external_links.rs` valida HTTP, HTTPS e mailto, recusa IPs privados/loopback/especiais e verifica destinos maliciosos antes de usar o navegador padrão; os testes não fazem chamadas de rede.
- `productivity.rs` verifica CRLF, escape RFC 5545, UTC, dobra de linhas, datas com fuso, fim posterior ao início e CSV com acentos/delimitadores. A abertura do calendário padrão e a interoperabilidade do `.ics` precisam de validação nativa no Windows.
- A confirmação de domínio e a abertura pelo navegador padrão ainda precisam de validação visual/nativa no Windows.

### UI-3 — pet 2D

- `src/core/pets.test.mjs`: rejeita catálogo ausente e nomes de arquivo inválidos, verifica mapeamento da FSM e do estado do agente, fallback para PNG ausente e preferência de movimento reduzido.
- `src/core/pet-layout.test.ts`: verifica dimensões da janela compacta/expandida e escala do mascote no modo Livre; `pet_position.rs` verifica o ajuste do painel expandido à área útil.
- A animação CSS do sprite pausa por visibilidade do documento e eventos Tauri de foco, minimização e visibilidade da janela do pet. O comportamento renderizado ainda precisa de observação no Windows.
- `pet-motion.test.ts` cobre pausa por documento/janela oculta, minimização e foco perdido por 2,5 s, retomada ao focar, preferência de movimento reduzido e uso de `data-performance`. Testes de settings/Home/Agenda verificam persistência da opção de mascote estático e os respectivos estados vazios. A janela do pet observa a visibilidade própria e o foco da janela principal, pois é intencionalmente não focalizável.
- A carga de CPU em idle não foi medida: este ambiente não executa o binário Windows. A implementação não usa polling contínuo; eventos Tauri/documento e CSS controlam a pausa.
- `history.rs`: `pet_catalog_rejects_missing_json_and_unsafe_paths` valida o manifesto e bloqueia caminhos inseguros; `selected_pet_defaults_and_persists_in_sqlite` confere o padrão e persistência local.
- `npm run build`, `npm test`, `npm run format:check`, `cargo fmt --check`, `cargo test --offline --lib` e `git diff --check` devem ser executados para validar o código.
- L11: `performance.test.ts` exercita Automático com perfis simulados, inclusive os limites de 8 GiB e 4 núcleos. `performance-mode.test.ts` confere a aplicação em ambas as janelas e as regras CSS. Build medido em 2026-10-30: JavaScript+CSS somam 881.681 bytes (861,0 KiB); Desempenho é uma preferência de runtime, portanto Ligado/Desligado têm delta de bundle igual a 0 bytes. CPU em idle e total de IPC em 60 s ainda dependem de medição do executável no Windows.
- L12: `Modal.test.tsx`, `ActivitySidePanel.test.tsx`, `command-palette.test.ts`, `Overview.test.tsx`, `AgentControls.test.tsx` e `markdown.test.ts` verificam foco, teclado, rótulos ARIA, ausência de decisões de aprovação nos cartões/painel, parsing seguro e renderização de texto. `npm run check:design-tokens` cobre tokens e pares de contraste principais; não representa a conferência de cada pixel da interface. O roteiro de validação manual para 1000×600 e 1250×700, movimento, segurança e medições está em [`WINDOWS-MANUAL-VALIDATION.md`](WINDOWS-MANUAL-VALIDATION.md). CPU idle e IPC total por 60 s exigem o executável Windows; a contagem IPC ainda requer build de diagnóstico instrumentada.
- Validação visual nativa pendente no Windows: cápsula expandida/recolhida, ajuste em monitores pequenos, transparência real sem borda/sombra, animações nos dois modos, avatar, foco, arrasto, topmost, escala/DPI e limites da área útil em vários monitores. Medição de CPU idle não disponível nesta sessão; não validar click-through, pois a janela recebe cliques normalmente.
- Há SVGs locais em `assets-src/iconly/`, sem arquivo de licença e ainda não rastreados pelo Git; não são incorporados nesta fase.

### Fase 3 — permissões

- `policy.rs`: tabela completa de modos × riscos × origens normal/tainted × dentro/fora do workspace, incluindo exceções permissivas e bloqueios fixos.
- `security.rs`: schema tipado com campos desconhecidos recusados; validação de traversal, UNC, nomes reservados, reparse point e hard link; bloqueio de pagamento, caminhos comuns de credenciais e marcadores conhecidos de segredo; limite de frequência; escrita com backup e lixeira sem exclusão definitiva; hash de parâmetros, fila limitada e cadeia de auditoria com adulteração detectada.
- `lib.rs`: teste de isolamento do chamador da aprovação (`approval` somente).
- `lib.rs`: teste das transições de criação/fechamento das janelas opcionais (`feature_window_lifecycle_tests`).
- `settings.test.ts`: rejeição de endpoint cujo host não pertence ao provedor salvo, incluindo Groq com endpoint da xAI.
- `history.rs`: listagem do SQLite inclui título e `created_at` (`conversation_list_includes_title_and_creation_time`).
- `history.rs`: prévia, horário de mensagens, renomeação e leitura da conversa selecionada.
- `chat.test.ts`: busca, seleção válida, ordem das mensagens e alinhamento conforme o autor.
- `markdown.test.ts`: texto HTML não interpretado e esquemas executáveis recusados em links.
- `settings.test.ts`: persistência da preferência do mascote como avatar das respostas.
- `history-actions.test.ts`: confirmação do apagamento total exige a palavra `APAGAR`.
- Todas as ferramentas desta fase são simuladas e os testes não fazem chamadas de rede reais.
- A confirmação de modo e exceção usa caixa nativa Win32 em execução interativa; não é exercitada por teste automatizado.

Resultados numéricos de builds e testes anteriores não devem ser aplicados à árvore atual. Uma tentativa antiga de instalação NSIS em pasta temporária e abertura do Hub não comprovou pixels renderizados. Os resultados atuais ficam registrados abaixo.

## Pré-visualização de desenvolvimento

Na página Companion, abrir “Pré-visualização e ferramentas de diagnóstico”. O painel permite escolher qualquer expressão, testar movimentos, habilitar flutuação/animações, ajustar luz, escala, posição e órbita da câmera, e ler FPS/triângulos/chamadas. O aviso identifica a simulação como local; os controles não emitem eventos para outras janelas.

O GLB deste proprietário fica fora do Git e pode ser incluído pelo `npm run build` local neste checkout. O empacotamento Tauri é bloqueado enquanto o GLB ou referências forem detectados em `dist/`. Um checkout sem o GLB deve exibir o fallback e seguir compilando. As referências não são importadas pelo Vite.

## Checklist manual de validação nativa

Execute em uma sessão Windows com a janela visível. Registre sistema, escala DPI, resolução/monitores e resultado por etapa. Não marque como aprovado quando não for possível observar o comportamento.

1. Abra o HUB e confirme que a janela aparece, responde e fecha normalmente.
2. Ative a ilha; confirme posição inicial, transparência, expansão/recolhimento e que os cantos transparentes continuam interceptando cliques da janela.
3. No modo Livre, arraste o pet e confira foco e limites da área útil em cada monitor.
4. Reinicie o app e confirme posição e modo salvos; alterne entre Ilha e Livre.
5. Abra a pré-visualização e percorra `idle`, `happy`, `sad`, `angry`, `surprised`, `confused`, `thinking`, `listening`, `talking`, `sleeping`, `waking`, `celebrating`, `loading`, `error`, `working` e `notification`. Para cada uma, confirme olhos no visor, identidade preservada, transição suave e ausência de deformação.
6. Teste piscar, olhar, flutuar, inclinar, arrastar, retornar e movimento reduzido. Minimize/restaure e feche/reabra; confirme que a animação pausa/restaura e não há consumo crescente ou processos órfãos.
7. Compare frente e três quartos com as referências oficiais locais; confira UV, espelhamento, profundidade, materiais vermelhos/pretos, brilho dos olhos, nitidez e transparência do fundo.
8. Envie uma conversa real e confirme as expressões `thinking`, `responding`, `completed` e `error` somente nos eventos correspondentes.
9. Repita em DPI 100%, 125% e 150%, monitores distintos, reconexão/desconexão e fullscreen; observe escala, posição e interação com o desktop.
10. Instale o NSIS em pasta de teste isolada, abra e feche o app, depois desinstale. Confirme que o UDF normal, o histórico e as preferências existentes permanecem intactos.
11. Abra o aplicativo duas vezes; confirme que a segunda abertura traz o Hub existente ao foco e não reproduz `0x800700AA`. Se o erro reaparecer, registre o processo e o perfil WebView2 em uso.
12. Envie uma resposta longa e confirme texto incremental, cancelamento e retomada da conversa após reiniciar. Repita com Anthropic, um adaptador Chat Completions e Perplexity.
13. Em perfil Windows novo, confirme a sugestão `%USERPROFILE%\OdeKrai`, escolha outra pasta local, confira o aviso OneDrive e verifique a estrutura criada. Tente UNC, raiz de unidade, pasta do sistema, pasta do aplicativo, `..`, nome reservado e junction; confirme as recusas.
14. Edite `ODEKRAI.md` fora do aplicativo, volte à tela Identidade e memória e confira o diff. Confirme que o chat bloqueia a identidade alterada e só retoma após aprovação explícita.
15. Crie, leia, edite e apague `memory/estilo.md` e outra nota. Apague toda a memória. Reinicie e confirme persistência. Não use dados reais sensíveis nesse ensaio.
16. Crie duas conversas, selecione e renomeie uma, apague uma e depois apague todo o histórico em Configurações → Dados e privacidade após as duas confirmações. Reinicie e confirme que nenhuma reaparece. Confira no provedor real, com conta de teste, o campo de sistema/instruções recebido; testes locais não comprovam essa integração.
17. No perfil, salve nome e apelido, recarregue a aplicação e confirme persistência em SQLite. Tente SVG, GIF, arquivo corrompido, imagem acima do limite e PNG/JPEG/WebP válidos; confira recorte e dimensão 256 × 256.
18. Troque entre provedores e confirme que endpoint/modelo não se misturam. Informe endpoint de outro provedor e confirme a recusa antes do salvamento da credencial. Carregue modelos somente nos provedores com endpoint documentado; confirme o estado parcial nos demais.
19. Em Configurações → Permissões, troque cada modo e confirme a caixa nativa antes de salvar. Confira que o modo exibido em `ODEKRAI.md` não altera o modo efetivo do SQLite.
20. Em build de desenvolvimento, teste leitura, gravação com backup, mover para `.trash`, envio simulado e bloqueio financeiro. Confirme que só a janela `approval` pode aprovar, que Negar e fechar negam, que parâmetros adulterados/replay/expiração/fila cheia falham fechados e que conteúdo externo pede aprovação.
21. Confirme que a inicialização abre apenas `main`; habilite, oculte e alterne os modos Ilha/Livre do pet; resolva/expire aprovações e confirme que a janela do pet é criada sob demanda.
22. Na primeira execução com a janela visível, abra o seletor de pasta nativo, escolha um diretório local e confirme que o Rust ainda valida o caminho antes de criar a pasta do agente; teste os selects de idioma e tom.
23. Nas resoluções de janela 1250×700 e 1000×600, percorra Visão geral, Conversas, Agenda, Habilidades e Conectores de IA. No modal de Configurações, confira as nove seções, pesquisa, menu lateral e lista suspensa estreita; confirme uma área de rolagem própria, conteúdo de fundo preservado e rodapé fixo.
24. Ao mudar modo ou habilitar override, confirme que a caixa Win32 aparece sem congelar a janela principal enquanto permanece aberta; confirme Cancelar e Sim retornando à interface.
25. Execute `npm run build` e inspecione `dist/` para confirmar que o aplicativo não inclui modelos GLB nem arquivos do renderer 3D.
26. Teste `Ctrl+Shift+F12` e um atalho em conflito. Confirme que Parar tudo cancela uma resposta em streaming, nega aprovações pendentes, bloqueia novas ações e persiste após reiniciar; Retomar exige uma auditoria gravada.
27. Na tela Atividade, confirme a cadeia, exporte JSONL e altere uma cópia de teste para verificar a detecção. Não edite os logs reais durante o ensaio.
28. Repita a janela de aprovação em 100%, 125% e 150% de DPI, monitores distintos, foco alternado e telas menores; verifique texto longo sem renderização de Markdown/HTML.
29. A partir dos acionadores antigos, abra Configurações, Permissões e segurança, Meu perfil, Identidade e memória, Assistente, Seu computador e Atividade; confirme que cada um abre a seção correspondente e que “Abrir tela completa” em Atividade preserva a rota antiga.
30. Com teclado, percorra o menu do perfil, pesquisa, seções e controles do modal com Tab e Shift+Tab; confirme rótulos acessíveis, foco visível, Escape, retorno do foco ao acionador e ausência de captura do teclado após fechar.
31. No Windows, compare as telas com movimento normal e reduzido e com `data-performance="on"`; confirme ausência de animações não essenciais, sombras grandes, gradientes animados e transparência no modo de desempenho.
32. Em uma sessão parada de 60 segundos do executável, registre chamadas IPC e CPU do processo sem interagir; repita com o mesmo estado após a mudança. Se a contagem IPC não puder ser instrumentada, registre-a como não medida e não estime o valor.

## Inspeção visual manual pendente

A interface não ficou disponível para inspeção visual nesta sessão. O teste recente confirmou inicialização do executável e do UDF padrão por processo e título de janela, mas não comprova pixels renderizados, ilha visível, interação ou comportamento instalado. Execute o checklist acima numa sessão gráfica acessível.

Build/testes automatizados não comprovam visualização 3D correta, orientação UV, WebGL em Tauri, click-through ou comportamento em múltiplos monitores.

## Áudio

Não há sons Kenney integrados. As páginas oficiais de [UI Audio](https://kenney.nl/assets/ui-audio) e [Interface Sounds](https://kenney.nl/assets/interface-sounds) indicam CC0, mas os arquivos ZIP e licenças internas ainda precisam ser verificados. Não copiar sons do Coucou.

Leitura em voz alta: `speech.test.ts` verifica a limpeza de Markdown/código/links, o filtro de vozes locais `pt-BR`, substituição da fala anterior e parada explícita, usando sintetizador falso. A leitura não chama rede. **Validação no Windows pendente:** confira a enumeração das vozes instaladas no WebView2, instale uma voz `pt-BR` se necessário, compare velocidade/volume, e confirme “Parar”, nova mensagem, silenciamento do pet, saída da tela e parada de emergência.

## Capturas verificáveis da revisão atual

As capturas desta revisão ainda não foram produzidas. Em Windows com sessão gráfica disponível:

1. Feche o processo antigo de release e gere o pacote atualizado com `npm run tauri:build`.
2. Inicie o novo executável e capture o Hub em 1366×768 ou na resolução real usada, com a escala DPI anotada.
3. Capture o catálogo mostrando o estado do provedor sem credencial e, se houver conta autorizada, uma conexão testada sem expor campos secretos.
4. Capture a ilha e a janela livre em separado, incluindo uma imagem sobre desktop claro e outra escuro para avaliar transparência.
5. Salve PNGs em diretório de evidências local ignorado pelo Git; anote build/commit, Windows, resolução, escala, monitor e etapa do checklist. Nunca publique imagens que mostrem chaves ou dados privados.

## Resultados da Fase 1 (24/10/2026)

- Registro histórico da revisão 3.1: o build daquela versão incluiu um chunk Three.js e um GLB local. Esse resultado não descreve o código atual após a remoção do renderer 3D.
- `npm test -- --pool=threads --maxWorkers=1`: 16 testes passaram em 5 arquivos, executados fora da restrição que causou `spawn EPERM` na primeira tentativa.
- `cargo check --manifest-path src-tauri/Cargo.toml --lib`: passou após resolver `rusqlite` e `tauri-plugin-single-instance`.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: passou.
- Prettier nos arquivos alterados: passou. `npm run format:check` global falhou em `index.html`, `README.en.md`, `src/ui/IslandWindow.tsx` e no arquivo local não rastreado `ODEKRAI.md`; esses arquivos não foram modificados nesta fase.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: 5 testes passaram, incluindo cabeçalhos com servidor simulado, parsing de streaming e esquema SQLite.
- Inspeção visual e teste com provedores reais: não executados.

## Fase 2

O roteiro nativo da Fase 2 está nos itens 13–16. A validação automatizada desta revisão deve incluir sanitização dos campos, caminhos proibidos, detecção de alterações externas e ordem/limite do prompt, além de `npm run build`, `npm test`, `npm run format:check`, `cargo fmt --check`, `cargo test` e `git diff --check`. Registrar os resultados efetivamente obtidos no relatório da tarefa; build e testes locais não comprovam o assistente visível, junctions do perfil real, Credential Manager nem o formato aceito por contas reais dos provedores.

# Janela inicial por área útil

A janela principal pede 1.250 × 700 px lógicos e é redimensionada após a criação usando a área útil do monitor (exclui a barra de tarefas) e o fator de escala informado pelo Tauri. Cada dimensão fica limitada a 90% da área útil. O mínimo nativo é reduzido quando não cabe na área útil, para evitar que a janela ultrapasse a tela.

O teste `window_sizing::tests` usa uma área útil sintética de 1.920 × 1.040 px físicos. Os tamanhos lógicos calculados são 1.250 × 700 em 100%, 1.250 × 700 em 125% e 1.152 × 624 em 150%. Esses valores validam o cálculo; não substituem a medição visual na tela real. O corte de conteúdo por tela ainda requer validação visual em Windows nos três fatores de escala.

## Entrada e cabeçalho

O estado inicial é Início; nome/apelido, mascote, mensagens, sugestões e requisitos disponíveis usam o estado carregado pelo aplicativo. A falta de provedor ou pasta aparece em um checklist com navegação para resolução. O chip do cabeçalho abre Conectores e mostra o provedor/modelo quando há credencial disponível. A disposição precisa de conferência visual em Windows.

O histórico local guarda o provedor e modelo da última resposta de cada conversa. `history.rs` testa a migração do esquema anterior e a leitura desses campos; `Conversations.test.tsx` verifica que o chip só aparece quando a conversa difere do modelo atual. A exibição ainda requer conferência visual em Windows.

## Fechamento e bandeja do Windows

O padrão é Encerrar. Nessa opção, o X tenta cancelar solicitações registradas de provedor e fechar as janelas auxiliares; a limpeza tem limite de três segundos e, mesmo se excedido, o encerramento segue pelo ciclo normal do Tauri. Antes da saída, o ícone da bandeja é removido. Em Minimizar para a bandeja, o X oculta a janela principal e o menu nativo oferece Abrir e Sair; Abrir e uma segunda instância desminimizam, exibem e focam a janela existente. Sair também encerra o processo. Testes unitários cobrem a escolha entre ocultar e encerrar, o limite e a finalização da limpeza e a restauração da janela simulada. A interação real com a bandeja, reabertura por segunda instância, encerramento de WebView2 e verificação de ícones residuais ainda requer teste visual/nativo em Windows; consulte `docs/qa/LOTE-6-E2.md`.

## Identidade e memória

Na inicialização, nome e apelido aprovados em `ODEKRAI.md` preenchem campos vazios do perfil local; valores existentes e avatar são preservados. `identity.test.ts` cobre essa combinação. A validação do perfil no cartão lateral e das mensagens permanece incluída na conferência visual do Windows.

`npm test` cobre a conversão das instruções pessoais e expressões para as notas locais e a sanitização dos campos de nome. `cargo test --manifest-path src-tauri/Cargo.toml` cobre a exigência de nova aprovação do arquivo base após trocar a pasta. A inspeção visual do assistente de primeira configuração, do painel unificado, da seleção de pasta e da comparação/confirmação do arquivo base ainda está pendente no Windows.

## Posição do pet por monitor

A janela do pet salva a fração da posição dentro da área útil para cada monitor, junto com nome, origem, dimensões da área útil e escala DPI. Ao restaurar, usa o monitor salvo se ainda estiver disponível; se não estiver, usa o primário. Mudanças de DPI/monitor e a reexibição reposicionam e limitam a janela à área útil. Eventos de movimento persistem no máximo uma vez por segundo, com gravação imediata ao ocultar.

A geometria tem testes unitários para escala alterada, monitor removido, origem negativa e resolução reduzida. A validação visual manual em múltiplos monitores e DPI permanece pendente; executar os itens 3, 4 e 9 do checklist nativo acima quando houver sessão Windows apropriada.

## Desfazer (L3)

Os testes de restauração de sistema devem usar `SystemAdapter` simulado e SQLite em memória; os testes de arquivo usam diretórios temporários isolados. Não execute esses testes sobre configurações reais do Windows nem sobre arquivos do usuário. Cobrem IDs de operação independentes, estado ausente, edição posterior do arquivo, confirmação e ausência de botão para ações irreversíveis. A suíte `cargo test` ainda deve ser executada no ambiente Rust; valide também a janela e o aviso no Windows.

## Planos informativos (L4)

`skills::tests` verifica os limites e que texto instrucional no plano não muda seu risco de leitura; `workspace::files_tests` verifica que `plan.propose` não modifica arquivos. `agent_runtime::plan_state_tests` cobre avanço, falha e chamada fora das etapas listadas; `AgentControls.test.tsx` verifica renderização numerada e escaping do texto como dado. Os testes Rust usam somente dados em memória/diretórios temporários e precisam ser executados em ambiente com Cargo. A conferência visual do cartão no Windows permanece pendente.

## Painel lateral de atividade (L5)

`ActivitySidePanel.test.tsx` cobre as cinco seções com dados simulados, estados vazios, resumo de auditoria sem parâmetros, exclusão de executáveis, ausência de controles de aprovação, gatilho/estado fechado e lógica de foco/teclado. A consulta de auditoria é compartilhada com a página Atividade; a consulta só é iniciada com o painel aberto ou a página completa ativa. Não há polling do painel fechado. A validação de dimensões, sobreposição, foco real do WebView2 e operação dos comandos do Windows exige validação visual em Windows.
