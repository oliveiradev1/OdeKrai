# Privacidade

Preferências de aparência, Companion e configuração não secreta do provedor são salvas no armazenamento local do WebView. Credenciais suportadas são guardadas pelo processo Tauri no Gerenciador de Credenciais do Windows e não entram em `localStorage`.

O perfil (nome e apelido) fica em `profile.sqlite3` dentro dos dados locais do aplicativo. O avatar é reprocessado pelo Rust e salvo como PNG quadrado de 256 × 256 na mesma pasta. Esses arquivos ficam em texto simples; o aplicativo não os criptografa. O avatar não é adicionado ao histórico nem enviado aos provedores; o nome e o apelido só saem do dispositivo se o usuário os inserir numa mensagem. A escolha de atualizar o nome em `ODEKRAI.md` exige ação e aprovação explícitas.

Ao conversar, o texto enviado pelo usuário é encaminhado ao provedor selecionado. O teste de conexão consulta somente a lista de modelos documentada, quando disponível; o ping opcional envia apenas “ping” com limite de uma saída e pode gerar cobrança. As mensagens do chat são salvas em SQLite na pasta de dados local do aplicativo para permitir retomar conversas. Esse arquivo não é criptografado pelo OdeKrai; a proteção em repouso depende do Windows e da configuração do dispositivo. A política de retenção e uso do conteúdo enviado depende do provedor e da conta do usuário. Não envie dados que não deseja compartilhar com esse serviço.

A integração de credenciais depende do aplicativo nativo Windows. A prévia web não encaminha chamadas de IA.

Servidores MCP remotos podem usar token pessoal ou autorização OAuth. No OAuth, o OdeKrai consulta
os metadados HTTPS do recurso MCP e do emissor anunciado, registra um cliente quando o servidor
oferece DCR e abre a página de consentimento no navegador padrão depois de o usuário iniciar a
conexão. O emissor recebe a solicitação de autorização, o identificador do recurso MCP, os escopos
solicitados e o endereço IP da conexão; o servidor MCP recebe o access token Bearer nas chamadas
autorizadas. Refresh tokens e metadados da autorização ficam no Gerenciador de Credenciais do
Windows; access tokens ficam somente em memória. O callback local usa `127.0.0.1`, porta aleatória,
state e PKCE e só é aceito uma vez. O usuário pode remover o servidor para apagar sua autorização.
Se os metadados do emissor fornecerem um endpoint de revogação, ao remover o servidor o OdeKrai
envia a ele o refresh token e, se ainda estiver em memória, o access token. Mesmo se a revogação
falhar, a credencial local e a conexão em memória são removidas; o app mostra um aviso para revogar
o token no painel do provedor.

Ao conectar o GitHub por OAuth, o OdeKrai solicita somente o escopo `read:user` para obter o login
da conta pelo endpoint fixo `https://api.github.com/user`; a chamada usa o access token apenas no
Rust, não segue redirecionamentos e não guarda a resposta bruta. O login fica junto à autorização
no Gerenciador de Credenciais. As chamadas MCP do GitHub enviam `X-MCP-Readonly: true` para limitar
as ferramentas disponibilizadas pelo servidor às de leitura. O GitHub recebe o endereço IP normal
da conexão e o pedido autenticado de perfil. Desconectar apaga a autorização local e tenta a
revogação anunciada pelo emissor; quando ela não estiver disponível ou falhar, o app avisa que a
revogação remota não foi confirmada.

O GitHub exige o segredo do GitHub App/OAuth App para trocar o código de autorização. A pessoa o
informa na tela de Habilidades; o OdeKrai o guarda no Gerenciador de Credenciais, nunca no SQLite,
no log ou no contexto do modelo, e limpa o campo após salvar. O segredo fica disponível ao usuário
local do dispositivo, por isso use um app dedicado ao OdeKrai e não um segredo compartilhado com
outros sistemas. Ao desconectar, o app envia o access token ao endpoint oficial de revogação do
GitHub (`DELETE /applications/{client_id}/grant` em `api.github.com`), autenticando com o client ID
e o segredo do app; o endereço IP fica visível ao GitHub como em qualquer pedido HTTPS. O token e o
segredo não são registrados em logs. Se o GitHub não confirmar a revogação, o app remove as
credenciais locais e mostra um aviso para revogar no GitHub.

ShardCloud oferece consultas de leitura de VMs, detalhes, métricas e distribuições e, pelo agente,
ações de iniciar/parar VM e execução remota de comandos mediante aprovação nativa obrigatória.
O OdeKrai não oferece projetos Docker dedicados, firewalls ou backups para ShardCloud. Também não
cria/compra, exclui ou reinstala VPS, nem efetua pagamentos.
Salvar o token não faz uma chamada de rede. Ele é enviado pelo cabeçalho `Authorization: Bearer` ao
domínio fixo `shardcloud.app` quando o usuário pede uma consulta ou aprova uma ação de iniciar,
parar ou executar comandos em uma VM.
O token fica no Gerenciador de Credenciais do Windows, não é salvo no SQLite, não aparece novamente
na interface e não entra em logs, auditoria, mensagens de erro ou contexto de conversa. Nomes,
descrições, endereços IP, estados e métricas retornados são dados externos não confiáveis e ficam
temporariamente em memória para exibição e para limitar a seleção às VMs listadas pela própria
credencial. O modelo fornece somente o nome; o Rust confirma nome e ID e mostra ambos na aprovação.
Parar pode interromper serviços em execução. A auditoria registra a habilidade, decisão e resultado
sem o token. A ShardCloud recebe a rota consultada e o endereço IP normal da conexão. Ao aprovar
`shardcloud.vm_exec`, o conteúdo integral dos comandos é enviado à API ShardCloud na VM selecionada;
a execução usa o diretório `/`, até 10 comandos e limite fixo de 30 segundos por comando. Comandos
aprovados podem ler ou alterar dados da VM e podem usar a rede conforme as permissões dela; revise
todo o texto mostrado antes de aprovar. O resultado é devolvido ao modelo como saída externa não
confiável, com limite de 64 KiB. O aplicativo não anexa arquivos ou conversas automaticamente à
requisição e não envia cookies. Não há comandos no OdeKrai para criar, comprar, excluir, redimensionar
ou reinstalar VMs, restaurar backups ou pagar por serviços. A preferência de teto mensal da
ShardCloud fica localmente e não é enviada nem aplicada pela API.

