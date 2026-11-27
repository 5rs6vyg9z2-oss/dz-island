# Descoberta de bateria

O motor mantém a ordem dos drivers específicos, necessária porque alguns fabricantes compartilham IDs USB. Se um driver específico não entregar porcentagem válida, tenta aproveitar a propriedade Bluetooth do Windows e os campos HID padronizados, preservando a identidade do dispositivo. Uma porcentagem válida não é substituída por uma leitura alternativa.

A associação exige VID/PID e um único dispositivo físico identificado pelo Windows. Estados explícitos de desligado, suspensão e níveis qualitativos são preservados. Isso evita atribuir a bateria de um receptor ao outro.

Interfaces HID com nomes de dispositivos sem fio, ou IDs de receptores BK conhecidos, podem aparecer mesmo sem bateria. A presença da interface USB não comprova que o periférico esteja ligado. Esses registros mostram bateria indisponível; não recebem porcentagens inventadas. Periféricos identificados apenas por áudio, com nomes genéricos ou sem descritor acessível podem exigir suporte adicional.

O arquivo local `data/battery-snapshot.json` inclui `source`, `vid`, `pid` e `note`. As origens são `vendor`, `vendor-passive`, `windows-bluetooth`, `standard-hid` e `unavailable`. O diagnóstico detalhado existente do motor (`--dump`) continua disponível. Não são enviados dados a servidores.

## Attack Shark

O receptor BK 1D57:FA60/FA65 envia mensagens espontâneas. Foram acrescentados nomes de modelos e interpretação de estados da família B, incluindo o X8 Pro (código 08). A porcentagem exige duas mensagens iguais; zero é aceito, e valores acima de 100 ou estados desconhecidos são rejeitados. Nenhum comando de configuração é enviado. Testes de protocolo passam, mas o X8 Pro não foi validado fisicamente neste computador.

Referências de protocolo e tabela de modelos (GPL-3.0):
- https://github.com/lucasbrito-wdt/attack-shark-configurator/blob/main/docs/protocol/family-b.md
- https://github.com/lucasbrito-wdt/attack-shark-configurator/blob/main/crates/shark-core/src/platform/bk/registry.rs

## Custo das consultas

Os nomes HID são reutilizados a partir da enumeração já necessária, evitando reabrir cada interface para buscar o mesmo nome em vários drivers. O vínculo entre interfaces e dispositivos físicos é consultado uma vez por interface por ciclo e descartado no próximo ciclo. A leitura passiva BK processa no máximo 64 mensagens por ciclo; as tentativas iniciais mantêm o prazo existente de cinco segundos. Os eventos de reconexão e a agenda de consultas existente continuam ativos, sem varredura contínua.

## Validação

Testes portáveis: `battery_discovery_test.cpp`, `attack_shark_battery_test.cpp`, `json_output_test.cpp`, `refresh_policy_test.cpp` e `xbox_bluetooth_identity_test.cpp`. A compilação completa do motor e da aplicação valida a integração Windows.
