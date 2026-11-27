# Motor de bateria da ilha

A ilha usa as leituras dos protocolos suportados, do Windows e dos campos padrão HID. Quando um driver específico fica sem porcentagem, o motor tenta complementar a leitura sem duplicar ou confundir os dispositivos. A identificação e a atualização seguem automáticas, inclusive após reconexões.

Um receptor pode ser detectado sem fornecer bateria legível. Nesse caso, aparece com bateria indisponível. A presença do USB não comprova que o periférico esteja ligado. Nomes genéricos, dispositivos sem interface HID e protocolos não suportados ainda podem exigir diagnóstico adicional.

O diagnóstico local `data/battery-snapshot.json` registra a origem da leitura, IDs e observações. O motor não aprende protocolos desconhecidos sozinho nem envia comandos arbitrários. O X8 Pro ganhou interpretação de mensagens passivas do receptor BK compatível, ainda sem validação no hardware daquele usuário.

# Bateria automática do Zeus Pro

O motor de bateria identifica os periféricos compatíveis automaticamente. A ilha não exige um Zeus Pro: o perfil específico só aparece quando um Zeus Pro é detectado localmente pelo Bluetooth conectado ou por uma saída de áudio ativa com seu nome. Histórico salvo sozinho não ativa esse perfil. Nenhum cadastro ou identificação pessoal é necessário.

Quando disponível, abra o relógio, selecione a central de controles e expanda **Opções de bateria**, abaixo de volume e brilho. Essa seção começa recolhida. Sem o headset detectado, ela desaparece e a estimativa é pausada.

- **Bluetooth · Windows**: mostra o último nível informado pelo Windows, sem o símbolo de estimativa. A ilha verifica a conexão Bluetooth real, valida os dados recentes do motor de bateria e recalibra a base automaticamente.
- **Estimativa**: quando o Bluetooth se desconecta, desconta o tempo a partir da última base válida. O símbolo `≈` distingue esse cálculo da leitura do Windows.
- **Aguardando leitura**: Bluetooth conectado sem um nível válido disponível. O cartão não apresenta uma porcentagem estimada como se viesse do Windows.
- Se uma nova leitura Bluetooth aumentar, o cartão registra que o nível subiu e recalibra. Isso não prova o instante em que começou a carregar. O texto **Carregando** só aparece quando a fonte informa esse estado.

A verificação ocorre a cada 15 segundos; a tela pode levar aproximadamente 30 segundos para refletir uma mudança. O motor de bateria também tem seu próprio intervalo de consulta.

## Limites do receptor

O receptor USB não fornece bateria, carga ou estado de ligado/desligado. Sem Bluetooth, a ilha não distingue mudança para 2.4 GHz de um headset desligado. Use **Pausar** ao desligar ou carregar apenas pelo receptor; a pausa manual permanece mesmo após reconexão e reinicialização. Use **Retomar estimativa** para voltar à contagem.

Após uma carga completa sem Bluetooth disponível, use **Carreguei totalmente** e depois **Retomar estimativa**. Cargas parciais precisam de uma nova leitura Bluetooth para calibração automática.

## Autonomia e dados locais

A referência inicial de 18 horas pode ser ajustada de 1 a 100 horas. Ajustar a autonomia mantém a porcentagem atual e muda o ritmo da estimativa. A contagem retoma automaticamente após reinicialização quando existe base válida e não há pausa manual; períodos com o programa fechado e longos intervalos de suspensão não são descontados. O uso do headset longe do computador não pode ser contabilizado automaticamente.

O tempo e a última base Bluetooth são guardados em `data/zeus-estimate.json`, junto do executável portátil. As ações e transições são salvas imediatamente, e a contagem a cada minuto; uma interrupção inesperada pode perder até um minuto contado. O nível exibido pelo Windows pode ter atualização lenta ou passos de vários pontos percentuais, dependendo do headset.

Inspiração da estimativa linear: [Zeus Pro Suite](https://github.com/dorrigro/zeus-pro-suite). A implementação da ilha é própria e não instala esse programa de terceiros.

A calibração explícita `Nectar.exe --zeus-estimate-start=70` continua disponível para um nível conhecido, mas uma leitura Bluetooth válida tem prioridade. Não adicione esse argumento ao atalho de inicialização do Windows.
