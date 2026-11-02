# Medições de desempenho

Medições feitas em 2026-11-02 no HEAD `3ed6ea47f78a0b95ee6174bc30c93ea3f66431e0`, versão `0.17.0`.

## Resultados

| Métrica                                             |                                                                   Valor medido | Método e observação                                                                                                                                                                                                        |
| --------------------------------------------------- | -----------------------------------------------------------------------------: | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| JavaScript emitido pelo build                       | 838.760 bytes (819,10 KiB); gzip individual somado: 242.276 bytes (236,60 KiB) | `npm run build`; soma dos 14 arquivos `.js` em `dist/assets`, com `stat` para o tamanho bruto e `gzipSync` de cada arquivo para o comprimido. Inclui chunks carregados sob demanda; não representa apenas a carga inicial. |
| CSS emitido pelo build                              |          139.590 bytes (136,32 KiB); gzip individual: 26.574 bytes (25,95 KiB) | Mesmo build; soma dos arquivos `.css` em `dist/assets` (um arquivo nesta execução).                                                                                                                                        |
| Chamadas de comando em 60 segundos com o app parado |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows; não foi possível observar o app em repouso.                                                                                                                   |
| CPU em repouso com o pet visível                    |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows.                                                                                                                                                               |
| CPU em repouso com o pet oculto                     |                                                                                | **NÃO MEDIDO.** Ambiente Linux sem executável Tauri/Windows.                                                                                                                                                               |

## Método e limitações

O bundle foi medido após um build de produção. Os tamanhos comprimidos são a soma dos bytes produzidos ao aplicar gzip a cada arquivo separadamente; não representam uma transferência HTTP real nem deduplicam cabeçalhos entre arquivos.

O ambiente da medição foi Linux x86_64 (`uname -a` iniciou com `Linux e23917e82b5c 6.18.57`), sem `powershell.exe`, `pwsh`, `tauri` ou `cargo`. Por isso não foi possível iniciar o executável Windows, medir chamadas Tauri por 60 segundos, nem obter amostras de CPU com o mascote visível e oculto. Esses campos permanecem vazios porque não foram medidos; os valores não foram estimados.

Para completar as três medições pendentes, usar um build Windows do mesmo HEAD, deixar o app sem interação durante 60 segundos e registrar as chamadas de comandos observadas no DevTools/diagnóstico; em seguida medir a CPU do processo em repouso em duas amostras, com o pet visível e oculto, mantendo as mesmas condições e duração. Registrar ferramenta, duração e valores observados nesta tabela.
