# DZ Island

Ilha dinâmica para Windows com o dragão Serginho, controles de conexões, indicadores de bateria e integração local com agentes de IA.

## Download para Windows 10 e 11 de 64 bits

[Baixar o instalador](https://github.com/5rs6vyg9z2-oss/dz-island/releases/latest/download/DZ-Island-Setup.exe) ou [baixar o DZ Island portátil](https://github.com/5rs6vyg9z2-oss/dz-island/releases/latest/download/DZ-Island-portable-windows-x64.zip). Na edição portátil, extraia toda a pasta e execute `DZ-Island.exe`. Os outros arquivos da pasta incluem o serviço de bateria e o atualizador.

Consulte o [guia de uso local](USO-LOCAL.md), as [configurações](SETTINGS.md) e as [versões no GitHub](https://github.com/5rs6vyg9z2-oss/dz-island/releases).

## Compilar

Com Rust, Bun e as ferramentas C++ do Windows instalados:

```powershell
bun install --frozen-lockfile
bun run tauri build --no-bundle
```

O GitHub Actions gera o pacote completo, incluindo o serviço de bateria. [Detalhes das atualizações](docs/AUTO-UPDATE.md).

## Licença

Distribuído sob a [GPLv3](LICENSE). Os avisos de autoria e as contribuições anteriores constam na licença, nos arquivos de origem e no histórico Git. A [licença do motor de bateria](vendor/batteryhub/LICENSE) e os [avisos das dependências](scripts/BATTERY-THIRD-PARTY-NOTICES.txt) acompanham o código.
