# Teste de atualização no Windows

Este roteiro verifica a atualização do OdeKrai sem apagar os dados da instalação anterior. Faça o teste em um computador de teste ou em uma conta de usuário de teste. Não use dados importantes como único exemplar.

## Antes de começar

Separe os instaladores da versão anterior e da versão nova, baixados das páginas oficiais de release do OdeKrai. Anote as duas versões. O instalador desta versão não tem assinatura digital; o SmartScreen pode mostrar **“O Windows protegeu o computador”**. Se isso acontecer, confira que o arquivo veio da release esperada antes de escolher **Mais informações > Executar assim mesmo**.

## Atualizar por cima

1. Instale e abra a versão anterior.
2. Crie uma conversa e envie uma mensagem de teste.
3. Crie um perfil com um nome fácil de reconhecer.
4. Configure uma pasta pessoal do OdeKrai e crie nela um arquivo de teste, se o fluxo estiver disponível.
5. Se possível, configure um provedor de IA e salve uma chave de teste. Depois de salvar, a chave não deve ser exibida novamente. As credenciais do provedor ficam no **Gerenciador de Credenciais do Windows**.
6. Feche o OdeKrai. **Não o desinstale.**
7. Execute o instalador da versão nova sobre a instalação existente e conclua a instalação.
8. Abra a versão nova pelo menu Iniciar.

## Conferir os dados e a atualização

- Confirme que a conversa e as mensagens criadas antes da atualização continuam na lista.
- Confirme que o perfil continua com o nome escolhido.
- Confirme que a pasta pessoal e o arquivo de teste continuam no local escolhido.
- Confirme que a configuração do provedor continua disponível. Para verificar a credencial, use o teste de conexão do provedor; não copie nem registre o valor da chave. Se não puder testar a conexão, registre apenas que a credencial ainda consta no Gerenciador de Credenciais do Windows.
- Na versão anterior, o aviso de nova versão deve aparecer depois do intervalo de 24 horas, se a verificação automática estiver ligada. Também é possível solicitar a consulta em **Configurações > Sobre > Verificar agora**. A consulta precisa de internet.
- No aviso, escolha **Ver e baixar**. Confirme que o navegador abre a página da release no GitHub. O OdeKrai não instala nem executa o instalador sozinho: o download e a instalação são feitos manualmente pelo usuário.

## Conferir a desinstalação

1. Abra **Configurações do Windows > Aplicativos > Aplicativos instalados**.
2. Localize o OdeKrai e escolha **Desinstalar**. Confirme a operação.
3. Confira que o aplicativo foi removido da lista e que não abre pelo menu Iniciar.
4. Confira os dados que devem permanecer para permitir uma reinstalação: os dados locais do aplicativo no perfil do Windows (incluindo conversas e perfil), a pasta pessoal do OdeKrai escolhida/criada pelo usuário e as credenciais guardadas no Gerenciador de Credenciais do Windows. O NSIS está configurado para instalação por usuário; esses dados ficam fora da pasta de instalação do programa.
5. Anote quais pastas ou atalhos sobraram. Não apague os dados locais, a pasta pessoal nem as credenciais durante este teste. A remoção desses dados, se desejada, deve ser uma decisão separada do usuário.

## Resultado do teste

Registre o Windows e sua versão, as versões anterior e nova, se cada item acima passou, as mensagens exibidas e o que permaneceu após a desinstalação. Não inclua chaves, senhas, conteúdo privado de conversas ou outros dados pessoais no relatório.

### O que o Codex não validou

O Codex não executou este roteiro em um Windows com instaladores reais. Portanto, continuam pendentes a instalação por cima, a preservação dos dados em uma atualização real, a apresentação do SmartScreen, a abertura da página da release pelo botão **Ver e baixar** e a desinstalação pelo Windows com conferência dos arquivos e credenciais que permanecem.
