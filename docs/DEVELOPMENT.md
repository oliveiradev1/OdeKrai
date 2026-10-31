# Desenvolvimento

## Requisitos

- Node.js 22.12 ou superior e npm para a interface, conforme `package.json`.
- Rust estável, Microsoft C++ Build Tools e WebView2 para compilar Tauri no Windows.

## Comandos

```powershell
npm install
npm run dev
npm run build
```

## Executável Windows de diagnóstico

O workflow manual **Build Windows diagnostic executable** compila em uma máquina limpa do GitHub Actions. Para iniciá-lo, abra a aba **Actions**, selecione o workflow, clique em **Run workflow** e escolha se `diagnostics` ficará habilitado. Com `false` (padrão), o executável não inclui DevTools; com `true`, use `Ctrl+Shift+I` no Windows para abrir o inspetor do WebView2.

Ao concluir, abra a execução do workflow e baixe o artefato `odekrai-windows-<sha-curto>`. Extraia o arquivo e execute `odekrai.exe`. O artefato contém o executável, `SHA256SUMS.txt` e este aviso em `LEIA-ME.txt`. O executável não é assinado, então o SmartScreen pode mostrar um aviso, e requer o Microsoft Edge WebView2 Runtime instalado.

A seção **Configurações > Sobre** mostra versão, commit curto, datas local e UTC do build, versões do WebView2 e Windows e licença. Os botões copiam essas informações e abrem a pasta de logs; o commit usa um comando Rust que constrói o endereço fixo do repositório.

Para compilar localmente sem empacotar instalador, use `npx tauri build --no-bundle`. O build de diagnóstico pode ser feito com `npx tauri build --no-bundle --features diagnostics`; a feature `diagnostics` ativa `tauri/devtools` apenas quando explicitamente passada. O workflow verifica que as features padrão não ativam DevTools.

O aplicativo grava `diagnostic.log` em `app_data_dir/logs`. O arquivo registra horários, etapas de inicialização e nomes, resultado e duração dos comandos; evento, comando e resultado precisam pertencer a listas fechadas. Detalhes gravados têm limite de tamanho e removem credenciais conhecidas, emails e o caminho do perfil do Windows, substituído por `%USERPROFILE%`. O log não registra mensagens de conversa nem texto livre dos erros do frontend. O banner **Diagnóstico** mostra mensagens amigáveis em português, agrupa causas iguais com contador (`x2`), mantém referência e causa técnica nos detalhes recolhidos e permite fechar cada aviso. Uma leitura bem-sucedida limpa o aviso do mesmo comando; o banner também oferece acesso à configuração da pasta quando `agent_home_status` falha. Se a execução anterior não concluiu a inicialização, o app mantém a janela principal aberta, desliga ilha e mascote nessa sessão e informa o modo seguro no mesmo banner.

`npm run check:window-api-acl` verifica chamadas diretas à API de janelas do Tauri no frontend contra as capabilities associadas às janelas. O CI executa essa verificação; ações adicionais de janela devem preferir comandos Rust validados pelo rótulo da janela.

## Estrutura

- `src/core`: configurações e acesso ao provedor.
- `src/ui`: Hub React e tema.
- `src-tauri`: shell nativo Tauri.
- `assets/odekrai`: organização de assets com controle de versão seletivo.
- `docs`: arquitetura, operação e status verificável.
