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

Há dois transportes. O local continua em **stdio**: `src-tauri/src/mcp.rs`,
`McpStdioClient::connect`, inicia o executável com os fluxos padrão e usa mensagens JSON-RPC por
linha, com protocolo legado `2025-11-25` e handshake `initialize`. O remoto usa
**Streamable HTTP**, implementado em `src-tauri/src/mcp_http.rs` (`McpHttpClient`), com protocolo
`2026-07-28`: POST por chamada, cabeçalho `MCP-Protocol-Version`, metadados por requisição e
resposta JSON ou SSE. A versão moderna não usa o handshake nem sessões legadas. O código não faz
fallback automático para versões HTTP antigas.

**Confirmação oficial em 1º de novembro de 2026:** a página atual de Streamable HTTP identifica a
revisão `2026-07-28` e diz que ela removeu o endpoint GET e as sessões de protocolo. A mesma página
define que cada pedido JSON-RPC é um POST e que respostas podem ser JSON ou SSE. [Especificação
oficial — Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http).
Na página oficial de Autorização, a revisão relaciona OAuth 2.1 draft, Protected Resource Metadata
(RFC 9728), Authorization Server Metadata (RFC 8414), OIDC Discovery e outras normas; também diz
que clientes HTTP MCP devem descobrir o servidor de autorização usando Protected Resource Metadata
e suportar os mecanismos de descoberta indicados. [Especificação oficial — Autorização](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization).

O transporte HTTP também oferece autorização OAuth para servidores que publicam descoberta
Protected Resource Metadata e suportam registro dinâmico de cliente (DCR). O fluxo está em
`src-tauri/src/mcp_oauth.rs` (`discover_resource_metadata`, `discover_authorization_server`,
`register_client`, `run_authorization`, `accept_callback`, `exchange_code` e `refresh_access`); a
interface inicia `mcp_oauth_authorize` em `PrivacyConfigurations.tsx`. O cliente consulta o recurso
protegido, valida a origem anunciada, verifica igualdade exata do emissor, associa emissor, recurso,
state e PKCE ao fluxo, abre a autorização pelo comando Tauri validado e recebe um callback de uso
único em `127.0.0.1` numa porta aleatória. Redirects HTTP são recusados. Refresh tokens e metadados
de autorização ficam no Credential Manager; access tokens permanecem em memória. Um HTTP 401 tenta
renovar o token uma vez; se a renovação falhar, a interface recebe instrução para conectar novamente.

A conferência oficial foi feita em 1º de novembro de 2026 na revisão `2026-07-28`. A especificação
diz: “MCP clients MUST use OAuth 2.0 Protected Resource Metadata for authorization server
discovery.” Também exige `resource` nas solicitações de autorização e token e validação do `iss`
antes da troca do código. [Autorização MCP](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
e [descoberta do servidor de autorização](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization/authorization-server-discovery).

**Limitação de registro:** esta implementação usa DCR quando o servidor o anuncia e retorna erro
claro se o registro estiver indisponível. Ela ainda não suporta pre-registro/CIMD; se a metadata do
servidor anunciar suporte a CIMD, o OdeKrai não tenta DCR fora da prioridade definida pela
especificação. A especificação recomenda CIMD para clientes sem relação prévia, e classifica DCR
como mecanismo legado de compatibilidade. [Registro de cliente MCP](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization/client-registration).

A URL HTTPS continua fixa pelo cadastro do usuário e nunca é argumento de ferramenta do modelo.
Tokens pessoais e OAuth são guardados no Credential Manager e removidos ao excluir ou substituir a
autorização do servidor. Respostas são limitadas a 8 MiB no transporte e truncadas/sanitizadas antes
de serem oferecidas ao agente. A validação Rust ainda depende do CI, pois `cargo` não está disponível
no ambiente que produziu esta implementação.

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
