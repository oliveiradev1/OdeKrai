# Auditoria do fluxo MCP

## Escopo e conclusão

Esta auditoria descreve o fluxo depois de MCP-B, sobre a base inicial
`3d4304f097b482c90417fa29d52e8bab32f47bfa`. O usuário cadastra um servidor local e, após confirmação,
inicia o processo para testar a conexão e consultar `tools/list`. Ferramentas com schema aceito
podem ser ativadas individualmente (todas começam desativadas) e oferecidas ao agente. Cada chamada
é validada no Rust e exige aprovação nativa por ser comunicação externa.

| Etapa                                   | Existe? | Ligada?                                                      | Evidência e comportamento                                                                                                                                                                                                                                                                                                                                                       |
| --------------------------------------- | ------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Cadastro                             | Sim     | Sim, pela tela de Privacidade e permissões                   | `src/ui/PrivacyConfigurations.tsx`: `addServer` envia cadastro e eventuais segredos; `src-tauri/src/mcp_registry.rs`: `mcp_add_server` persiste com `enabled: false`; `mcp_set_secret` usa o Credential Manager.                                                                                                                                                                |
| 2. Conexão e handshake `initialize`     | Sim     | Sim, somente por “Testar conexão”, com confirmação explícita | `mcp_test_connection` exige `confirmed` e chama `inspect`; `McpStdioClient::connect` envia `initialize`, confere versão/capacidade `tools` e envia `notifications/initialized`.                                                                                                                                                                                                 |
| 3. `tools/list`                         | Sim     | Sim, durante o teste de conexão                              | `inspect` chama `McpStdioClient::tools_list`; a resposta passa por `truncate_untrusted_result`. `tool_catalog` converte a lista em entradas locais e valida nome e schema.                                                                                                                                                                                                      |
| 4. Registro em `security/skills.rs`     | Sim     | Sim, depois da conexão confirmada e ativação individual      | `skill_turn_offer` inclui ferramentas conectadas; `SkillTurnOffer::from_registry_with_mcp` oferece só entradas disponíveis e explicitamente ativadas. `tool_catalog` gera `mcp.<servidor>.<ferramenta>` e recusa colisões e schemas inválidos/não suportados.                                                                                                                   |
| 5. Execução pelo agente com aprovação   | Sim     | Sim, para cada chamada                                       | `RuntimeBackend::execute_tool` roteia para `OfferedSkill::Mcp`; `McpToolCall` recebe `Risk::ComunicacaoExterna` em `dispatch::action_metadata`. `policy::decide` exige aprovação nos três modos. `McpExternalTool::preview` exibe servidor, ferramenta e argumentos completos antes da aprovação; a execução passa por `mcp_registry::execute_tool_action` somente depois dela. |
| 6. Resultado marcado como não confiável | Sim     | Sim                                                          | `tools_call` aplica `_untrusted: true` e limita o resultado a 32 KiB; o agente envolve a saída em bloco não confiável e propaga `tainted` às ações seguintes. `RuntimeBackend::execute_tool` registra apenas hash/tamanho do resultado. `dispatch::audit_params` guarda IDs e hash/tamanho dos argumentos, sem argumentos em claro.                                             |

## Cadastro e interface

`src/ui/PrivacyConfigurations.tsx`, componente `PrivacyConfigurations`:

- lista servidores por `mcp_list_servers`;
- cadastra nome, comando, argumentos e nomes de variáveis de ambiente por `mcp_add_server`;
- envia valores secretos separadamente por `mcp_set_secret`;
- liga/desliga o cadastro por `mcp_set_enabled`;
- pede confirmação com o comando antes de chamar `mcp_test_connection`;
- exibe estado, erros, nomes/descrições externos e ferramentas encontradas;
- mostra schemas inválidos e colisões como indisponíveis;
- oferece um interruptor por ferramenta disponível, desativado por padrão;
- pede confirmação antes de remover com `mcp_remove_server`.

Em `src-tauri/src/lib.rs`, os comandos MCP são incluídos no `invoke_handler`. No backend,
`src-tauri/src/mcp_registry.rs` mantém clientes em `McpRuntime`. O cadastro não inicia o processo:
`mcp_add_server` cria com `enabled: false`, e `mcp_test_connection` exige confirmação e servidor
habilitado. Desativar ou remover o servidor encerra e descarta seu cliente conectado.

## Transporte e versão MCP

O transporte implementado é **stdio**. `src-tauri/src/mcp.rs`, `McpStdioClient::connect`, inicia o
executável com `stdin`, `stdout` e `stderr` piped; mensagens JSON-RPC são delimitadas por linha. Não
há transporte HTTP no cliente MCP nem configuração de URL/endereço de servidor na tela.

`src-tauri/src/mcp.rs` define `PROTOCOL_VERSION` como `2025-11-25`. O cliente envia esse valor em
`initialize` e recusa a conexão se a resposta não repetir exatamente a versão. Portanto, o código
não negocia uma versão diferente. A especificação oficial da versão [2025-11-25 — Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
documenta `initialize`, negociação de `protocolVersion` e `notifications/initialized`. A página
oficial [2025-11-25 — Transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
define stdio e Streamable HTTP como transportes distintos. A versão configurada no código foi
confirmada contra essas páginas oficiais.

## Limites e segurança

O validador `validate_tool_schema` / `validate_tool_arguments` em `src-tauri/src/mcp.rs` aceita um
subconjunto fechado de JSON Schema. Keywords não reconhecidas (por exemplo, `pattern`, `$ref` e
combinadores) deixam a ferramenta indisponível em vez de serem ignoradas. Argumentos têm limite de
30 KiB e profundidade local; resultados têm limite de 32 KiB. Anotações textuais do schema são
retiradas antes de apresentar o schema ao modelo; descrições e resultados do servidor são dados não
confiáveis, nunca instruções. Ativação é associada ao hash do schema descoberto, então uma alteração
do schema volta a deixar a ferramenta desativada. Anotações de risco do servidor aparecem apenas
como sugestão na tela e não definem o risco: toda chamada MCP é comunicação externa e exige aprovação
na janela nativa em qualquer modo. O log de auditoria não registra argumentos em claro; a prévia de
aprovação os mostra antes da decisão do usuário.
