# Catálogo de serviços públicos

O catálogo de serviços públicos tem uma única fonte de dados: [`public-mcp-catalog.json`](../src-tauri/resources/public-mcp-catalog.json). O backend valida o arquivo no comando `mcp_public_catalog` e o fornece à tela Habilidades. A interface não mantém cópia dos nomes, fornecedores, links, requisitos ou permissões dos serviços.

Cada item contém os campos obrigatórios `id`, `name`, `provider`, `connectionType`, `status`, `officialLink`, `requirements` e `permissions`. O arquivo também contém metadados operacionais para os fluxos existentes, como endpoint fixo, métodos de autenticação e dados de configuração OAuth. Não coloque segredos nesse catálogo.

## Tipos de conexão

- `one_click`: autorização pelo navegador, sem token pessoal digitado.
- `client_id`: depende de um identificador público de cliente configurado pelo desenvolvedor.
- `personal_token`: autenticação por token pessoal fornecido pelo usuário.
- `informational`: o cartão só apresenta informações e documentação.

Um serviço pode aceitar mais de um tipo, como OAuth e token pessoal.

## Estados do catálogo

- `available`: fluxo de conexão pode ser configurado.
- `in_preview`: serviço em prévia do fornecedor.
- `configuration_pending`: falta um requisito de configuração ou habilitação do cliente.
- `not_verified`: documentação ou disponibilidade ainda não foi confirmada.

Esses estados descrevem a oferta do catálogo. A tela calcula separadamente o estado operacional (Disponível, Configure o serviço, Conectando, Conectado ou Erro) a partir dos servidores salvos; só um teste MCP bem-sucedido pode resultar em Conectado.

## Validação

`npm run check:mcp-catalog` valida o esquema mínimo, links HTTPS, domínios oficiais, fontes e tipos de conexão. O comando não acessa a rede. O teste automatizado também reprova um serviço sem link oficial, um domínio de endpoint não permitido e um link de terceiro sem justificativa.

As conexões de contas reais não são executadas por essa validação. Consulte os requisitos e permissões atuais no arquivo JSON e nas páginas oficiais apontadas por cada item.
