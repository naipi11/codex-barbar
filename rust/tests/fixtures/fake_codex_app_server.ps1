[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet(
        'normal',
        'interleaved',
        'out-of-order',
        'unknown-notification',
        'duplicate-id',
        'invalid-json',
        'truncated',
        'oversized',
        'initialize-timeout',
        'rpc-timeout',
        'crash',
        'refuse-exit',
        'login-failed',
        'login-cancelled',
        'login-start-crash'
    )]
    [string] $Mode,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Remaining
)

$ErrorActionPreference = 'Stop'

function Write-Frame([object] $Value) {
    $json = $Value | ConvertTo-Json -Compress -Depth 32
    [Console]::Out.WriteLine($json)
    [Console]::Out.Flush()
}

function Write-Raw([string] $Text) {
    [Console]::Out.Write($Text)
    [Console]::Out.Flush()
}

function Response([object] $Id, [object] $Result) {
    Write-Frame @{
        id = [int64]$Id
        result = $Result
    }
}

function ErrorResponse([object] $Id, [int] $Code) {
    Write-Frame @{
        id = [int64]$Id
        error = @{
            code = $Code
            message = 'fixture error'
        }
    }
}

function AccountResult() {
    return @{
        account = @{
            type = 'chatgpt'
            email = 'fixture@example.invalid'
            planType = 'plus'
        }
    }
}

function RateLimitsResult() {
    return @{
        rateLimitsByLimitId = @{
            codex = @{
                primary = @{
                    usedPercent = 25
                    windowDurationMins = 300
                    resetsAt = 4102444800
                }
                secondary = @{
                    usedPercent = 10
                    windowDurationMins = 10080
                    resetsAt = 4102444800
                }
            }
        }
    }
}

$heldRequest = $null
$initializedNotificationSeen = $false

while ($true) {
    $line = [Console]::In.ReadLine()
    if ($null -eq $line) {
        if ($Mode -eq 'refuse-exit') {
            Start-Sleep -Milliseconds 100
            continue
        }
        break
    }

    if ([string]::IsNullOrWhiteSpace($line)) {
        continue
    }

    if ($Mode -eq 'invalid-json') {
        Write-Raw ('{"id":1,"result":' + "`n")
        continue
    }
    if ($Mode -eq 'truncated') {
        Write-Raw '{"id":1,"result":'
        break
    }
    if ($Mode -eq 'oversized') {
        Write-Raw (('x' * 1048577) + "`n")
        continue
    }

    try {
        $request = $line | ConvertFrom-Json
    } catch {
        Write-Raw '{"id":1,"error":{"code":-32700,"message":"invalid request"}}'
        continue
    }

    $id = $request.id
    $method = [string]$request.method

    if ($method -eq 'initialized' -and $null -eq $id) {
        $initializedNotificationSeen = $true
        continue
    }

    if ($method -eq 'initialize') {
        if ($Mode -eq 'initialize-timeout') {
            Start-Sleep -Seconds 60
            continue
        }
        if ([int64]$id -ne 1) {
            ErrorResponse $id (-32602)
            continue
        }
        if ($request.params.experimentalApi -ne $false) {
            ErrorResponse $id (-32602)
            continue
        }
        Response $id @{
            serverInfo = @{
                name = 'fake-codex'
                version = '0.0.0-test'
            }
        }
        continue
    }

    if ($method -eq 'account/read') {
        if (-not $initializedNotificationSeen) {
            ErrorResponse $id (-32001)
            continue
        }
        if ($Mode -eq 'crash') {
            exit 17
        }
        if ($Mode -eq 'rpc-timeout') {
            Start-Sleep -Seconds 60
            continue
        }
        if ($Mode -eq 'out-of-order') {
            if ($null -eq $heldRequest) {
                $heldRequest = $request
                continue
            }
            Response $id (AccountResult)
            if ([string]$heldRequest.method -eq 'account/read') {
                Response $heldRequest.id (AccountResult)
            } else {
                Response $heldRequest.id (RateLimitsResult)
            }
            $heldRequest = $null
            continue
        }
        if ($Mode -eq 'unknown-notification' -or $Mode -eq 'interleaved') {
            if ($Mode -eq 'interleaved') {
                Write-Frame @{
                    method = 'account/updated'
                    params = @{
                        marker = 'known'
                    }
                }
            }
            Write-Frame @{
                method = 'fixture/unknown'
                params = @{
                    marker = 'ignored'
                }
            }
        }
        Response $id (AccountResult)
        if ($Mode -eq 'duplicate-id') {
            Response $id (AccountResult)
        }
        continue
    }

    if ($method -eq 'account/rateLimits/read') {
        if (-not $initializedNotificationSeen) {
            ErrorResponse $id (-32001)
            continue
        }
        if ($Mode -eq 'crash') {
            exit 17
        }
        if ($Mode -eq 'rpc-timeout') {
            Start-Sleep -Seconds 60
            continue
        }
        if ($Mode -eq 'out-of-order') {
            if ($null -eq $heldRequest) {
                $heldRequest = $request
                continue
            }
            Response $id (RateLimitsResult)
            if ([string]$heldRequest.method -eq 'account/read') {
                Response $heldRequest.id (AccountResult)
            } else {
                Response $heldRequest.id (RateLimitsResult)
            }
            $heldRequest = $null
            continue
        }
        if ($Mode -eq 'unknown-notification' -or $Mode -eq 'interleaved') {
            if ($Mode -eq 'interleaved') {
                Write-Frame @{
                    method = 'account/updated'
                    params = @{
                        marker = 'known'
                    }
                }
            }
            Write-Frame @{
                method = 'fixture/unknown'
                params = @{
                    marker = 'ignored'
                }
            }
        }
        Response $id (RateLimitsResult)
        if ($Mode -eq 'duplicate-id') {
            Response $id (RateLimitsResult)
        }
        continue
    }

    if ($method -eq 'account/login/start') {
        if ($Mode -eq 'login-start-crash') {
            exit 17
        }
        $loginType = [string]$request.params.type
        if ($loginType -eq 'chatgpt') {
            Response $id @{
                loginId = 'login-browser'
                authorizationUrl = 'https://auth.openai.com/authorize?client=codex-barbar'
            }
            if ($Mode -eq 'login-failed') {
                Write-Frame @{
                    method = 'account/login/failed'
                    params = @{
                        loginId = 'login-browser'
                        error = 'fixture secret text'
                    }
                }
            } elseif ($Mode -eq 'login-cancelled') {
                Write-Frame @{
                    method = 'account/login/cancelled'
                    params = @{
                        loginId = 'login-browser'
                    }
                }
            } else {
                Write-Frame @{
                    method = 'account/login/completed'
                    params = @{
                        loginId = 'login-browser'
                    }
                }
            }
            continue
        }
        if ($loginType -eq 'chatgptDeviceCode') {
            Response $id @{
                loginId = 'login-device'
                verificationUrl = 'https://auth.openai.com/codex/device'
                userCode = 'ABCD-EFGH'
            }
            continue
        }
        ErrorResponse $id (-32602)
        continue
    }

    if ($method -eq 'account/login/cancel') {
        $cancelId = [string]$request.params.loginId
        Response $id @{
            cancelled = $true
        }
        Write-Frame @{
            method = 'account/login/cancelled'
            params = @{
                loginId = $cancelId
            }
        }
        continue
    }

    ErrorResponse $id (-32601)
}
