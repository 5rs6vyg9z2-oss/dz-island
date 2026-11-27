# Atualizações do DZ Island

Fonte oficial desta cópia: https://github.com/5rs6vyg9z2-oss/dz-island.

Cada push na branch `main` inicia o workflow **Build and publish Windows update**. Ele testa e compila a aplicação e o serviço de bateria, monta `DZ-Island-portable-windows-x64.zip` gera o instalador `DZ-Island-Setup.exe`, valida instalação e desinstalação e publica uma Release estável. A versão usa o major/minor de `package.json` e o número crescente da execução como patch: por exemplo, `1.11.23`. Falhas de build ou teste impedem a publicação.

O aplicativo consulta a última Release 20 segundos após abrir e, depois, a cada hora. Uma versão estável mais recente é baixada e instalada automaticamente; perguntas pendentes adiam a instalação.

São exigidos HTTPS, URL do repositório acima, nome fixo do pacote, limite de tamanho e SHA-256 do asset informado pela API do GitHub. Não se trata de uma assinatura de editor independente: a confiança vem do acesso HTTPS ao repositório e ao digest do GitHub. Proteja o acesso à conta e à branch `main`.

A substituição usa o instalador nativo `DZUpdater.exe`, sem executar scripts nem alterar políticas do Windows. A ilha só fecha depois que o instalador confirmar a validação do pacote. O instalador usa uma lista limitada de arquivos. `data/`, preferências, histórico e atalhos não são sobrescritos. Antes da troca, os arquivos atuais são movidos para `data/updates/<id>/backup` antes que os novos ocupem seus caminhos. Isso permite atualizar mesmo com uma integração de IA usando a imagem anterior do executável. Uma falha na troca restaura os arquivos já substituídos e relança a versão anterior. O resultado fica em `data/update-result.json` e os erros em `data/updates/<id>/error.txt`. Uma falha funcional que só apareça depois de abrir a nova versão exige restauração manual do backup.

Instalações antigas continuam recebendo um pacote de compatibilidade com o nome de executável anterior. Para adotar o novo nome, use o pacote portátil atual e aponte os atalhos para DZ-Island.exe.

## Validar antes de publicar

```powershell
bun install --frozen-lockfile
bun run tauri build --no-bundle
./scripts/test-portable-update.ps1
rustc --edition=2021 --test src-tauri/src/update_policy.rs -o update-policy-tests.exe
./update-policy-tests.exe
```

Os testes de instalação usam aplicativos simulados em uma pasta temporária. Não alteram a instalação do usuário.

O payload inclui o código principal compilado, WebView2Loader, serviço de bateria recompilado, HeadsetControl com hash fixado, avisos de licença e a ponte local de perguntas. Arquivos pessoais e tokens de `data/` nunca devem ser publicados.
