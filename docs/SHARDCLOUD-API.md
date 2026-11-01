# ShardCloud: API e funções disponíveis no OdeKrai

## Verificação da documentação

Documentação oficial consultada em **2026-11-01**. As páginas abertas não informam uma data de
publicação ou de última atualização; essa data é **NÃO VERIFICADA**. A conferência confirma que a
API usa token Bearer, que sua base oficial é `https://shardcloud.app/api` e que a documentação
oficial lista operações de VMs, incluindo criação, remoção, métricas, início/parada, comandos e
backups. A existência dessas rotas na API não significa que estejam implementadas ou disponíveis no
OdeKrai.

Fontes oficiais:

- [Welcome / API reference](https://docs.shardcloud.app/api-reference/introduction)
- [Índice oficial da documentação](https://docs.shardcloud.app/llms.txt)
- [List your VMs](https://docs.shardcloud.app/api-reference/vms/list-your-vms)
- [Start the VM](https://docs.shardcloud.app/api-reference/vms/start-the-vm)
- [Create a VM](https://docs.shardcloud.app/api-reference/vms/create-a-vm)
- [Run shell commands inside the VM](https://docs.shardcloud.app/api-reference/vms/run-shell-commands-inside-the-vm)
- [List a project's backups](https://docs.shardcloud.app/api-reference/project/list-a-projects-backups)
- [Restore a backup](https://docs.shardcloud.app/api-reference/project/restore-a-backup)

As páginas oficiais de detalhes, métricas, distribuições, parada e execução de comandos não
carregaram individualmente durante esta consulta. Seus caminhos abaixo são observados no código do
OdeKrai; a confirmação individual do contrato desses endpoints fica **NÃO VERIFICADA**. As páginas
oficiais list-your-VMs e start-the-VM foram abertas e confirmam `GET /api/vms` e
`POST /api/vms/{vm_id}/start`, respectivamente. O site oficial documenta o stream SSE da execução de
comandos com limite de até 50 comandos; o OdeKrai envia no máximo 10, com timeout fixo definido no
Rust.

## O que o OdeKrai implementa

| Ação no OdeKrai           | Operação observada no código         | Risco e aprovação                              | Estado                                                                                                                                                             |
| ------------------------- | ------------------------------------ | ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Testar conexão/listar VMs | `GET /api/vms`                       | Leitura                                        | Disponível após salvar token; a resposta é tratada como dado externo não confiável.                                                                                |
| Detalhes da VM            | `GET /api/vms/{id}`                  | Leitura                                        | Disponível para VM que apareceu na lista consultada.                                                                                                               |
| Métricas                  | `GET /api/vms/{id}/timeseries`       | Leitura                                        | Há comando no Rust; a confirmação da rota exata na documentação oficial não foi verificada nesta consulta.                                                         |
| Distribuições             | `GET /api/vms/distros`               | Leitura                                        | Disponível; a confirmação da rota exata na documentação oficial não foi verificada nesta consulta.                                                                 |
| Iniciar/parar VM          | `POST /api/vms/{id}/start` e `/stop` | Aprovação nativa obrigatória, em qualquer modo | Disponível para VM obtida da lista. O Rust confirma nome e ID.                                                                                                     |
| Executar comandos         | `POST /api/vms/{id}/exec`            | Aprovação nativa obrigatória, em qualquer modo | Disponível em lote de até 10; usa diretório `/`, timeout fixo de 30 segundos por comando, bloqueios locais adicionais e saída limitada/tratada como não confiável. |
| Projetos Docker           | —                                    | —                                              | **Não disponível** como habilidade própria no OdeKrai.                                                                                                             |
| Firewalls                 | —                                    | —                                              | **Não disponível** no OdeKrai.                                                                                                                                     |
| Backups                   | —                                    | —                                              | **Não disponível** no OdeKrai, apesar de a API documentar endpoints de backup.                                                                                     |
| Criar/comprar VM ou pagar | —                                    | Financeiro                                     | **Não disponível** no OdeKrai. O teto local não é enviado nem aplicado pela ShardCloud.                                                                            |
| Excluir/reinstalar VM     | —                                    | —                                              | **Não disponível** como ação própria no OdeKrai.                                                                                                                   |

O registro de habilidades e os comandos correspondentes ficam em
[`security/skills.rs`](../src-tauri/src/security/skills.rs) e
[`shardcloud.rs`](../src-tauri/src/shardcloud.rs). Os testes de contrato HTTP locais estão no módulo
de testes de `shardcloud.rs`; eles usam servidor simulado e não fazem chamadas à ShardCloud real.
