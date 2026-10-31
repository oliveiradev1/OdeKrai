# Changelog

Este arquivo segue o formato Keep a Changelog. Todas as versões listadas são pré-lançamentos SemVer 0.x.

## [Não lançado]

- Conexão Google Calendar com habilidades Rust separadas, aprovação de escritas e escopo `calendar.events.owned`.
- Habilidades de papel de parede com restauração pelo estado anterior e bloqueio do Windows com aprovação obrigatória; bloqueio negado para origem em conteúdo externo.

## [v0.16.0]

# v0.16.0: navegação nova, habilidades, Nuvem (ShardCloud) e aviso de atualização

Esta versão reorganiza o OdeKrai e traz as primeiras habilidades de agente que usam serviços externos.

Importante antes de instalar

- O instalador não é assinado digitalmente. O Windows pode mostrar "O Windows protegeu o computador". Clique em "Mais informações" e depois em "Executar assim mesmo".
- Esta é uma pré-lançamento. Algumas funções foram testadas só automaticamente; a parte visual e a de Windows ainda não foram validadas.

Novidades

- Navegação: sidebar com Início, Conversas, Agenda, Habilidades e Conectores de IA. Perfil no rodapé e Configurações em janela com busca.
- Início, Conversas com cartões de ação e desfazer, painel de Atividades, paleta de comandos (Ctrl+K).
- Habilidades organizadas por famílias, com busca e detalhes.
- Captura de tela e microfone sempre sob sua confirmação.
- Discord: mensagens por webhook, sempre com aprovação.
- Nuvem: ShardCloud (ver VMs; ligar e desligar com aprovação; comandos em lote aprovado; criação com limite mensal). Os demais provedores são planejados.
- Atualizações: aviso diário de versão nova com link para download. Não instala sozinho.
- Primeiro uso e modo desempenho.

Correções

- Confirmações (apagar conversa, memória, credencial) usam diálogo do próprio app.
- Links abrem no navegador pelo caminho validado.
- Habilidades não ficam vazias quando uma tem erro.
- Campos de token ficam em formulário, sem sugestão de salvar pelo navegador.

Limitações conhecidas

- Análise de imagens pela IA e voz em tempo real não estão disponíveis.
- ShardCloud não foi testado com conta real.
- Google Calendar, Gmail, Sheets e MCP não estão disponíveis.
- Erro 429 vem do provedor: confira o uso e o plano da sua conta.

Verificação

- Testes automáticos no CI (status no commit final).
- Validação visual e no Windows: pendente.

## [v0.15.0] — Ferramentas locais e análise visual — 2026-10-28

### Adicionado

- Agenda local com edição, recorrência e importação/exportação iCalendar; lembretes, planilhas e memória manual como habilidades do agente.
- Habilidades com limites para operações de arquivos e controles de áudio/mídia do sistema.
- Integração Discord por webhook cadastrado, com credencial no Gerenciador de Credenciais do Windows, prévia e aprovação antes do envio e revogação confirmada.
- Delegação de tarefas para Codex CLI ou Claude Code em `git worktree` temporário, sob aprovação.
- Inventário de hardware e arquivos GGUF do catálogo, com download confirmado, retomada e verificação SHA-256.
- Captura de tela local com prévia, salvamento separado e análise enviada a modelos permitidos somente após confirmação.
- Enquadramento JSON-RPC limitado como base técnica para MCP; transporte e integração de servidores não estão disponíveis.

### Alterado

- Catálogo de provedores, apresentação de conexão e mensagens de erro do agente revisados.
- Telas de Habilidades, Agenda, Computador e configurações ampliadas para os recursos acima.

### Segurança e limitações

- Decisões de ferramentas e permissões continuam no Rust; conteúdo externo permanece não confiável e ações de risco seguem o motor de aprovação.
- Captura e análise visual estão implementadas somente para Windows. Verificação visual no executável, múltiplos monitores, DPI, Credential Manager, contas reais de provedores/Discord e execução real das CLIs não foram validados neste marco.
- A IA local não inclui executor GGUF; o usuário precisa configurar um serviço compatível separado. As integrações Google são pesquisa, não conectores implementados.
- O workflow Windows é manual e produz artefato diagnóstico de curta retenção; ele não publica nem anexa binários a uma release.

## [v0.1.0] — Fundação

### Alterado

- Base inicial do aplicativo Tauri com interface React e Vite.
- Primeira integração do mascote e da ilha de trabalho.
- Estrutura inicial para conectores nativos e execução no Windows.

## [v0.2.0] — Provedores, janelas e histórico — Fase 1

### Alterado

- Estabilização de provedores, janelas e histórico.
- Metadados de autenticação Anthropic alinhados ao contrato utilizado.
- Base da Fase 1 registrada nos commits do marco.

## [v0.3.0] — Identidade e memória local — Fase 2

### Alterado

