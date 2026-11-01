# Roteiro manual de validação no Windows

Use o executável Tauri gerado para Windows, com o mesmo perfil, escala de tela, tema do Windows e configuração de provedores em todas as passagens. Não execute ações externas reais durante os testes; para exercitar o agente, use uma tarefa local reversível em uma pasta de teste.

## Preparação

1. Instale e abra o executável. Anote versão do app, versão do Windows, escala/DPI e dimensões da janela.
2. Confirme que a janela principal e o pet abriram e que nenhuma chave de provedor aparece em campos já salvos.
3. Para cada tamanho abaixo, use a janela principal restaurada, sem maximizar. Registre capturas e marque cada resultado como passou, falhou ou não aplicável.

## Janela em 1000 × 600

1. Abra **Início**. Confirme que mascote, título, saudação e campo cabem sem sobreposição ou rolagem horizontal. Verifique que sugestões só aparecem quando há os dados e habilidades correspondentes.
2. Use Tab e Shift+Tab desde a navegação até o campo, sugestões, checklist e Retomar. Confirme foco visível e que cada botão tem nome compreensível pelo Narrador.
3. Selecione uma sugestão. Confirme que ela preenche o campo e não é enviada. Digite texto; confirme Enter envia e Shift+Enter cria uma linha, conforme a ação que você escolher executar.
4. Abra **Conversas**. Percorra busca, lista, modo agente, editor, envio e cartões. Confirme que dados externos são apresentados como texto e que o cartão não tem botões de aprovar/negar.
5. Abra **Atividade** pelo cabeçalho. Verifique as cinco seções, estados vazios e ordem de Tab. Pressione Escape: o painel deve fechar e o foco voltar ao botão. Com uma aprovação pendente, **Revisar** deve abrir a janela nativa; somente essa janela deve mostrar as decisões.
6. Pressione Ctrl+K. Pesquise, navegue com as setas, execute um destino inofensivo e confirme que Escape fecha e devolve o foco. Confirme que a lista anuncia a opção ativa.
7. Abra Configurações e outro modal da aplicação. Confirme foco inicial, contenção por Tab/Shift+Tab, rótulo do diálogo, fechamento por Escape e retorno ao acionador.

## Janela em 1250 × 700

1. Repita o fluxo do Início e confirme que Retomar e checklist ficam legíveis junto ao campo.
2. Abra Atividade. Confirme que o painel aparece ao lado do chat sem reduzir a área do chat abaixo de 600 px; em viewport estreita, confirme a apresentação sobre o conteúdo.
3. Abra a paleta e Configurações. Confirme que o foco não escapa para o conteúdo de fundo e que a rolagem do conteúdo permanece utilizável.
4. Inspecione cartões do agente, prévias longas e artefatos. A prévia deve continuar texto puro; **Abrir** e **Mostrar na pasta** não devem abrir executáveis nem caminhos fora das pastas permitidas.

## Movimento, contraste e Desempenho

1. Em Configurações > Aparência, compare animações com **Reduzir animações** ligado e desligado.
2. Altere o modo do Windows para reduzir movimento e deixe o app em **Automático**. Confirme que a justificativa informa movimento reduzido. Depois teste perfil com até 8 GiB de RAM ou até 4 núcleos lógicos e confirme o motivo correspondente.
3. Teste **Ligado** e **Desligado**. Em Ligado, confirme ausência de sombras grandes, fundos de gradiente, translucidez das superfícies de UI e movimento dos sprites/modais. Os dois estados devem manter as mesmas ações e dados.
4. No modo Livre recolhido, clique no sprite e na margem ao redor: o pet deve responder. Clique em áreas transparentes ao redor e confirme que a janela subjacente recebe o clique. Repita com DPI 100%, 125% e 150% e em cada monitor. No modo Ilha, confirme que a cápsula continua recebendo eventos normalmente. Restaure a configuração original do Windows ao terminar.
5. Confira texto, foco, bordas e cores de estado em todas as superfícies usadas. Use as razões de contraste de `DESIGN-SYSTEM.md` como referência e registre qualquer exceção visual.

## MCP e descoberta de IA local

Execute os casos abaixo no executável Windows. Use uma credencial descartável de teste, sem permissões de escrita, e remova-a ao terminar. Não cole tokens em capturas, relatórios ou diagnósticos. OAuth do Notion usa uma conta de teste autorizada. Os testes automatizados correspondentes usam dados e servidores simulados; não fazem chamadas reais.