A integração Hostinger permite consultar VMs, detalhes, data centers, templates, ações recentes,
backups, projetos Docker, firewalls/regras e chaves SSH públicas anexadas. O agente pode iniciar,
parar ou reiniciar VPS; operar projetos Docker; criar ou alterar firewalls/regras; iniciar/parar modo
de recuperação e restaurar backups. Essas ações são enviadas somente após aprovação nativa; remoção
de recursos e restauração também exigem confirmação por nome. O Compose local aprovado é enviado
integralmente à API Hostinger para ser executado na VPS. Não há ferramenta de shell genérico. Criar
ou comprar, excluir ou reinstalar VPS, restaurar snapshots, acessar a área de pagamento ou efetuar
cobranças não está disponível pelo OdeKrai.

O token Hostinger fica no Gerenciador de Credenciais do Windows e não volta a ser exibido. A
documentação oficial informa que ele herda as permissões da conta que o criou; portanto pode
alcançar a conta inteira, inclusive informações de faturamento. O token só é enviado como
`Authorization: Bearer` ao host fixo `developers.hostinger.com` durante uma leitura ou ação escolhida
pelo usuário, e o endereço IP de origem fica visível à Hostinger como em qualquer conexão HTTPS. Os
parâmetros de consulta incluem apenas os nomes/identificadores de recursos selecionados e, para uma
ação, os campos necessários à operação aprovada. Não há chamadas de compra ou pagamento.

Quando “Verificar atualizações automaticamente” está ligado (padrão) ou o usuário escolhe “Verificar agora”, o app faz no máximo uma consulta a cada 24 horas com `GET https://api.github.com/repos/oliveiradev1/OdeKrai/releases/latest`, sem corpo. Os cabeçalhos definidos pelo app são `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2026-03-10` e `User-Agent: OdeKrai/<versão>`. Não são enviados token, cookie, nome, apelido, configuração, arquivos, identificador do dispositivo ou outro dado pessoal. Como em qualquer conexão à internet, o GitHub vê o endereço IP de origem e os metadados normais de rede da requisição. O OdeKrai não baixa nem executa o instalador; o botão “Ver e baixar” apenas abre a página oficial da release no navegador padrão.

Uma conversa pode ser apagada pelo menu da lista. Para apagar todo o histórico, abra Configurações → Dados e privacidade, avance pela primeira confirmação e digite `APAGAR` na segunda etapa. A exclusão remove os registros SQLite e executa `VACUUM`; cópias de segurança e conteúdo já enviado ao provedor permanecem fora do alcance dessa exclusão.

As respostas usam o ícone do aplicativo como avatar por padrão. A opção “Usar meu mascote como avatar do OdeKrai” troca esse ícone pelo pet selecionado; a preferência fica no armazenamento local do WebView.

Após configurar a pasta, `ODEKRAI.md`, `memory/` e a versão aprovada ficam em texto simples no dispositivo. O chat envia ao provedor as regras fixas, a identidade aprovada, até 1.024 caracteres de `memory/estilo.md` e os nomes/estados das habilidades registrados localmente, para explicar quais ações estão habilitadas, desativadas ou aguardam configuração. No modo agente, também envia as descrições das habilidades efetivamente oferecidas; no chat normal, nenhuma ferramenta é enviada. As outras notas não entram automaticamente no prompt. Não coloque senhas, tokens ou dados bancários nesses arquivos. Uma pasta com aparência de OneDrive gera aviso de possível sincronização em nuvem.

As configurações do motor de permissões (modo efetivo, exceções por ferramenta, atalho do kill switch, prazo de aprovação e estado parado) ficam em `permissions.sqlite3` nos dados locais do aplicativo, sem criptografia feita pelo OdeKrai. Aprovações pendentes ficam apenas em memória e não sobrevivem ao encerramento do processo.

A preferência “Usar microfone” fica no armazenamento local do WebView. O OdeKrai só pede acesso ao microfone depois de o usuário escolher “Ativar” ou clicar para gravar com essa preferência já ligada, e o backend restringe a permissão à janela principal. A gravação permanece em memória enquanto não for descartada; nenhum áudio é enviado sem a confirmação explícita da transcrição.

Quando a pasta do agente está configurada, a auditoria grava JSONL diário em `logs/`, também em texto simples. Ela inclui ferramenta, risco, modo, origem, regra, decisão, resultado, latência e resumo dos parâmetros. Para mensagens simuladas e gravações, registra tamanho, SHA-256 e um trecho inicial curto; marcadores conhecidos de segredos ocultam esse trecho. Nenhuma mensagem é enviada por essas ferramentas nesta fase. Os logs não são removidos automaticamente e a interface ainda não oferece exclusão; o usuário pode removê-los no Explorador com o aplicativo fechado, o que quebra a cadeia verificável.
