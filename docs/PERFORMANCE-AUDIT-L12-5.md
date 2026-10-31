# Auditoria de layout L12-5

## Escopo e método

Foi feita uma busca estática por `getBoundingClientRect`, `offsetWidth`, `offsetHeight`,
`scrollHeight`, `clientWidth` e `clientHeight` em Conversas e Habilidades. A medição dinâmica usou
uma gravação de timeline Chromium via DevTools Protocol, com as categorias `devtools.timeline` e
`disabled-by-default-devtools.timeline`.

Roteiro repetido nas duas gravações:

1. Recarregar o app local e abrir Conversas.
2. Digitar “Mensagem para medir layout”, um caractere a cada 25 ms; aguardar 300 ms.
3. Abrir Habilidades.
4. Digitar “agenda” na busca, um caractere a cada 25 ms; aguardar 300 ms.

Ambiente: Chromium headless 154.0.8037.57, Vite local, viewport 780 × 437. O WebView estava fora
do Tauri: comandos nativos não carregaram o catálogo, então a medição de Habilidades cobre o
estado de erro/sem dados e não representa uma lista real de habilidades. A janela do executável
Windows não foi medida.

## Leituras encontradas

- `Conversations.tsx`: `scrollHeight`, `scrollTop` e `clientHeight` são lidos uma vez no evento de
  rolagem para decidir se a conversa acompanha o fim. Outros acessos leem `scrollHeight` antes de
  escrever `scrollTop` nos efeitos de acompanhamento. Não há leitura de geometria em laço de
  renderização.
- `MessageComposer.tsx`: o efeito de redimensionamento escreve `height: auto`, lê `scrollHeight` e
  escreve a altura resultante quando o rascunho muda. É uma leitura por atualização do campo, fora
  de laços de renderização.
- `SkillsScreen.tsx`: não contém leituras de geometria (`offset*`, `getBoundingClientRect`,
  `scrollHeight` ou `client*`) nem leituras dentro de laços de renderização.
- `SidebarNavigation.tsx` tem duas leituras `getBoundingClientRect` na mesma chamada de
  `requestAnimationFrame`, antes de atualizar a posição do menu. Essa rotina não é chamada pelas
  telas auditadas durante a gravação.

## Medição antes e repetição

Não foi encontrado um caso de custo alto em laço que justificasse alteração. Por isso, não houve
mudança de código; a segunda gravação repetiu o mesmo roteiro como verificação de estabilidade, não
como comparação após uma correção.

| Tela e interação                   |  Antes: Layout / UpdateLayoutTree | Repetição: Layout / UpdateLayoutTree |
| ---------------------------------- | --------------------------------: | -----------------------------------: |
| Conversas, 26 caracteres           |                  0 eventos / 0 ms |                     0 eventos / 0 ms |
| Habilidades, busca de 6 caracteres | 1 evento / 0,094 ms; 1 / 0,032 ms |    1 evento / 0,114 ms; 1 / 0,141 ms |

Os valores de Habilidades são submilissegundo e variaram entre gravações; não indicam uma fonte
repetida de layout. Como não houve mudança de código, não há delta de desempenho atribuível a uma
correção.
