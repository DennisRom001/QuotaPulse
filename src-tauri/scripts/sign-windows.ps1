<#
.SYNOPSIS
  Authenticode-signs a file produced by `tauri build` (QuotaPulse.exe, the
  NSIS/MSI installers and the NSIS uninstaller). Tauri calls this once per
  file through `bundle.windows.signCommand`, passing the path as the only
  argument.

.DESCRIPTION
  Certificate selection, in order:
    1. QP_SIGN_PFX (+ QP_SIGN_PFX_PASSWORD): a real code-signing certificate
       (.pfx) from a CA in Microsoft's trusted root program. This is the only
       option that satisfies Windows Smart App Control on other machines.
    2. QP_SIGN_THUMBPRINT: a code-signing certificate already installed in
       Cert:\CurrentUser\My.
    3. Otherwise a local self-signed certificate "CN=QuotaPulse Local Code
       Signing" is created once in Cert:\CurrentUser\My and reused.

  No timestamp server is contacted, so signing never sends anything off the
  machine. The script never touches the machine-wide trust stores.

  NOTE: a self-signed signature proves the file was not modified after the
  build, but Smart App Control does not treat a self-signed publisher as
  trusted; see BUILDING.md.
#>
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Path
)

$ErrorActionPreference = 'Stop'
$subject = 'CN=QuotaPulse Local Code Signing'

function Get-SigningCertificate {
  if ($env:QP_SIGN_PFX) {
    $password = if ($env:QP_SIGN_PFX_PASSWORD) {
      ConvertTo-SecureString $env:QP_SIGN_PFX_PASSWORD -AsPlainText -Force
    } else { $null }
    return Get-PfxCertificate -FilePath $env:QP_SIGN_PFX -Password $password
  }

  if ($env:QP_SIGN_THUMBPRINT) {
    $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
      Where-Object { $_.Thumbprint -eq $env:QP_SIGN_THUMBPRINT } |
      Select-Object -First 1
    if (-not $cert) { throw "Certificate $($env:QP_SIGN_THUMBPRINT) not found in Cert:\CurrentUser\My" }
    return $cert
  }

  $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date).AddDays(7) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1
  if ($cert) { return $cert }

  Write-Host "[sign] creating self-signed certificate '$subject' in Cert:\CurrentUser\My"
  return New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $subject `
    -CertStoreLocation Cert:\CurrentUser\My `
    -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -KeyExportPolicy NonExportable `
    -NotAfter (Get-Date).AddYears(3)
}

if (-not (Test-Path -LiteralPath $Path)) { throw "File to sign not found: $Path" }

$cert = Get-SigningCertificate

# A freshly written binary can be locked for a moment by antivirus scanners,
# so allow a few short attempts before giving up.
$result = $null
for ($attempt = 1; $attempt -le 5; $attempt++) {
  try {
    $result = Set-AuthenticodeSignature -LiteralPath $Path -Certificate $cert -HashAlgorithm SHA256
    if ((Get-AuthenticodeSignature -LiteralPath $Path).SignerCertificate) { break }
  } catch {
    if ($attempt -eq 5) { throw }
  }
  Start-Sleep -Milliseconds 800
}

# With a self-signed certificate the status is 'UnknownError' (chain not
# trusted on this machine) even though the signature was written; only fail
# when no signature ended up on the file.
$check = Get-AuthenticodeSignature -LiteralPath $Path
if (-not $check.SignerCertificate) {
  throw "Signing failed for ${Path}: $($result.StatusMessage)"
}
Write-Host ("[sign] {0} -> signed by {1} ({2})" -f (Split-Path $Path -Leaf), $check.SignerCertificate.Subject, $check.Status)
