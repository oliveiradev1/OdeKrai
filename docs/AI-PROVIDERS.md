# Conectores de IA

## Comportamento do catálogo

A aba **Conectados** lista credenciais salvas e, para IA local, um servidor encontrado nas portas padrão. **Gratuitos** inclui apenas provedores com plano gratuito confirmado no catálogo; **Todos** apresenta o catálogo completo. A aba escolhida fica salva localmente. Os cartões usam o tipo de API como subtítulo; a aba Conectados oferece controles de ativação, provedor principal e ações, enquanto as demais abas abrem os detalhes sem deslocar a grade. Desativar o provedor principal exige escolher outro provedor conectado e ativo. A remoção de credencial pede confirmação.

## Capacidades dos adaptadores

Capacidades de tool calling conferidas em 25/10/2026: Groq, Cerebras e Mistral documentam chamadas de ferramentas no formato Chat Completions; OpenRouter documenta o mesmo formato, mas a disponibilidade depende do modelo e pode ser consultada no catálogo do próprio provedor. Google Gemini documenta Function Calling na API nativa, porém a documentação de compatibilidade OpenAI não confirma esse recurso nesse endpoint; por isso, o catálogo mantém `tools: false` e `verified: false` para Gemini. O cache local por `providerId` e modelo continua registrando recusas explícitas de tools.

