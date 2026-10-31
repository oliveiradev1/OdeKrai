# Integração do mascote

O runtime usa exclusivamente o componente React `PetSprite`, imagens PNG e animações CSS. O catálogo `assets/pets/kenney-mini-characters/pets.json` define IDs, nomes, arquivos e textos alternativos. A seleção é persistida no SQLite pelo backend Rust e aparece no Início, no chat, na prévia do Assistente e nas janelas auxiliares.

## Estados e movimento

`src/core/pets.ts` mapeia estados do assistente para rótulos de animação. `src/ui/PetSprite.tsx` aplica o estilo correspondente e overlays de estado. Movimento reduzido é respeitado tanto pela preferência do aplicativo quanto por `prefers-reduced-motion`.

## Ferramenta de desenvolvimento

`scripts/render-pets.mjs` usa Three.js, disponível somente em `devDependencies`, para gerar PNGs a partir dos modelos Kenney externos no Windows. O script não é importado pelo frontend e seus modelos-fonte não são incluídos no app. Consulte [`ASSET-GUIDE.md`](ASSET-GUIDE.md) para regenerar os arquivos e [`LICENSE-ASSETS.md`](../LICENSE-ASSETS.md) para os direitos dos assets.

A apresentação dos sprites, a área de clique das janelas auxiliares e os estados animados ainda precisam de validação visual nativa no Windows.
