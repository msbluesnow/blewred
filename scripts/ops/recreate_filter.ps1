$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$ws = New-Object System.Net.WebSockets.ClientWebSocket
$cts = New-Object System.Threading.CancellationTokenSource
$ws.ConnectAsync([Uri]'ws://127.0.0.1:4455', $cts.Token).Wait(2000) | Out-Null

$buf = New-Object byte[] 65536
$seg = [System.ArraySegment[byte]]::new($buf)

# Recv Hello
$res = $ws.ReceiveAsync($seg, $cts.Token)
$res.Wait(2000) | Out-Null
$helloJson = [System.Text.Encoding]::UTF8.GetString($buf, 0, $res.Result.Count) | ConvertFrom-Json

$salt = $helloJson.d.authentication.salt
$challenge = $helloJson.d.authentication.challenge
$password = if ($env:OBS_PASSWORD) { $env:OBS_PASSWORD } else { "" }

$sha256 = [System.Security.Cryptography.SHA256]::Create()
$passSaltBytes = [System.Text.Encoding]::UTF8.GetBytes($password + $salt)
$secret = [Convert]::ToBase64String($sha256.ComputeHash($passSaltBytes))
$secretChallengeBytes = [System.Text.Encoding]::UTF8.GetBytes($secret + $challenge)
$authResponse = [Convert]::ToBase64String($sha256.ComputeHash($secretChallengeBytes))

$id = @{
    op = 1
    d = @{
        rpcVersion = 1
        authentication = $authResponse
    }
} | ConvertTo-Json -Compress

$idb = [System.Text.Encoding]::UTF8.GetBytes($id)
$ws.SendAsync([System.ArraySegment[byte]]::new($idb), [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait(2000) | Out-Null

$res = $ws.ReceiveAsync($seg, $cts.Token)
$res.Wait(2000) | Out-Null

function SendReq($type, $data) {
    $req = @{
        op = 6
        d = @{
            requestType = $type
            requestId = "req_" + [Guid]::NewGuid().ToString().Substring(0,8)
            requestData = $data
        }
    } | ConvertTo-Json -Compress
    
    $reqb = [System.Text.Encoding]::UTF8.GetBytes($req)
    $ws.SendAsync([System.ArraySegment[byte]]::new($reqb), [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait(2000) | Out-Null

    $res = $ws.ReceiveAsync($seg, $cts.Token)
    $res.Wait(2000) | Out-Null
    return [System.Text.Encoding]::UTF8.GetString($buf, 0, $res.Result.Count) | ConvertFrom-Json
}

$scenesResp = SendReq "GetSceneList" @{}
$currentScene = $scenesResp.d.responseData.currentProgramSceneName

$itemsResp = SendReq "GetSceneItemList" @{ sceneName = $currentScene }
$monitorSource = ($itemsResp.d.responseData.sceneItems | Where-Object { $_.inputKind -like "*monitor*" -or $_.inputKind -like "*display*" -or $_.inputKind -like "*screen*" }).sourceName

if (!$monitorSource) {
    Write-Host "Monitor capture source not found!"
    exit
}

Write-Host "Found monitor source: '$monitorSource'"

Write-Host "Removing existing filter on '$monitorSource'..."
$remResp = SendReq "RemoveSourceFilter" @{
    sourceName = $monitorSource
    filterName = "BlewRed_GPU_Censor"
}
Write-Host "Remove Response: $(ConvertTo-Json -Compress $remResp.d.requestStatus)"

Write-Host "`nCreating fresh filter 'BlewRed_GPU_Censor' on '$monitorSource'..."
$createResp = SendReq "CreateSourceFilter" @{
    sourceName = $monitorSource
    filterName = "BlewRed_GPU_Censor"
    filterKind = "blewred_filter"
    filterSettings = @{
        show_debug = $true
        censor_mode = 0
    }
}
Write-Host "Create Response: $(ConvertTo-Json -Compress $createResp.d.requestStatus)"

$ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, '', $cts.Token).Wait(1000) | Out-Null
