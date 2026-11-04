# 生成自签名代码签名证书（PRD 6.7 / F8-2）
#
# 用法（在仓库根目录执行）：
#   powershell -ExecutionPolicy Bypass -File scripts\gen-signing-cert.ps1
#
# 产物：
#   证书写入当前用户个人存储 Cert:\CurrentUser\My，有效期 5 年
#   屏幕上打印指纹（Thumbprint），复制后设为环境变量：
#     setx QINGQIUHU_CERT_THUMBPRINT "<指纹>"
#
# 分发公钥证书给 IT 推进域控（否则没推根证书的机器安装时仍会弹警告）：
#   powershell -ExecutionPolicy Bypass -File scripts\gen-signing-cert.ps1 -ExportCer
#
# 删除证书：
#   Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "*Qingqiuhu Silicon Forge*" } | Remove-Item

param(
  [string]$Subject = "CN=Qingqiuhu Silicon Forge, O=Qingqiuhu, C=CN",
  [int]$Years = 5,
  [switch]$ExportCer,
  [string]$CerPath = ""
)

$ErrorActionPreference = 'Stop'

$existing = Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.Subject -eq $Subject -and $_.NotAfter -gt (Get-Date) } |
  Sort-Object NotAfter -Descending |
  Select-Object -First 1

if ($existing) {
  Write-Host "已存在有效期内的同名证书，直接复用：" -ForegroundColor Yellow
  $cert = $existing
} else {
  $cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -KeyUsage DigitalSignature `
    -KeyAlgorithm RSA `
    -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -KeyExportPolicy Exportable `
    -CertStoreLocation Cert:\CurrentUser\My `
    -NotAfter (Get-Date).AddYears($Years)

  Write-Host "证书已创建" -ForegroundColor Green
}

Write-Host ""
Write-Host "Subject   : $($cert.Subject)"
Write-Host "Thumbprint: $($cert.Thumbprint)"
Write-Host "有效期至  : $($cert.NotAfter.ToString('yyyy-MM-dd'))"
Write-Host "存储位置  : Cert:\CurrentUser\My"
Write-Host ""
Write-Host "下一步：把指纹设为环境变量，构建时才会签名" -ForegroundColor Cyan
Write-Host "  setx QINGQIUHU_CERT_THUMBPRINT $($cert.Thumbprint)"
Write-Host ""
Write-Host "注意：仅设置了环境变量的机器会签，未设置时构建会明确提示「未签名」。" -ForegroundColor Yellow
Write-Host "注意：私钥不可导出到共享位置；换机器需重新生成或导入同一份 .pfx。" -ForegroundColor Yellow

if ($ExportCer) {
  if ([string]::IsNullOrWhiteSpace($CerPath)) {
    $CerPath = Join-Path (Get-Location) 'QingqiuhuSiliconForge-CodeSigning.cer'
  }
  Export-Certificate -Cert $cert -FilePath $CerPath -Type CERT | Out-Null
  Write-Host ""
  Write-Host "公钥证书已导出（可交给 IT 走 GPO 推送到「受信任的根证书颁发机构」）：" -ForegroundColor Green
  Write-Host "  $CerPath"
}
