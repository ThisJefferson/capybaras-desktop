# SPIKE 003 -- evaluate generated mascots with a vision model.
#
# The workspace's usual vision script is prompted for phone screenshots, so it
# answers the wrong question about illustration. This asks its own.

param(
    [Parameter(Mandatory = $true)][string[]]$Images,
    [Parameter(Mandatory = $true)][string]$Question,
    [string]$Model = 'google/gemini-3.7-flash'
)

$ErrorActionPreference = 'Stop'

$key = $env:OPENROUTER_API_KEY
if (-not $key) { $key = [Environment]::GetEnvironmentVariable('OPENROUTER_API_KEY', 'User') }
if (-not $key) { Write-Host 'NO KEY'; exit 2 }

$content = @()
$labels = @()
$n = 1
foreach ($img in $Images) {
    if (-not (Test-Path $img)) { Write-Host "missing: $img"; exit 1 }
    $b64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($img))
    $content += @{ type = 'text'; text = "IMAGE $n ($([System.IO.Path]::GetFileName($img))):" }
    $content += @{ type = 'image_url'; image_url = @{ url = "data:image/png;base64,$b64" } }
    $labels += $img
    $n++
}
$content += @{ type = 'text'; text = $Question }

$payload = @{
    model    = $Model
    messages = @(@{ role = 'user'; content = $content })
} | ConvertTo-Json -Depth 14

$resp = Invoke-RestMethod -Uri 'https://openrouter.ai/api/v1/chat/completions' -Method Post -Headers @{
    Authorization  = "Bearer $key"
    'Content-Type' = 'application/json'
    'HTTP-Referer' = 'https://github.com/ThisJefferson/capybaras-desktop'
    'X-Title'      = 'Capybaras mascot evaluation'
} -Body $payload -TimeoutSec 300

$resp.choices[0].message.content
Write-Host ''
Write-Host ("[model " + $resp.model + " | cost " + $resp.usage.cost + "]")
