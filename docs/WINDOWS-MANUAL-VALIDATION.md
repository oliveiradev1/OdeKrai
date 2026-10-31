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
4. Repita a verificação do sprite na janela do pet e confirme que a janela continua respondendo ao ponteiro. Restaure a configuração original do Windows ao terminar.
5. Confira texto, foco, bordas e cores de estado em todas as superfícies usadas. Use as razões de contraste de `DESIGN-SYSTEM.md` como referência e registre qualquer exceção visual.

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