Fontes: [Groq Tool Use](https://console.groq.com/docs/tool-use), [Cerebras Tool Calling](https://inference-docs.cerebras.ai/capabilities/tool-use), [Mistral Function Calling](https://docs.mistral.ai/studio/conversations/function-calling), [OpenRouter Tool Calling](https://openrouter.ai/docs/guides/features/tool-calling) e [compatibilidade OpenAI do Gemini](https://ai.google.dev/gemini-api/docs/openai).

O transporte de tool calls está implementado para Anthropic Messages e OpenAI Chat Completions.

## Servidor de IA local

O conector **IA local** reutiliza Chat Completions e conecta um servidor OpenAI compatível já iniciado pelo usuário. A busca verifica Ollama em `127.0.0.1:11434`, llama.cpp server em `127.0.0.1:9931` (porta padrão atual documentada), e mantém `127.0.0.1:8080` para instalações configuradas nessa porta. Também é possível informar uma porta adicional entre 1024 e 65535; a consulta continua limitada a `127.0.0.1`. A busca consulta `/v1/models` e a conversa usa `/v1/chat/completions`. A chave é opcional e, quando informada, fica no Gerenciador de Credenciais do Windows.

Baixar um GGUF em “Meu computador” não instala nem inicia um servidor, e o arquivo ainda não é usado pelo OdeKrai. Para Ollama, a documentação oficial orienta criar um `Modelfile` cuja linha `FROM` aponta para o arquivo GGUF, executar `ollama create meu-modelo` na pasta desse arquivo e iniciar `ollama serve` se o servidor ainda não estiver ativo. Para llama.cpp, a documentação mostra `llama-server.exe -m <arquivo.gguf> -c 2048`; a porta padrão documentada é `127.0.0.1:9931`. Referências: [importar GGUF no Ollama](https://github.com/ollama/ollama/blob/main/docs/import.mdx), [FAQ do Ollama (endereço e porta padrão)](https://github.com/ollama/ollama/blob/main/docs/faq.mdx) e [guia do servidor llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).

HTTP é aceito apenas para `127.0.0.1`, `::1` ou `localhost` cuja resolução contenha exclusivamente IPs de loopback; os IPs aprovados são fixados no cliente da requisição. Outros hosts são rejeitados. Redirecionamentos são desativados, com timeout de conexão de 1 segundo e timeout total curto. Tools permanecem desativadas e não verificadas. A tela “Seu computador” pode baixar GGUF do catálogo após confirmação e conferir arquivo, tamanho e SHA-256 na pasta selecionada; o app não inicia o executor e não importa arquivos externos ao catálogo.

Documentação oficial dos caminhos usados: [compatibilidade OpenAI do Ollama](https://github.com/ollama/ollama/blob/main/docs/api/openai-compatibility.mdx) e [servidor OpenAI compatível do llama.cpp](https://github.com/ggml-org/llama.cpp/tree/master/tools/server).
Os tipos neutros incluem blocos de texto, chamadas e resultados; os adaptadores serializam esses
blocos, interpretam respostas completas e em streaming, validam argumentos JSON e expõem uso de
tokens quando a API retorna esses dados. O chat normal envia uma lista vazia de tools e não executa
ferramentas. O modo agente anuncia apenas ferramentas habilitadas e disponíveis no registro Rust;
nomes internos são mapeados para nomes aceitos pelo provedor e restaurados antes da validação e
execução local. No adaptador OpenAI, nomes de funções seguem o limite documentado de 64 caracteres;
o modelo GPT-6 Luna recebe `reasoning_effort: none` quando usa ferramentas. Respostas HTTP 400 são
classificadas apenas por categorias seguras extraídas do corpo e o corpo bruto não é registrado.

O catálogo marca tools como verificadas para Anthropic, OpenAI, Groq, Cerebras, Mistral e OpenRouter.
No OpenRouter, o suporte depende do modelo. Gemini pelo endpoint compatível com OpenAI permanece sem
suporte verificado porque a documentação consultada não confirma tools nesse endpoint. Se uma resposta
HTTP 400 indicar que o modelo não aceita tools, o par provedor/modelo é salvo no SQLite local e a
capacidade efetiva daquele modelo passa a ser “sem suporte”. Visão, geração de imagens e roteamento
não são capacidades implementadas nesta integração.

Referências oficiais conferidas em 29/10/2026: [Anthropic tool use](https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools),
[Anthropic streaming](https://platform.claude.com/docs/en/build-with-claude/streaming),
[OpenAI Chat Completions](https://developers.openai.com/api/reference/resources/chat) e
[OpenAI streaming de Chat Completions](https://developers.openai.com/api/docs/guides/streaming),
[function calling](https://developers.openai.com/api/docs/guides/function-calling),
[GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna) e
[definição de function tool](https://github.com/openai/openai-python/blob/main/src/openai/types/shared_params/function_definition.py).

## Adaptadores no código

O catálogo tem 17 itens: 13 APIs remotas e o conector local usam o formato Chat Completions; Anthropic usa Messages API; Perplexity usa Agent API; Vertex AI permanece sem autenticação ADC/OAuth e aparece como guia. A presença de um adaptador não comprova chave, modelo, conta ou serviço.

| Provedor                                                                         | Adaptador                                 | Autenticação e limites conhecidos                                                                         |
| -------------------------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| OpenAI                                                                           | Chat Completions                          | Bearer; ID de modelo manual                                                                               |
| Anthropic                                                                        | Messages API                              | `x-api-key` e `anthropic-version`; modelo manual                                                          |
| Gemini                                                                           | Endpoint OpenAI compatível                | Chave do Google AI Studio; modelo manual                                                                  |
| xAI, Mistral, Cohere, Groq, DeepSeek, Cerebras, Together, OpenRouter e Fireworks | Chat Completions                          | Bearer; modelo selecionado da lista oficial quando disponível                                             |
| Perplexity                                                                       | Agent API `/v1/agent`                     | Bearer; resposta `output[].content[].output_text`; modelo manual; busca web não é habilitada pelo ODEKRAI |
| Azure OpenAI                                                                     | Endpoint v1 `/openai/v1/chat/completions` | Cabeçalho `api-key`; nome do deployment enviado como modelo                                               |
| AWS Bedrock                                                                      | Chat Completions regional                 | Chave de API Bedrock; IAM/SigV4 não implementado                                                          |
| IA local                                                                         | Chat Completions em Ollama/llama.cpp      | HTTP somente em loopback; chave opcional; tools não verificadas                                           |
| Vertex AI                                                                        | Guia                                      | ADC/OAuth ainda não integrado                                                                             |

O processo Tauri valida HTTPS, host e caminho por provedor remoto antes de enviar a chave; a exceção do servidor local verifica e fixa IPs de loopback. Redirecionamentos são desativados e credenciais não são registradas. Endpoint, modelo e região ficam separados por `providerId`; a migração v1 preserva a configuração existente. A listagem de modelos é habilitada apenas para endpoints cuja documentação foi verificada. Provedores sem endpoint de listagem são identificados no catálogo; seu teste de conexão fica parcial e não envia geração. O chat usa streaming para Anthropic e adaptadores Chat Completions, incluindo o servidor local; a Agent API da Perplexity ainda usa resposta completa. Não há failover nem medição de uso.

Na tela de conectores, as abas filtram credenciais salvas, provedores com plano gratuito confirmado e o catálogo completo. Groq, Google Gemini, Mistral, Cerebras e OpenRouter mostram o selo de plano gratuito com links para as páginas oficiais de limites ou preços; no OpenRouter, o selo se refere aos modelos marcados com `:free`. As condições e os modelos podem variar por conta e mudar conforme o provedor. A interface não exibe limites numéricos fixos. Ao abrir os detalhes de um plano gratuito, o app avisa que alguns provedores podem usar os dados enviados nesses planos.

O catálogo versionado em `src-tauri/resources/provider-catalog.json` registra endpoint, caminho, cabeçalho de autenticação, forma de listar modelos, campos específicos, URL de documentação e data de conferência. Entradas sem confirmação documental permanecem `unverified` e não recebem botão de listagem. Modelos são selecionados da conta quando possível ou podem ser digitados manualmente; não há modelo padrão preenchido.

Credenciais ficam no Gerenciador de Credenciais do Windows pelo crate `keyring`. O frontend nunca lê a credencial já armazenada. Essa integração exige execução nativa Windows; preview web não envia chamadas com chaves.

Nos detalhes de cada conector, “Criar conta” abre a página de cadastro ou entrada do provedor e “Gerar chave” abre o console de credenciais apropriado. O app usa o plugin oficial Tauri Opener e limita os domínios externos aos endereços dos provedores catalogados. Os consoles podem exigir autenticação ou encaminhar para uma etapa adicional dentro do painel.

O filtro “Conectados” consulta as credenciais em sequência para evitar consultas concorrentes ao Gerenciador do Windows. Se alguma consulta falhar, a tela informa que o resultado pode estar incompleto em vez de afirmar que não há credenciais.

## Fontes oficiais consultadas

- [OpenAI Chat Completions](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- [Anthropic Messages](https://platform.claude.com/docs/en/api/messages/create)
- [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai)
- [Cohere Compatibility API](https://docs.cohere.com/docs/compatibility-api)
- [Perplexity Agent API](https://docs.perplexity.ai/api-reference/agent-post)
- [Cerebras Chat Completions](https://inference-docs.cerebras.ai/api-reference/chat-completions) e [lista de modelos](https://inference-docs.cerebras.ai/api-reference/models/list-models)
- [Limites do Groq](https://console.groq.com/docs/rate-limits), [Gemini](https://ai.google.dev/gemini-api/docs/rate-limits), [Mistral](https://docs.mistral.ai/admin/billing-usage/usage-limits), [preços do Cerebras](https://inference-docs.cerebras.ai/support/pricing) e [preços/modelos gratuitos do OpenRouter](https://openrouter.ai/pricing)
- [Azure OpenAI v1](https://learn.microsoft.com/en-us/azure/foundry/openai/latest)
- [Amazon Bedrock Chat Completions](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions.html)
- [Vertex AI ADC](https://docs.cloud.google.com/docs/authentication/application-default-credentials)

Todos os 16 provedores do catálogo exibem um SVG de marca local. Os arquivos foram incorporados de `@lobehub/icons-static-svg` 1.98.0, sob MIT; os provedores sem variações de cor usam seus símbolos monocromáticos.

Os links de cadastro, chaves, limites e documentação abrem no navegador padrão por meio do plugin oficial `opener` do Tauri. A janela principal só pode abrir endereços HTTPS; se o sistema não conseguir iniciar o navegador, o painel mostra o endereço para cópia manual. Essa integração precisa de validação visual e nativa no Windows.
