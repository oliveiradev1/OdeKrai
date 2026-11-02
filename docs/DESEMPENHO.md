# Medições de desempenho

Medições de referência feitas em 2026-11-02 no HEAD `3ed6ea47f78a0b95ee6174bc30c93ea3f66431e0`, versão `0.17.0`. A comparação após F6-2 foi feita no mesmo ambiente e versão, a partir do estado inicial `fde985861eb2a747d19745738259b7ce9be842bf`.

## Resultados

| Métrica                                             |                                                                   Valor medido | Método e observação                                                                                                                                                                                                        |
| --------------------------------------------------- | -----------------------------------------------------------------------------: | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| JavaScript emitido pelo build                       | 838.760 bytes (819,10 KiB); gzip individual somado: 242.276 bytes (236,60 KiB) | `npm run build`; soma dos 14 arquivos `.js` em `dist/assets`, com `stat` para o tamanho bruto e `gzipSync` de cada arquivo para o comprimido. Inclui chunks carregados sob demanda; não representa apenas a carga inicial. |
| CSS emitido pelo build                              |          139.590 bytes (136,32 KiB); gzip individual: 26.574 bytes (25,95 KiB) | Mesmo build; soma dos arquivos `.css` em `dist/assets` (um arquivo nesta execução).                                                                                                                                        |
| Chamadas de comando em 60 segundos com o app parado |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows; não foi possível observar o app em repouso.                                                                                                                   |
| CPU em repouso com o pet visível                    |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows.                                                                                                                                                               |
| CPU em repouso com o pet oculto                     |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows.                                                                                                                                                               |

## Comparação F6-2

`npm run build` foi executado antes e depois no mesmo ambiente Linux. Para a carga inicial, contei o JavaScript e CSS referenciados diretamente por `dist/index.html`; gzip foi aplicado individualmente a cada arquivo. O total emitido inclui também os chunks que só são carregados ao abrir suas telas.

| Métrica                                             |     Antes (HEAD `fde9858`) |        Depois (build F6-2) |                                     Variação |
| --------------------------------------------------- | -------------------------: | -------------------------: | -------------------------------------------: |
| JavaScript da carga inicial                         | 675.179 B (gzip 190.702 B) | 551.620 B (gzip 158.166 B) | −123.559 B (−18,3%); gzip −32.536 B (−17,1%) |
| CSS da carga inicial                                |  139.590 B (gzip 26.574 B) |  138.200 B (gzip 26.337 B) |        −1.390 B (−1,0%); gzip −237 B (−0,9%) |
| JavaScript total emitido                            | 838.760 B (gzip 242.276 B) | 842.664 B (gzip 248.324 B) |                      +3.904 B; gzip +6.048 B |
| CSS total emitido                                   |  139.590 B (gzip 26.574 B) |  139.591 B (gzip 26.797 B) |                            +1 B; gzip +223 B |
| Chamadas de comando em 60 segundos com o app parado |             **NÃO MEDIDO** |             **NÃO MEDIDO** |     Requer executar e observar o app Windows |
| CPU em repouso com o pet visível/oculto             |             **NÃO MEDIDO** |             **NÃO MEDIDO** |                Requer executar o app Windows |

As telas Habilidades, Segurança, Aprovação e Configurações, o painel Atividade e os estilos próprios de Configurações agora são carregados sob demanda. Isso reduziu o payload inicial de JavaScript e CSS; o total dos arquivos emitidos cresceu ligeiramente pelo custo de separação em chunks. O painel Atividade também só é montado após a primeira abertura.

O contexto do Início compartilha uma única leitura em andamento se o efeito for reiniciado por navegação ou atualização enquanto `get_skills`, `calendar_list_events` ou `get_home_reminder_count` ainda está respondendo. O teste de `runOnceWhilePending` confirma que duas solicitações concorrentes executam o trabalho uma vez e que uma atualização posterior continua possível. Não foi medida a frequência real desses comandos durante uma sessão Windows.

O polling de hit-test do pet quando o cursor está longe do sprite passou de 60 ms para 120 ms (8,3 consultas por segundo em vez de 16,7, conforme o intervalo configurado). Perto do sprite, o intervalo de 16 ms foi mantido para preservar a resposta ao clique; pet oculto/desativado continua aguardando notificação. Esse é um intervalo configurado no código, não uma medição de CPU. A validação de clique e CPU no Windows continua pendente.

## Método e limitações

O bundle foi medido após um build de produção. Os tamanhos comprimidos são a soma dos bytes produzidos ao aplicar gzip a cada arquivo separadamente; não representam uma transferência HTTP real nem deduplicam cabeçalhos entre arquivos.

O ambiente da medição foi Linux x86_64 (`uname -a` iniciou com `Linux e23917e82b5c 6.18.57`), sem `powershell.exe`, `pwsh`, `tauri` ou `cargo`. Por isso não foi possível iniciar o executável Windows, medir chamadas Tauri por 60 segundos, nem obter amostras de CPU com o mascote visível e oculto. Esses campos permanecem vazios porque não foram medidos; os valores não foram estimados.

Para completar as três medições pendentes, usar um build Windows do mesmo HEAD, deixar o app sem interação durante 60 segundos e registrar as chamadas de comandos observadas no DevTools/diagnóstico; em seguida medir a CPU do processo em repouso em duas amostras, com o pet visível e oculto, mantendo as mesmas condições e duração. Registrar ferramenta, duração e valores observados nesta tabela.
