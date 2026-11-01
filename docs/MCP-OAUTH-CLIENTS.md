# Clientes OAuth para serviços públicos MCP

Os IDs de cliente são identificadores públicos do aplicativo e ficam em
`src-tauri/resources/oauth-clients.json`. O valor inicial é vazio. Segredos não pertencem a esse
arquivo: quando um serviço os exige, o usuário os cadastra na interface e o Rust os grava no
Gerenciador de Credenciais do Windows. Se o registro dinâmico retornar um segredo próprio do
cliente, o Rust também o guarda no registro OAuth do Credential Manager; ele não vai para a tela.

O Rust valida cada guia como HTTPS e compara seu host com `allowedGuideDomains` no mesmo recurso.
O link ainda passa por `open_external_link`, que usa o fluxo validado de links externos do app.
O segredo nunca é devolvido ao frontend; a interface recebe somente `clientSecretSaved`.

## O que as fontes oficiais confirmam

| Serviço          | Fonte oficial e trecho curto                                                                                                                                                                                                       | Conclusão para cliente desktop                                                                                                                                                                | Estado no OdeKrai                                                                                                                                                               |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GitHub           | [Configurar o GitHub MCP Server](https://docs.github.com/en/copilot/how-tos/copilot-in-your-ide/customize-copilot/extend-copilot-with-tools-and-context/set-up-the-github-mcp-server): “one-click OAuth authentication by default” | A página confirma OAuth padrão e PAT alternativo, mas não especifica se um cliente desktop independente pode usar apenas um ID público. **NÃO VERIFICADO.**                                   | ID vazio e DCR desativado; OAuth fica bloqueado até o ID do app ser configurado. PAT continua separado.                                                                         |
| Figma            | [Introdução ao servidor MCP](https://help.figma.com/hc/en-us/articles/39216419318551-Get-started-with-the-Figma-MCP-server): “Only clients listed in the Figma MCP catalog”                                                        | A página restringe a conexão remota a clientes listados e direciona novos clientes para a lista de espera. Aceitação de ID sem segredo por um cliente desktop externo: **NÃO VERIFICADO.**    | ID vazio e DCR desativado; a limitação de clientes aprovados continua valendo.                                                                                                  |
| Lovable          | [Servidor MCP Lovable](https://docs.lovable.dev/integrations/lovable-mcp-server): clientes locais são registrados “automatically”                                                                                                  | Clientes locais com callback loopback são registrados sem credenciais estáticas. A alternativa de cliente OAuth próprio pede ID e segredo; ID estático sem segredo não é o fluxo documentado. | DCR habilitado; ID vazio é aceitável somente quando o servidor anuncia registro e a consulta confirma esse recurso. A alternativa estática exige segredo no Credential Manager. |
| Notion           | [Criar um cliente MCP](https://developers.notion.com/guides/mcp/build-mcp-client): “supports dynamic client registration (RFC 7591)”                                                                                               | O fluxo desktop faz DCR. A documentação define `client_secret` como opcional na resposta; um cliente estático com ID sem segredo: **NÃO VERIFICADO.**                                         | DCR habilitado; conexão sem ID estático depende do endpoint anunciar o registro. Segredo do DCR fica no Credential Manager.                                                     |
| Google Workspace | [Configurar servidores MCP do Google Workspace](https://developers.google.com/workspace/guides/configure-mcp-servers): “OAuth client ID and secret”                                                                                | Os exemplos para MCP pedem os dois valores e usam cliente OAuth do tipo Web. A página não confirma fluxo desktop com ID sem segredo. **NÃO VERIFICADO; os exemplos exigem segredo.**          | ID vazio, DCR desabilitado e segredo requerido. O segredo só pode ser salvo depois que o ID público for incluído no recurso.                                                    |

### Limite conhecido

Os exemplos do Google registram um callback específico do cliente Web. O fluxo MCP atual do OdeKrai
usa um callback loopback em porta aleatória. A documentação consultada não confirma que esse callback
seja aceito para os servidores Google Workspace; por isso, essa integração permanece indisponível
até que o fluxo de callback seja confirmado e compatibilizado. Nenhuma credencial de cliente foi
incluída no repositório.
