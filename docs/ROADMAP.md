# Roadmap

1. **Fundação:** projeto reproduzível, interface, contratos e documentação.
2. **Hub desktop:** Tauri, preferências SQLite, idioma e credenciais pelo armazenamento seguro do sistema.
3. **Chat:** provedores com adaptadores próprios, streaming, histórico local e controles de custo.
4. **Companion:** ilha e janelas Windows, estados do personagem e carregador seguro de assets.
5. **Ferramentas e voz:** capacidades locais com permissões explícitas, auditoria e confirmação.
6. **Distribuição:** CI, testes Windows, instalador assinado quando houver certificado e documentação de atualização.

Cada fase depende de validação no Windows real antes de ser declarada pronta.

## Execução e hospedagem

O OdeKrai tem conectores de API específicos para ShardCloud e Hostinger. Eles não formam um motor
genérico de hospedagem de habilidades. As funções descritas abaixo são as que estão expostas pelo
registro de habilidades e pela tela Nuvem nesta versão.

| Provedor   | Leitura disponível                                                                                                                | Ações remotas disponíveis                                                                                                                                                                                                                                                                                                                                                             | Não disponível pelo OdeKrai                                                                                                                                                                                 |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ShardCloud | Listar VMs, consultar detalhes e métricas, listar distribuições.                                                                  | Iniciar/parar uma VM e executar de 1 a 10 comandos. Cada ação exige aprovação nativa; a lista de comandos é mostrada completa antes do envio. O comando usa `/`, tem timeout fixo de 30 segundos e a saída volta como dado externo não confiável.                                                                                                                                     | Projetos Docker dedicados, firewall, backups, criar/comprar, excluir ou reinstalar VPS e pagamentos.                                                                                                        |
| Hostinger  | VMs, detalhes, data centers, templates, ações recentes, backups; projetos Docker; firewalls/regras; chaves SSH públicas anexadas. | Iniciar/parar/reiniciar VPS; iniciar/parar/reiniciar projeto Docker; criar/atualizar projeto por Compose local; remover projeto; criar firewall/regra, ativar/desativar firewall e remover firewall/regra; restaurar backup; iniciar/parar modo de recuperação. Escritas exigem aprovação. Remoções e restauração também exigem confirmação por nome; restauração substitui os dados. | Criar/comprar VPS, pagamento, excluir/reinstalar VPS, restaurar snapshot, alterar chaves SSH e executar shell genérico. Compose é uma operação Docker específica e executa no servidor depois da aprovação. |

As ações da Hostinger relacionadas a projetos Docker seguem marcadas como experimentais na documentação
oficial; o OdeKrai limita o Compose a texto local de até 8 KiB e bloqueia montagens de caminhos de
sistema, mas a aprovação continua necessária porque o conteúdo é executado na VPS.

Criar ou comprar uma VPS e efetuar pagamentos não está disponível pelo OdeKrai em nenhum dos dois
provedores. O teto mensal mostrado para ShardCloud é apenas uma preferência local informativa: não
cria VPS, não envia limite à ShardCloud e não controla cobranças. Os outros provedores listados na
interface permanecem apenas informativos.

As datas e os limites da verificação documental estão em [`HOSTINGER-API.md`](HOSTINGER-API.md) e
[`SHARDCLOUD-API.md`](SHARDCLOUD-API.md).
