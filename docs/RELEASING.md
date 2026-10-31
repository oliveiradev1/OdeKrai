# Processo de release

## Próxima versão

1. Escolha o próximo patch ou minor SemVer dentro de `0.x`, conforme a compatibilidade mantida. Atualize a mesma versão em `package.json`, `package-lock.json` (raiz), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (pacote `odekrai`) e `src-tauri/tauri.conf.json`.
2. Rode `npm run check:version` e atualize o `CHANGELOG.md` e `docs/releases/vX.Y.Z.md`. Registre somente mudanças comprovadas pelo código e pelos commits, a data real do commit de fechamento, limitações e validações pendentes.
3. Execute as verificações definidas no CI e revise o diff. Crie um commit de versão separado e uma tag anotada local apontada para esse commit, com título e resumo em português.
4. Revise `.github/workflows/build-windows.yml`: o workflow permanece um build diagnóstico, sem instalador, e o `odekrai.exe` produzido serve somente para validação. Depois que o workflow NSIS terminar, baixe o artefato da execução e extraia-o para uma pasta local. Valide essa pasta com `node scripts/check-release-artifact.mjs --release <pasta>`; ela deve conter somente o instalador NSIS e `SHA256SUMS.txt`.
5. Envie a tag individualmente, depois de revisar o commit e o nome: `git push origin vX.Y.Z`. Não use `git push --tags`; o publisher não envia tags. Confira que a tag existe no remoto e rode `npm run publish:releases -- --artifacts <pasta>` para simular. A simulação não chama a API do GitHub. Para executar, prepare `GITHUB_TOKEN` no ambiente e rode `npm run publish:releases -- --artifacts <pasta> --execute`. Acrescente `--final` para criar uma release normal; sem essa flag, ela será pré-lançamento.

O publisher não faz push, não cria tags e não publica nada sem `--execute`. Antes de acessar a API, compara o SHA-256 do instalador com o registro da pasta; a soma não pode ser divergente. A release já existente não é recriada e qualquer asset de mesmo nome já anexado é ignorado. Os dois arquivos validados são enviados como dados binários.

Para executar, o `GITHUB_TOKEN` precisa ter permissão de repositório **Contents: write**. A documentação da API do GitHub também informa que pode ser necessária permissão **Workflows: write** se o commit da tag incluir mudanças de workflow ainda não presentes na branch padrão. O token é lido somente do ambiente pelo publisher.

O corpo de uma release nova vem de `docs/releases/vX.Y.Z.md` e inclui o aviso fixo: “Instalador sem assinatura digital. O Windows pode mostrar o aviso 'O Windows protegeu o computador' (clique em Mais informações → Executar assim mesmo).” Pré-lançamentos nunca são marcados como `latest`. Uma release final só é marcada como `latest` se sua versão for superior às outras releases finais existentes. A data exibida na página do GitHub pode ser a data em que a release foi criada; por isso, cada nota registra separadamente a data real do commit do marco.

## Pré-lançamentos e SemVer 0.x

Por padrão, `publish-releases.mjs` cria releases como pré-lançamento e nunca as marca como `latest`. A flag `--final` cria uma release normal; só a de maior versão entre as releases finais existentes pode receber `latest`. Releases ainda em `0.x` continuam sendo pré-lançamentos por padrão enquanto os contratos públicos e os fluxos críticos não estiverem estabilizados.

O projeto permanece em `0.x` enquanto a API, os formatos locais e os fluxos de segurança ainda puderem mudar sem compatibilidade estável. Considere `1.0.0` quando houver contratos públicos definidos, política de compatibilidade, fluxos críticos validados em Windows e provedores suportados verificados com contas reais. O histórico de versões, por si só, não é critério de estabilidade.

## Artefato permitido

A release pública pode anexar exatamente dois arquivos: o instalador NSIS do Tauri, com nome
`OdeKrai_<versão>_x64-setup.exe`, e `SHA256SUMS.txt` contendo o hash SHA-256 desse instalador.
Não anexe o `odekrai.exe` solto do build de diagnóstico/teste, executáveis avulsos, README ou
outros arquivos. Rode `node scripts/check-release-artifact.mjs --release <pasta>` para validar o
conjunto. O verificador reprova qualquer `.exe` fora do padrão NSIS, além de GLB, FBX, OBJ e
`assets-src`, no modo `--release`. No modo padrão, usado pelo workflow diagnóstico, só é aceita a
pasta temporária exata com `odekrai.exe`, `SHA256SUMS.txt` e `LEIA-ME.txt`; isso não a torna um
artefato de release.

O instalador não é assinado. O Windows SmartScreen exibirá um aviso ao usuário. Esta versão não tem
atualização automática; novas versões serão obtidas pelas releases públicas. O workflow **Build
Windows diagnostic** permanece inalterado e continua gerando somente `odekrai.exe` para validação,
sem instalador e sem anexá-lo à release.

O workflow **Build Windows NSIS installer artifact** é acionado por tags `v*` ou manualmente e
somente gera um artefato da execução do Actions; não cria uma release do GitHub. O instalador usa
`currentUser` no NSIS para instalar no perfil atual sem exigir privilégios de administrador. A
assinatura de código e a geração de artefatos assinados do updater ficam desativadas.
