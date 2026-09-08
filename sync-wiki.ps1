$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$api = 'https://ecliptica.miraheze.org/w/api.php'
$raw = 'https://ecliptica.miraheze.org/w/index.php'
$root = Join-Path $PSScriptRoot 'wiki'
$imgDir = Join-Path $root 'images'
New-Item -ItemType Directory -Force $root, $imgDir | Out-Null

function Get-SafeName($title) {
    ($title -replace '[\\/:*?"<>|]', '_') -replace ' ', '_'
}

$pageCount = 0
foreach ($ns in 0, 6, 14) {
    $cont = ''
    do {
        $url = "$api" + "?action=query&list=allpages&aplimit=500&format=json&apnamespace=$ns"
        if ($cont) { $url += '&apcontinue=' + [uri]::EscapeDataString($cont) }
        $r = Invoke-RestMethod $url -UseBasicParsing
        foreach ($p in $r.query.allpages) {
            $enc = [uri]::EscapeDataString($p.title)
            $dest = Join-Path $root ((Get-SafeName $p.title) + '.wiki')
            Invoke-WebRequest ("$raw" + "?title=$enc&action=raw") -OutFile $dest -UseBasicParsing
            $pageCount++
        }
        $cont = $r.continue.apcontinue
    } while ($cont)
}

$imgCount = 0
$imgNew = 0
$cont = ''
do {
    $url = "$api" + '?action=query&list=allimages&ailimit=500&aiprop=url%7Csize&format=json'
    if ($cont) { $url += '&aicontinue=' + [uri]::EscapeDataString($cont) }
    $r = Invoke-RestMethod $url -UseBasicParsing
    foreach ($img in $r.query.allimages) {
        $dest = Join-Path $imgDir (Get-SafeName $img.name)
        $current = (Test-Path $dest) -and ((Get-Item $dest).Length -eq $img.size)
        if (-not $current) {
            Invoke-WebRequest $img.url -OutFile $dest -UseBasicParsing
            $imgNew++
        }
        $imgCount++
    }
    $cont = $r.continue.aicontinue
} while ($cont)

"$pageCount pages fetched, $imgCount images on wiki, $imgNew downloaded"
