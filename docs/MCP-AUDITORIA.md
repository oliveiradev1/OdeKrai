# Auditoria do fluxo MCP

## Escopo e conclusão

Esta auditoria descreve o estado do código na branch `main`, HEAD
`c4019a3f33d74d6a765e166fc1d7f5bb4b630167`. O MCP está parcialmente ligado: o usuário pode
cadastrar um servidor local e, após confirmação, iniciar o processo para testar a conexão e ver
`tools/list`. As ferramentas recebidas ainda não são registradas nem oferecidas ao agente; portanto,
o agente não as executa e não há aprovação de ações MCP por ferramenta.

| Etapa                                   | Existe?                         | Ligada?                                                     | Evidência e comportamento                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| --------------------------------------- | ------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Cadastro                             | Sim                             | Sim, pela tela de Privacidade e permissões                  | `src/ui/PrivacyConfigurations.tsx`: `addServer` envia o cadastro e eventuais segredos; `src-tauri/src/mcp_registry.rs`: `mcp_add_server` valida e persiste os dados, criando o servidor com `enabled: false`. `mcp_set_secret` guarda os valores no Windows Credential Manager. `mcp_list_servers` lista o estado.                                                                                                                                                                                                                                                    |
| 2. Conexão e handshake `initialize`     | Sim                             | Sim, apenas por “Testar conexão”, com confirmação explícita | `PrivacyConfigurations.tsx`: `testServer` mostra confirmação antes de chamar `mcp_test_connection`. `mcp_registry.rs`: `mcp_test_connection` exige `confirmed`, busca o cadastro e chama `inspect`; `inspect` recusa servidor desativado, recupera os segredos e chama `McpStdioClient::connect`. `mcp.rs`: `connect` inicia o processo, envia `initialize`, verifica a versão e a capacidade `tools`, e envia `notifications/initialized`. Ativar o servidor, por si só, não inicia o processo.                                                                      |
| 3. `tools/list`                         | Sim                             | Sim, durante o teste de conexão                             | `mcp_registry.rs`: `inspect` chama `McpStdioClient::tools_list` depois do handshake. `mcp.rs`: `tools_list` envia a requisição JSON-RPC `tools/list` e limita/marca a resposta como não confiável. O resultado é exibido na tela de configurações, não enviado ao agente.                                                                                                                                                                                                                                                                                             |
| 4. Registro em `security/skills.rs`     | Há apenas marcadores reservados | Não                                                         | `src-tauri/src/security/skills.rs` declara `SkillFamily::IntegracoesMcp` e `SkillOrigin::Mcp`, ambos anotados como reservados para uso futuro. Não há habilidades MCP dinâmicas em `registry()`. `skill_turn_offer` constrói a oferta a partir desse registro estático. `src-tauri/src/security/mod.rs`: `SimulatedAction` não tem variante MCP.                                                                                                                                                                                                                      |
| 5. Execução pelo agente com aprovação   | O cliente tem método de chamada | Não                                                         | `src-tauri/src/mcp.rs`: `tools_call`, `tools_call_timeout` e `tools_call_cancellable` implementam `tools/call`, mas não encontrei chamada de produção para esses métodos fora do próprio cliente. `src-tauri/src/agent_runtime.rs`: o agente recebe as ferramentas de `skill_turn_offer`; como o registro não inclui as ferramentas MCP, não há caminho de execução MCP do agente nem aprovação por chamada. A confirmação de “Testar conexão” na interface autoriza iniciar o processo local para inspeção; não é aprovação de uma ferramenta executada pelo agente. |
| 6. Resultado marcado como não confiável | Sim no cliente                  | Parcialmente ligado à interface; não ao agente              | `src-tauri/src/mcp.rs`: `truncate_untrusted_result` adiciona `_untrusted: true` e limita a resposta a `MAX_TOOL_RESULT_BYTES` (32 KiB); `tools_list` e `tools/call` passam por essa função. `PrivacyConfigurations.tsx` exibe a lista com o rótulo acessível “Ferramentas e formatos não confiáveis”. Como `tools/call` não está integrado ao agente, nenhum resultado de execução MCP chega ao contexto do agente por esse caminho.                                                                                                                                  |

## Cadastro e interface

`src/ui/PrivacyConfigurations.tsx`, componente `PrivacyConfigurations`:

- lista os servidores usando `mcp_list_servers`;
- cadastra nome, comando, argumentos e nomes de variáveis de ambiente por `mcp_add_server`;
- envia valores secretos separadamente por `mcp_set_secret`;
- liga/desliga o cadastro por `mcp_set_enabled`;
- pede confirmação mostrando o comando completo antes de chamar `mcp_test_connection`;
- mostra estado, erro e a resposta de ferramentas/formatos do teste;
- pede confirmação antes de remover com `mcp_remove_server`.

Em `src-tauri/src/lib.rs`, esses comandos são incluídos no `invoke_handler`. No backend,
`src-tauri/src/mcp_registry.rs` implementa os comandos e mantém o cliente conectado em
`McpRuntime`. A execução do programa ocorre no teste, não ao cadastrar: `mcp_add_server` grava
`enabled: false`, e `mcp_test_connection` requer confirmação e servidor habilitado.

## Transporte e versão MCP

O transporte implementado é **stdio**. `src-tauri/src/mcp.rs`, `McpStdioClient::connect`, inicia
o executável com `stdin`, `stdout` e `stderr` piped; o cliente troca mensagens JSON-RPC delimitadas
por linha. Não encontrei transporte HTTP no cliente MCP nem configuração de URL/endereço de servidor
na tela. O suporte a HTTP descrito na especificação não está implementado neste fluxo.

`src-tauri/src/mcp.rs` define `PROTOCOL_VERSION` como `2025-11-25`. O cliente envia esse valor no
campo `protocolVersion` da requisição `initialize` e rejeita a conexão se a resposta não repetir
exatamente a mesma versão. Portanto, o código não negocia uma versão diferente.

A especificação oficial da versão [2025-11-25 — Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
documenta `initialize`, a negociação do campo `protocolVersion` e a notificação
`notifications/initialized`. A página oficial [2025-11-25 — Transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
define stdio e Streamable HTTP como transportes distintos. A versão configurada no código foi
confirmada contra essas páginas oficiais.

## Limite da implementação atual

O cliente já conhece `tools/list` e `tools/call`, mas a ponte entre ferramentas MCP recebidas e o
registro central de habilidades não está implementada. Assim, “Conectado” significa que o processo
respondeu ao handshake e à listagem durante o teste; não significa que suas ferramentas estejam
disponíveis ao agente.
