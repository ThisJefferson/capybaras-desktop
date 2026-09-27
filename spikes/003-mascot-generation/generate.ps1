# SPIKE 003 -- mascot generation via OpenRouter.
#
# Answers a direct question: can we just generate the capybara mascots instead of
# commissioning illustration? The answer should come from output, not opinion.
#
# Usage:
#   powershell -File generate.ps1 -Prompt "<text>" -Out "zeca.png" [-Model ...]

param(
    [Parameter(Mandatory = $true)][string]$Prompt,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Model = 'google/gemini-3-pro-image'
)

$ErrorActionPreference = 'Stop'

# Key stays in the environment. Never printed, never written to disk.
$key = $env:OPENROUTER_API_KEY
if (-not $key) { $key = [Environment]::GetEnvironmentVariable('OPENROUTER_API_KEY', 'User') }
if (-not $key) { Write-Host 'NO KEY'; exit 2 }

$payload = @{
    model      = $Model
    messages   = @(@{ role = 'user'; content = $Prompt })
    modalities = @('image', 'text')
} | ConvertTo-Json -Depth 10

$headers = @{
    Authorization  = "Bearer $key"
    'Content-Type' = 'application/json'
    'HTTP-Referer' = 'https://github.com/ThisJefferson/capybaras-desktop'
    'X-Title'      = 'Capybaras mascot spike'
}

$started = Get-Date
try {
    $resp = Invoke-RestMethod -Uri 'https://openrouter.ai/api/v1/chat/completions' `
        -Method Post -Headers $headers -Body $payload -TimeoutSec 300
}
catch {
    Write-Host ("REQUEST FAILED: " + $_.Exception.Message)
    if ($_.ErrorDetails.Message) { Write-Host ("  detail: " + $_.ErrorDetails.Message) }
    exit 1
}

$elapsed = [int]((Get-Date) - $started).TotalSeconds
Write-Host ("  model : " + $resp.model)
Write-Host ("  time  : ${elapsed}s")
if ($resp.usage) { Write-Host ("  usage : " + ($resp.usage | ConvertTo-Json -Compress)) }

# Images arrive either as a dedicated array or embedded in content parts.
$urls = @()
if ($resp.choices[0].message.images) {
    foreach ($i in $resp.choices[0].message.images) { $urls += $i.image_url.url }
}
if (-not $urls -and $resp.choices[0].message.content -is [array]) {
    foreach ($part in $resp.choices[0].message.content) {
        if ($part.type -eq 'image_url') { $urls += $part.image_url.url }
    }
}

if (-not $urls) {
    Write-Host '  NO IMAGE RETURNED'
    $text = $resp.choices[0].message.content
    if ($text -is [string]) { Write-Host ('  text: ' + $text.Substring(0, [Math]::Min(300, $text.Length))) }
    exit 1
}

$data = $urls[0]
if ($data -match '^data:image/(\w+);base64,(.*)$') {
    $bytes = [Convert]::FromBase64String($Matches[2])
    [System.IO.File]::WriteAllBytes($Out, $bytes)
    Write-Host ("  wrote : $Out  (" + $bytes.Length + ' bytes, ' + $Matches[1] + ')')
}
else {
    Invoke-WebRequest -Uri $data -OutFile $Out -TimeoutSec 120
    Write-Host ("  wrote : $Out  (downloaded, " + (Get-Item $Out).Length + ' bytes)')
}
