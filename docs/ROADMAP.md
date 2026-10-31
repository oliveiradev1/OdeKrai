# Roadmap

1. **Fundação:** projeto reproduzível, interface, contratos e documentação.
2. **Hub desktop:** Tauri, preferências SQLite, idioma e credenciais pelo armazenamento seguro do sistema.
3. **Chat:** provedores com adaptadores próprios, streaming, histórico local e controles de custo.
4. **Companion:** ilha e janelas Windows, estados do personagem e carregador seguro de assets.
5. **Ferramentas e voz:** capacidades locais com permissões explícitas, auditoria e confirmação.
6. **Distribuição:** CI, testes Windows, instalador assinado quando houver certificado e documentação de atualização.

Cada fase depende de validação no Windows real antes de ser declarada pronta.

## Execução e hospedagem

A execução hospedada de habilidades em nuvem continua planejada e não está disponível nesta versão.
A integração ShardCloud consulta VMs, detalhes, métricas e distribuições; o agente também pode
iniciar ou parar VMs obtidas pela lista da conta, sempre com aprovação nativa. Ela não executa
comandos dentro das VMs. Os outros provedores listados na interface continuam planejados. Uma futura
execução hospedada de habilidades deverá pedir aprovação para cada ação e aplicar limites de gasto.
