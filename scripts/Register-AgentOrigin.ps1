<#
.SYNOPSIS
Register an explicit ChatGPT conversation -> ACP/thread association in the local Agent Monitor.
.DESCRIPTION
Use this immediately after an AgentDock ACP create/resume result, with the exact returned ACP ID.
This script does not start, modify, stop or inspect a business agent. It only writes Monitor attribution.
The pairing key is read from this user's local Monitor directory and never printed or passed on a command line.
An existing association to a different conversation is rejected by the server.
.EXAMPLE
.\Register-AgentOrigin.ps1 -EntityId acps_0123456789abcdef -ConversationUrl https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc -Title 'My development conversation'
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidatePattern('^(acps_[a-zA-Z0-9]{8,64}|[0-9a-fA-F-]{36})$')][string]$EntityId,
    [Parameter(Mandatory=$true)][uri]$ConversationUrl,
    [ValidateLength(0,250)][string]$Title = ''
)
$ErrorActionPreference = 'Stop'
if ($ConversationUrl.Scheme -ne 'https' -or $ConversationUrl.Host -ne 'chatgpt.com' -or
    $ConversationUrl.UserInfo -or -not $ConversationUrl.IsDefaultPort -or
    $ConversationUrl.AbsolutePath -notmatch '/c/([0-9a-fA-F-]{36})$') {
    throw 'Use the exact HTTPS ChatGPT conversation URL, not a share link or a guessed ID.'
}
$conversationId = $Matches[1]
$parsedId = [guid]::Empty
if (-not [guid]::TryParse($conversationId, [ref]$parsedId)) { throw 'Invalid conversation UUID.' }
$keyPath = Join-Path $env:LOCALAPPDATA 'AgentMonitor\bridge.key'
if (-not (Test-Path -LiteralPath $keyPath)) { throw 'Start the installed Agent Monitor once before registering an origin.' }
$key = [System.IO.File]::ReadAllText($keyPath).Trim()
if ($key -notmatch '^[0-9a-f]{64}$') { throw 'The local pairing credential is invalid. Check the desktop setup page.' }
$body = @{entityId=$EntityId;conversationUrl=$ConversationUrl.AbsoluteUri;title=$Title} | ConvertTo-Json -Compress
try {
    $result = Invoke-RestMethod -Uri 'http://127.0.0.1:43217/v1/bind/manual' -Method Post `
        -Headers @{Authorization=('Bearer ' + $key)} -ContentType 'application/json; charset=utf-8' `
        -Body ([System.Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 8
    [pscustomobject]@{ok=$result.ok;entityId=$EntityId;conversationId=$result.conversationId;source=$result.source} | ConvertTo-Json -Compress
} finally {
    $key = $null
}
