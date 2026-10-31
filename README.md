![Banner do OdeKrai](banner.png)

[Português (Brasil)](README.md) · [English](README.en.md)

**Um assistente de IA para o seu Windows, com um mascote que fica ao seu lado.**

O OdeKrai conversa com o provedor de IA que você configurar e pode executar habilidades no computador dentro das permissões definidas por você. A interface reúne conversas, configurações e um mascote que acompanha o uso do aplicativo.

> Projeto em pré-lançamento (versão 0.15.0), para Windows.

## Destaques

- **Escolha a sua IA:** provedores de nuvem, opções com plano gratuito sinalizado e conexão com servidor local compatível.
- **Um mascote com nome:** 12 personagens com animações e modos Ilha e Livre.
- **Habilidades locais:** o agente pode executar ações registradas no aplicativo, sujeitas às permissões do Rust.
- **Agenda local:** organize eventos por dia e mês, configure recorrências e importe ou exporte arquivos `.ics`.
- **Google Calendar:** conecte o calendário principal em Habilidades > Agenda e lembretes > Gerenciar para consultar e gerenciar eventos com aprovação nas alterações. Consulte [a configuração e os escopos](docs/GOOGLE-CALENDAR.md).
- **Você no controle:** modos de permissão, aprovações, registro de atividade e kill switch.
- **Configurações locais:** credenciais de provedores são armazenadas pelo Rust no Gerenciador de Credenciais do Windows.

## O que o OdeKrai faz

### Conversas e provedores

O catálogo inclui OpenAI, Anthropic, Google Gemini, xAI, Mistral, Cohere, Groq, DeepSeek, Perplexity, Cerebras, Together AI, OpenRouter, Fireworks AI, Azure OpenAI e AWS Bedrock. Vertex AI aparece como guia de integração, sem autenticação integrada. A conexão local é compatível com servidores como Ollama e llama.cpp já iniciados.

O catálogo sinaliza opções gratuitas para Google Gemini, Mistral, Groq, Cerebras e OpenRouter; disponibilidade e limites dependem do provedor, modelo e conta. É possível configurar um provedor principal e reservas. O failover é limitado a falhas transitórias antes de haver saída observada; chamadas de ferramentas não são repetidas pelo failover.

As conversas são mantidas no histórico SQLite local. Para responder, o conteúdo enviado é encaminhado ao provedor configurado. O aplicativo também oferece respostas em streaming quando suportadas pelo adaptador, busca no histórico e abertura de links externos com confirmação por domínio.

### Agente e habilidades

Ative o modo agente em uma conversa para permitir que o modelo solicite habilidades registradas no Rust. As habilidades disponíveis incluem operações de leitura e organização de arquivos em pastas permitidas; criação, consulta, edição e remoção aprovada de eventos na Agenda local ou criação em um arquivo `.ics` escolhido; criação de arquivos Markdown, texto e CSV; abertura de links, arquivos e configurações do Windows; notificações, área de transferência, informações do sistema, volume e controles de mídia; alteração e restauração do papel de parede em pastas permitidas; bloqueio do Windows com aprovação obrigatória; rascunhos de e-mail; envio aprovado por webhook do Discord; leitura de páginas web; e delegação de código ao Codex CLI ou Claude Code.

As habilidades começam desativadas e podem ser habilitadas na tela Habilidades. Os resultados de conteúdo externo são tratados como não confiáveis, e ações com efeitos colaterais podem exigir aprovação conforme a política aplicada pelo Rust.

### Permissões e segurança

- **Aprovar por mim**, **Sempre perguntar** e **Controle total** são os modos disponíveis; bloqueios fixos continuam valendo em todos eles.
- Aprovações são apresentadas em janela própria. O atalho global configurado aciona o kill switch, que interrompe execuções e bloqueia novas ações até ser retomado.
- O registro de auditoria local encadeia entradas e bloqueia ações se uma gravação ou verificação falhar.
- O Rust valida habilidades, riscos, caminhos e permissões. O WebView apresenta o estado e encaminha decisões do usuário.

### Acesso rápido e desempenho

- **Início** reúne uma mensagem inicial, sugestões baseadas nos dados disponíveis, conversas recentes e um checklist de configuração. As sugestões preenchem o campo para revisão; não são enviadas automaticamente.
- **Ctrl+K** abre a paleta de comandos na janela focada. O painel de Atividade abre sob demanda; o modo Compacto expande a janela existente do mascote e oferece um campo de comando rápido.
- **Desempenho** tem os modos Automático, Ligado e Desligado. Automático ativa com movimento reduzido do Windows, até 8 GiB de RAM ou até 4 núcleos lógicos; Ligado reduz efeitos visuais e animações do mascote.

### Identidade, mascote e sons

O aplicativo oferece uma pasta pessoal, identidade `ODEKRAI.md` aprovada pelo usuário, notas de memória editáveis e perfil local. O mascote tem 12 personagens e animações associadas aos estados do aplicativo. Pode aparecer nos modos Ilha e Livre, com posição persistida. Há sons de interface configuráveis e opção para minimizar para a bandeja ao fechar a janela.

### Privacidade

Credenciais de provedores são guardadas pelo backend Rust no Gerenciador de Credenciais do Windows. O histórico, perfil e preferências usam armazenamento local; isso não significa que o banco local ou backups sejam criptografados. O conteúdo enviado em uma conversa vai ao provedor escolhido. Os logs de diagnóstico não incluem o conteúdo das conversas.

## Instalação e desenvolvimento

O projeto está em pré-lançamento. O bundle Windows configurado é NSIS. Para desenvolvimento, são necessários Windows, Node.js 22.12 ou superior, Rust e as ferramentas C++ da Microsoft, Windows SDK e WebView2 Runtime.

```powershell
npm ci
npm run tauri:dev
```

## Capturas de tela

As capturas ficam em `docs/screenshots/`.

## Licenças e créditos

Consulte [LICENSE](LICENSE), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) e [LICENSE-ASSETS.md](LICENSE-ASSETS.md) para os termos do código, dependências e assets. Não distribua assets privados do proprietário.