- Identidade local aprovada para o assistente.
- Memória manual local e template de identidade.
- Estrutura da pasta pessoal do agente.

## [v0.4.0] — Segurança e permissões — Fases 2.1, 3 e 3.1

### Alterado

- Políticas de risco e permissões implementadas em Rust.
- Aprovações, auditoria, kill switch e ferramentas de workspace.
- Controles de segurança e validações de encerramento das fases 3 e 3.1.

## [v0.5.0] — Nova interface e mascotes 2D

### Alterado

- Interface reorganizada e textos padronizados.
- Personagens 2D Kenney renderizados para o aplicativo.
- Seleção do mascote e escala da janela ajustadas.

## [v0.6.0] — Chat, conectores gratuitos e identidade unificada

### Alterado

- Conversas pesquisáveis em duas colunas.
- Catálogo de planos gratuitos e gestão de provedores conectados.
- Perfil e pasta pessoal integrados em uma identidade.

## [v0.7.0] — Estabilidade de inicialização e ícones Phosphor

### Alterado

- Leituras nativas de inicialização executadas de forma assíncrona.
- Ícones Phosphor aplicados aos elementos da interface.
- Fechamento e abertura de janelas revisados no histórico do marco.

## [v0.8.0] — Qualidade, CI Rust, tool calling e sons

### Alterado

- CI passou a verificar código Rust e lint.
- Adaptadores neutros para tool calling e capacidades exibidas por provedor.
- Manifesto, seleção e prévia de efeitos sonoros.

## [v0.9.0] — Endurecimento, refatoração e motor do agente

### Alterado

- Módulos de provedores, segurança e interface extraídos em unidades menores.
- Motor Rust limitado por turnos, tempo e tokens.
- Proteções fail-closed e remoção de caminhos de panic de produção.

## [v0.10.0] — Diagnóstico, travamentos e mascote animado

### Alterado

- Diagnóstico de inicialização e modo seguro apresentados na interface.
- Ciclo de vida de janelas auxiliares estabilizado.
- Mascote 2D animado com estados de interação.

## [v0.11.0] — Testes no Windows, privacidade do log, sons e monitor do pet

### Alterado

- Harness de janelas Tauri ajustado para testes no Windows.
- Logs com detalhes sensíveis reduzidos.
- Sons de eventos e posição do mascote persistida por monitor.

## [v0.12.0] — Roteador de provedores, tokens e IA local

### Alterado

- Failover de provedores com auditoria e limites diários de tokens.
- Conector OpenAI compatível para servidores locais já iniciados.
- Catálogo e escolha de modelos locais com confirmação para download.

## [v0.13.0] — Chat, pet e perfil

### Alterado

- Navegação do chat redimensionável e recolhível.
- Abertura de links validada no backend.
- Recuperação de identidade e ajustes no pet e no perfil.

## [v0.14.0] — Agente e habilidades

### Alterado

- Broker assíncrono de aprovações e registro Rust de habilidades.
- Loop Rust conectado às habilidades e aprovações existentes.
- Ferramentas limitadas de arquivos, calendário, rascunhos de email e busca web aprovada.

[v0.1.0]: https://github.com/Anceryx/OdeKrai/compare/5ccaef5...v0.1.0
[v0.2.0]: https://github.com/Anceryx/OdeKrai/compare/v0.1.0...v0.2.0
[v0.3.0]: https://github.com/Anceryx/OdeKrai/compare/v0.2.0...v0.3.0
[v0.4.0]: https://github.com/Anceryx/OdeKrai/compare/v0.3.0...v0.4.0
[v0.5.0]: https://github.com/Anceryx/OdeKrai/compare/v0.4.0...v0.5.0
[v0.6.0]: https://github.com/Anceryx/OdeKrai/compare/v0.5.0...v0.6.0
[v0.7.0]: https://github.com/Anceryx/OdeKrai/compare/v0.6.0...v0.7.0
[v0.8.0]: https://github.com/Anceryx/OdeKrai/compare/v0.7.0...v0.8.0
[v0.9.0]: https://github.com/Anceryx/OdeKrai/compare/v0.8.0...v0.9.0
[v0.10.0]: https://github.com/Anceryx/OdeKrai/compare/v0.9.0...v0.10.0
[v0.11.0]: https://github.com/Anceryx/OdeKrai/compare/v0.10.0...v0.11.0
[v0.12.0]: https://github.com/Anceryx/OdeKrai/compare/v0.11.0...v0.12.0
[v0.13.0]: https://github.com/Anceryx/OdeKrai/compare/v0.12.0...v0.13.0
[v0.14.0]: https://github.com/Anceryx/OdeKrai/compare/v0.13.0...v0.14.0
[v0.15.0]: https://github.com/Anceryx/OdeKrai/compare/v0.14.0...v0.15.0
[v0.16.0]: https://github.com/Anceryx/OdeKrai/compare/v0.15.0...v0.16.0
