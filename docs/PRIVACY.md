# Privacidade

Preferências de aparência, Companion e configuração não secreta do provedor são salvas no armazenamento local do WebView. Credenciais suportadas são guardadas pelo processo Tauri no Gerenciador de Credenciais do Windows e não entram em `localStorage`.

O perfil (nome e apelido) fica em `profile.sqlite3` dentro dos dados locais do aplicativo. O avatar é reprocessado pelo Rust e salvo como PNG quadrado de 256 × 256 na mesma pasta. Esses arquivos ficam em texto simples; o aplicativo não os criptografa. O avatar não é adicionado ao histórico nem enviado aos provedores; o nome e o apelido só saem do dispositivo se o usuário os inserir numa mensagem. A escolha de atualizar o nome em `ODEKRAI.md` exige ação e aprovação explícitas.

Ao conversar, o texto enviado pelo usuário é encaminhado ao provedor selecionado. O teste de conexão consulta somente a lista de modelos documentada, quando disponível; o ping opcional envia apenas “ping” com limite de uma saída e pode gerar cobrança. As mensagens do chat são salvas em SQLite na pasta de dados local do aplicativo para permitir retomar conversas. Esse arquivo não é criptografado pelo OdeKrai; a proteção em repouso depende do Windows e da configuração do dispositivo. A política de retenção e uso do conteúdo enviado depende do provedor e da conta do usuário. Não envie dados que não deseja compartilhar com esse serviço.

A integração de credenciais depende do aplicativo nativo Windows. A prévia web não encaminha chamadas de IA.

ShardCloud oferece consultas de leitura e, pelo agente, ações de iniciar/parar VM e execução de
comandos remotos mediante aprovação nativa obrigatória.
Salvar o token não faz uma chamada de rede. Ele é enviado pelo cabeçalho `Authorization: Bearer` ao
domínio fixo `shardcloud.app` quando o usuário pede uma consulta ou aprova uma ação de iniciar/parar.
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
requisição e não envia cookies. Não há comandos para criar, excluir, redimensionar ou reinstalar VMs.

Quando “Verificar atualizações automaticamente” está ligado (padrão) ou o usuário escolhe “Verificar agora”, o app faz no máximo uma consulta a cada 24 horas com `GET https://api.github.com/repos/oliveiradev1/OdeKrai/releases/latest`, sem corpo. Os cabeçalhos definidos pelo app são `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2026-03-10` e `User-Agent: OdeKrai/<versão>`. Não são enviados token, cookie, nome, apelido, configuração, arquivos, identificador do dispositivo ou outro dado pessoal. Como em qualquer conexão à internet, o GitHub vê o endereço IP de origem e os metadados normais de rede da requisição. O OdeKrai não baixa nem executa o instalador; o botão “Ver e baixar” apenas abre a página oficial da release no navegador padrão.

Uma conversa pode ser apagada pelo menu da lista. Para apagar todo o histórico, abra Configurações → Dados e privacidade, avance pela primeira confirmação e digite `APAGAR` na segunda etapa. A exclusão remove os registros SQLite e executa `VACUUM`; cópias de segurança e conteúdo já enviado ao provedor permanecem fora do alcance dessa exclusão.

As respostas usam o ícone do aplicativo como avatar por padrão. A opção “Usar meu mascote como avatar do OdeKrai” troca esse ícone pelo pet selecionado; a preferência fica no armazenamento local do WebView.

Após configurar a pasta, `ODEKRAI.md`, `memory/` e a versão aprovada ficam em texto simples no dispositivo. O chat envia ao provedor as regras fixas, a identidade aprovada e até 1.024 caracteres de `memory/estilo.md`; as outras notas não entram automaticamente no prompt. Não coloque senhas, tokens ou dados bancários nesses arquivos. Uma pasta com aparência de OneDrive gera aviso de possível sincronização em nuvem.

As configurações do motor de permissões (modo efetivo, exceções por ferramenta, atalho do kill switch, prazo de aprovação e estado parado) ficam em `permissions.sqlite3` nos dados locais do aplicativo, sem criptografia feita pelo OdeKrai. Aprovações pendentes ficam apenas em memória e não sobrevivem ao encerramento do processo.

A preferência “Usar microfone” fica no armazenamento local do WebView. O OdeKrai só pede acesso ao microfone depois de o usuário escolher “Ativar” ou clicar para gravar com essa preferência já ligada, e o backend restringe a permissão à janela principal. A gravação permanece em memória enquanto não for descartada; nenhum áudio é enviado sem a confirmação explícita da transcrição.

Quando a pasta do agente está configurada, a auditoria grava JSONL diário em `logs/`, também em texto simples. Ela inclui ferramenta, risco, modo, origem, regra, decisão, resultado, latência e resumo dos parâmetros. Para mensagens simuladas e gravações, registra tamanho, SHA-256 e um trecho inicial curto; marcadores conhecidos de segredos ocultam esse trecho. Nenhuma mensagem é enviada por essas ferramentas nesta fase. Os logs não são removidos automaticamente e a interface ainda não oferece exclusão; o usuário pode removê-los no Explorador com o aplicativo fechado, o que quebra a cadeia verificável.
