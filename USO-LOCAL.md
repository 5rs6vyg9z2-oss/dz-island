# DZ Island portátil

Esta cópia local foi modificada em 21 de novembro de 2026. O dock substitui a barra do Windows enquanto estiver aberto. Ao sair pelo menu do DZ Island, a barra original é restaurada. O dock mostra os dispositivos Bluetooth conectados e os demais periféricos detectados pelo motor do BatteryHub. Aparelhos sem leitura aparecem apenas com o nome. O DZ Island abre o motor integrado em segundo plano e usa as mesmas leituras para indicadores, alertas e histórico.

## Abrir

Abra **Portatil\DZ-Island.exe**. No dock, use Iniciar, Pesquisa, Visão de tarefas, ícones ocultos e os aplicativos fixados. Use **Opções do DZ Island → Sair do DZ Island** para devolver a barra do Windows. O atalho **DZ Island** no Menu Iniciar abre esta cópia. Outro atalho com o mesmo nome na pasta de inicialização do usuário abre o DZ Island ao entrar no Windows. O perfil e as configurações locais ficam em **Portatil\data**. A economia máxima de memória está ativada. Ela desliga a aceleração gráfica do WebView e pode deixar efeitos e animações mais lentos.

## Bateria dos dispositivos

Clique em um indicador ou abra **Opções do DZ Island → Bateria dos dispositivos**. O menu dá acesso aos nomes, à ordem, ao histórico, aos alertas, aos limites de carga, à atualização manual e às opções de exibição do BatteryHub. Em Exibição, **Mostrar apenas no DZ Island** oculta os ícones separados. Quando quiser consultar o diagnóstico original, disponível em inglês, o executável em **Portatil\Battery** aceita **--list**, **--json** e **--dump**.

O motor, o HeadsetControl e suas licenças ficam em **Portatil\Battery**. O código do motor fica em **vendor\batteryhub**. A cópia separada do BatteryHub foi movida para a Lixeira. O perfil de bateria do Windows é reutilizado para conservar histórico e preferências.

## Remover

Feche o DZ Island. Remova o atalho **DZ Island** da pasta de inicialização (Win+R, `shell:startup`) e do Menu Iniciar (Win+R, `shell:programs`). Para excluir a versão portátil, mova **Portatil** para a Lixeira. Para remover também o código e os arquivos de compilação, mova a pasta do projeto inteira para a Lixeira.

O Visual Studio Build Tools é um componente de desenvolvimento instalado separadamente e pode ser desinstalado pelo Visual Studio Installer. O teste anterior do Windows Sandbox fica na pasta separada **dz-island-sandbox**.

## Eventos de agentes de IA

Com **Ilha → Agentes de IA → Status dos agentes** ativado, reinicie o DZ Island. O DZ Island prepara hooks locais para o Codex, Claude Code e Gemini CLI e preserva os demais hooks presentes nos respectivos arquivos de configuração. Os eventos chegam ao DZ Island por localhost.

No Codex, abra uma nova sessão e revise os hooks do DZ Island no menu de hooks. No CLI, o comando é **/hooks**. O Codex exige essa revisão antes de rodar hooks novos em uma configuração de usuário.

A ilha mostra início, perguntas que passaram pelos hooks, solicitações de permissão, avisos suportados pelo provedor e conclusão. Para perguntas com opções, o Codex e o Gemini CLI podem usar a ferramenta **ask_island**, e o Claude Code pode usar o fluxo de perguntas compatível. Clique numa opção na ilha para devolver a resposta ao agente. Aprovações de ferramentas, perguntas em texto livre e eventos sem suporte seguem no cliente de origem. Eventos exclusivos de outros clientes e eventos na nuvem que não passam por hooks locais ficam no cliente de origem.

Outros agentes podem informar estados observáveis executando `DZ-Island.exe agent working --name "Nome do agente"`, ou substituindo `working` por `needs-input`, `done` ou `error`.

## Atualizações

A edição portátil consulta versões estáveis no GitHub do DZ Island e adia a instalação enquanto houver uma pergunta de IA pendente. As configurações locais são preservadas. Consulte as [versões publicadas](https://github.com/5rs6vyg9z2-oss/dz-island/releases).
