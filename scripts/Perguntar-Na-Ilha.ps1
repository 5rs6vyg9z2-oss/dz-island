param([Parameter(Mandatory)][string]$QuestionFile)
$ErrorActionPreference = 'Stop'
$questions = @(Get-Content -Raw -LiteralPath $QuestionFile | ConvertFrom-Json)
if ($questions.Count -lt 1 -or $questions.Count -gt 4) { throw 'Use de 1 a 4 perguntas.' }
foreach ($q in $questions) {
    if (!$q.question -or $q.question.Length -gt 600 -or @($q.options).Count -lt 2 -or @($q.options).Count -gt 4) { throw 'Cada pergunta precisa de texto e de 2 a 4 opções.' }
    foreach ($o in $q.options) { if ($o -isnot [string] -or !$o.Trim() -or $o.Length -gt 80) { throw 'Opção inválida (máximo de 80 caracteres).' } }
}
$config = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'data/agent.json') | ConvertFrom-Json
if ($config.port -lt 1 -or $config.port -gt 65535 -or !$config.token) { throw 'Integração local indisponível.' }
$body = @{name='Codex';project='DZ Island';tool='Pergunta';questions=@($questions | ForEach-Object { @{question=$_.question;options=@($_.options)} })} | ConvertTo-Json -Depth 8 -Compress
$bytes = [Text.Encoding]::UTF8.GetBytes($body)
if ($bytes.Length -gt 8192) { throw 'Perguntas excedem o limite da integração local.' }
$response = Invoke-RestMethod -Uri "http://127.0.0.1:$($config.port)/ask" -Method Post -Headers @{Authorization="Bearer $($config.token)"} -ContentType 'application/json; charset=utf-8' -Body $bytes -TimeoutSec 610
if ($response.decision -ne 'allow' -or @($response.choices).Count -ne $questions.Count) { @{status='cancelled'} | ConvertTo-Json -Compress; exit }
$answers = for ($i=0; $i -lt $questions.Count; $i++) {
    $choice = $response.choices[$i]
    if ($choice -isnot [long] -and $choice -isnot [int]) { throw 'Resposta inválida.' }
    if ($choice -lt 0 -or $choice -ge $questions[$i].options.Count) { throw 'Opção recebida fora dos limites.' }
    @{question=$questions[$i].question;answer=$questions[$i].options[$choice]}
}
@{status='answered';answers=@($answers)} | ConvertTo-Json -Depth 5 -Compress
