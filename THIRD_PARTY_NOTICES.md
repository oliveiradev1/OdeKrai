# Avisos de terceiros

## Código

### Coucou — técnica da ilha Windows adaptada sob MIT

Referência examinada: [Louis-CFM/coucou](https://github.com/Louis-CFM/coucou), clone local `C:\Users\Daniel oliveira\Documents\Projetos Olivera\coucou\coucou`, tag `windows-v0.5.3`, commit `47d1c2da12bf1dce8fb28b825757d497c1e1d88d`. A licença MIT (`LICENSE`, copyright Louis Raillé, 2026) abrange o código-fonte. `LICENSE-ASSETS.md` reserva os nomes Coucou e Mochi, o personagem, ícones, sons e imagens/GIFs/vídeos. Esses nomes, personagem, ícones, sons e mídias não podem ser copiados para OdeKrai.

`src-tauri/src/lib.rs` e `src-tauri/src/pet_position.rs` reimplementam a transição direcional de janela; `src-tauri/src/pet_hit_test.rs` implementa hit-test para passagem de cliques em área transparente, com base no estudo de `windows/src-tauri/src/island.rs` do Coucou. A geometria e os contratos de posição, escala, persistência, janelas e hit-test são próprios do OdeKrai. Os arquivos adaptados incluem o aviso “Referência MIT: Louis-CFM/coucou, windows-v0.5.3, © Louis Raillé, 2026.” Nenhum trecho-fonte, nome, personagem, ícone, som ou mídia foi copiado. As restrições de `LICENSE-ASSETS.md` permanecem aplicáveis.

```text
MIT License

Copyright (c) 2026 Louis Raillé

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Áudio e artes

Os arquivos abaixo estão em `public/sounds/` junto ao `License.txt` de Kenney Interface Sounds 1.0. O texto incluído identifica Creative Commons Zero (CC0) 1.0; o crédito é opcional. Os eventos sem arquivo correspondente estão marcados como não informados.

| Evento            | Arquivo            | Pacote de origem            | URL                                       | Licença       |
| ----------------- | ------------------ | --------------------------- | ----------------------------------------- | ------------- |
| `island_open`     | `island_open.ogg`  | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `island_close`    | `island_close.ogg` | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `hover`           | `hover.ogg`        | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `click`           | não informado      | não informado               | não informado                             | não informado |
| `notification`    | não informado      | não informado               | não informado                             | não informado |
| `listening_start` | não informado      | não informado               | não informado                             | não informado |
| `listening_stop`  | não informado      | não informado               | não informado                             | não informado |
| `thinking`        | não informado      | não informado               | não informado                             | não informado |
| `success`         | `success.ogg`      | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `error`           | `error.ogg`        | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `wake`            | `wake.ogg`         | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `sleep`           | `sleep.ogg`        | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `happy`           | não informado      | não informado               | não informado                             | não informado |
| `drag_start`      | `drag_start.ogg`   | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `drag_end`        | `drag_end.ogg`     | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |
| `return_home`     | `return_home.ogg`  | Kenney Interface Sounds 1.0 | https://kenney.nl/assets/interface-sounds | CC0 1.0       |

O texto de licença incluído está em `public/sounds/License.txt`. Não foram usados arquivos de som ou arte do Coucou/Mochi.

## Dependências

As dependências declaradas em `package.json` mantêm suas próprias licenças e avisos. Antes de distribuir binários, gerar e revisar um inventário das dependências efetivamente empacotadas e incluir os avisos exigidos por cada licença.

### Crate dom_query

- `dom_query` 0.28.0, usado para converter HTML recebido em texto Markdown. Licença MIT; origem e documentação: https://github.com/niklak/dom_query e https://docs.rs/dom_query/0.28.0. Nenhum código-fonte foi copiado para o repositório.

```text
MIT License

Copyright (c) 2023 Mykola Humanov

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

This project contains portions of code and architectural concepts originally
derived from the "nipper" project (https://github.com/importcjj/nipper),
developed by Chen Jiaju and licensed under the MIT License and the Apache
License 2.0.
```

### Fonte Inter

- Inter Variable, pacote `@fontsource-variable/inter` 5.3.0. Autoria: The Inter Project Authors, liderado por Rasmus Andersson. Licença SIL Open Font License 1.1 (OFL); o texto integral acompanha o pacote em `node_modules/@fontsource-variable/inter/LICENSE`. Projeto: https://github.com/rsms/inter. Pacote: https://fontsource.org/fonts/inter.
- Os arquivos da fonte são empacotados localmente pelo Vite; não há carregamento por CDN. A fonte monoespaçada usada em trechos de código usa fontes do sistema e não adiciona arquivos de terceiros.

## Personagens 2D

Os modelos e a textura do pacote **Mini Characters (1.0)** foram criados e distribuídos por Kenney
(www.kenney.nl), em 17-07-2024. Licença Creative Commons Zero (CC0):
https://creativecommons.org/publicdomain/zero/1.0/. Atribuição não é obrigatória.
Texto da licença: `assets/pets/kenney-mini-characters/CC0-LICENSE.txt`.
Origem do pacote: https://kenney.nl/assets/mini-characters. Os 12 PNGs estáticos de 512×512 px e
as folhas WebP de animação (quadros de 192×192 px) são renderizações derivadas dos modelos GLB
originais e da textura externa, com câmera e iluminação uniformes e fundo transparente. Os PNGs
estáticos mostram o quadro central do clipe `idle`; os manifestos em
`assets/pets/kenney-mini-characters/animations/` identificam os clipes e os parâmetros de cada
folha. Não foram incluídos os modelos nem a textura no repositório. As renderizações derivadas
permanecem cobertas pela licença CC0 do pacote. O script de desenvolvimento é
`scripts/render-pets.mjs`; instruções de regeneração: `docs/ASSET-GUIDE.md`.

## Logotipos de provedores de IA

- 16 arquivos SVG de marca foram incorporados do pacote `@lobehub/icons-static-svg` 1.98.0, por LobeHub, sob licença MIT. Origem: https://github.com/lobehub/lobe-icons. Os arquivos foram copiados localmente para `src/assets/providers/`; os nomes dos provedores são marcas de seus respectivos titulares.
- MIT License (Copyright (c) 2023 LobeHub):

```text
MIT License

Copyright (c) 2023 LobeHub

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Phosphor Icons

- `@phosphor-icons/react` 2.1.10, por Phosphor Icons, sob licença MIT. Origem: https://github.com/phosphor-icons/react. Os ícones usados são importados individualmente por nomes para permitir tree-shaking; o componente local aplica o peso Light e a cor atual do texto.
- MIT License (Copyright (c) 2020 Phosphor Icons):

```text
MIT License

Copyright (c) 2020 Phosphor Icons

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