| Caso                               | Procedimento                                                                                                                                                                                                                          | Resultado esperado                                                                                                                                                                            | Mensagem exibida                                                                                                                                                                                                             |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GitHub com PAT de teste            | Em Habilidades > Públicas, configure o cartão GitHub usando o endpoint fixo `https://api.githubcopilot.com/mcp/`, salve um PAT fine-grained descartável e teste a conexão. Depois reabra o cartão e confira o gerenciamento do token. | A conexão só aparece como conectada depois de listar ferramentas; o token não reaparece no campo nem no cartão. Ao final, use **Remover token** e confirme que o servidor fica desconectado.  | `Teste MCP concluído.` após sucesso. Em 401 ou 403: `O token foi recusado. Confira se ele tem as permissões e se não expirou.`                                                                                               |
| Notion com OAuth                   | Em Habilidades > Públicas, abra Notion, confira as opções OAuth, use **Verificar opções OAuth** e prossiga com o navegador do sistema e a aprovação na página oficial. Retorne ao OdeKrai e teste a conexão.                          | O registro dinâmico não pede client ID manual; após consentimento, o servidor lista ferramentas e só então aparece conectado. Não aprove nenhuma ferramenta MCP durante este teste.           | `Registro dinâmico disponível. O identificador será registrado e salvo com a autorização.`; após sucesso: `Teste MCP concluído.`                                                                                             |
| Servidor local inexistente         | Em Conectores de IA > Gratuitos, deixe a porta adicional vazia e clique **Procurar IA local** com os servidores padrão parados.                                                                                                       | A busca usa somente loopback; nenhum provedor é marcado como conectado e o cartão mantém o estado sem servidor.                                                                               | Estado: `Servidor não encontrado`. Aviso da busca: `Servidor local não encontrado nas portas verificadas.`                                                                                                                   |
| Modelo local baixado, sem servidor | Em Meu computador, selecione um GGUF já baixado e deixe Ollama/llama.cpp parados. Confira também o cartão de IA local em Conectores.                                                                                                  | O download não é tratado como modelo carregado ou provedor conectado.                                                                                                                         | `Estado: Baixado. Ainda não é usado pelo OdeKrai.` e, no cartão, `Servidor não encontrado`. A orientação **Como usar** explica como iniciar um servidor compatível.                                                          |
| Porta informada pelo usuário       | Inicie Ollama ou llama.cpp apenas em `127.0.0.1` numa porta entre 1024 e 65535, informe essa porta adicional e clique **Procurar IA local**. Repita com texto, decimal, 1023 e 65536.                                                 | A porta válida é incluída na descoberta local. Entradas inválidas não fazem a busca; nenhum host fora de `127.0.0.1` é consultado. Com servidor ativo sem modelo, não aparece como conectado. | Entrada inválida: `Informe uma porta inteira entre 1024 e 65535.` Sem modelos: `Serviço local online, mas sem modelos disponíveis.` Com modelos encontrados, aparece a contagem, por exemplo `1 modelos locais encontrados.` |

### Conferência dos cartões públicos

Para cada serviço em **Habilidades > Públicas**, confira a ordem e a função das ações: nome e fornecedor; um dos estados **Disponível**, **Configure o serviço**, **Conectando**, **Conectado** ou **Erro**; botão principal **Conectar**; menu **Ações** com **Ver documentação** e somente as ações de credencial/desconexão aplicáveis à autenticação do serviço. GitHub deve oferecer OAuth e **Usar token**; os cartões Google, Lovable e Notion devem oferecer apenas OAuth; Figma deve direcionar a pessoa ao guia e não declarar conexão antes da inclusão do OdeKrai no catálogo Figma. Durante uma tentativa, o botão fica desabilitado com **Conectando…**. Em falhas, confira mensagens curtas sem corpo técnico: **O serviço recusou a autorização**, **A conexão expirou, tente de novo** ou **Permissão insuficiente**. Não deve haver botão sem ação nem abrir link dentro do WebView.

Registre cada linha como **passou**, **falhou** ou **não aplicável**, incluindo a versão do app e capturas sem credenciais. Os testes automatizados relacionados ficam em `src/ui/SkillsScreen.test.tsx`, `src/core/mcp-service-catalog.test.ts`, `src/ui/LocalAiProviderCard.test.tsx`, `src/ui/DownloadedModelNotice.test.tsx` e `src/core/provider-directory.test.ts`; o probe local também tem cobertura Rust em `src-tauri/src/providers/mod.rs`.

## Medições de runtime

Faça dois períodos de 60 segundos com o mesmo executável, máquina e estado em repouso: primeiro **Desligado**, depois **Ligado**. Aguarde dois minutos após abrir e feche conversas em execução, downloads e animações não essenciais. No Gerenciador de Tarefas ou Monitor de Recursos, registre o uso médio de CPU do processo OdeKrai e dos processos WebView2 filhos durante cada período.

Para contar IPC, use uma build de diagnóstico com contador de invocações Tauri aprovado para essa validação. O bundle atual não expõe esse contador; não estime a quantidade de IPC a partir de comandos ou eventos observados. Registre “não medido” até essa instrumentação estar disponível.

Para o tamanho do bundle, rode `npm run build` em duas cópias limpas dos commits `883280c` (antes da limpeza L12) e do commit que contém este roteiro. Some os bytes dos arquivos `.js` e `.css` em `dist/assets`; use a mesma versão do Node e dependências. O registro pré-L12 é 881.681 bytes (861,0 KiB). Fontes, imagens e instalador ficam fora dessa soma.

## Registro

| Área                                   | Resultado    | Evidência ou observação                                                    |
| -------------------------------------- | ------------ | -------------------------------------------------------------------------- |
| 1000 × 600                             | PENDENTE     |                                                                            |
| 1250 × 700                             | PENDENTE     |                                                                            |
| Teclado e Narrador                     | PENDENTE     |                                                                            |
| Atividade e janela nativa de aprovação | PENDENTE     |                                                                            |
| Paleta e modais                        | PENDENTE     |                                                                            |
| Movimento reduzido e Desempenho        | PENDENTE     |                                                                            |
| Caminhos e artefatos                   | PENDENTE     |                                                                            |
| CPU idle, Desligado/Ligado             | PENDENTE     |                                                                            |
| IPC em 60 s, Desligado/Ligado          | PENDENTE     | Depende de build de diagnóstico com contador                               |
| Bundle antes/depois                    | IMPLEMENTADO | Antes: 881.681 bytes; depois: 881.282 bytes; redução de 399 bytes (JS+CSS) |
