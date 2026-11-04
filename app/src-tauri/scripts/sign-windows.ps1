# 构建期签名脚本（由 tauri.conf.json 的 bundle.windows.signCommand 调用）
#
# 设计：签名是「按机器开关」而不是硬编码。证书指纹从环境变量读取，
# 这样同一个 tauri.conf.json 在开发机（不签名）和发布机（签名）都能用。
#
# 用法：
#   setx QINGQIUHU_CERT_THUMBPRINT "<证书指纹>"     # 设置后 git bash 需重开
#
# 参数：
#   -Path <可执行文件或安装包路径>
#   -Required                      缺少证书时直接失败（CI 发布用），默认只警告

param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Path,
  [switch]$Required,
  [string]$TimestampUrl = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'

$thumbprint = $env:QINGQIUHU_CERT_THUMBPRINT

if ([string]::IsNullOrWhiteSpace($thumbprint)) {
  if ($Required) {
    Write-Error "未设置 QINGQIUHU_CERT_THUMBPRINT，无法签名。发布构建必须签名。"
    exit 1
  }
  Write-Warning "未设置 QINGQIUHU_CERT_THUMBPRINT，跳过签名（产物未签名，仅供本机测试）"
  exit 0
}

# signtool 不在 PATH 里，从 Windows Kits 找最新版本
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending |
  Select-Object -First 1

if (-not $signtool) {
  Write-Error "未找到 signtool.exe，请安装 Windows SDK（含签名工具）。"
  exit 1
}

if (-not (Test-Path $Path)) {
  Write-Error "文件不存在：$Path"
  exit 1
}

# 打包器会把签名命令套用到它自己的工具链文件上（WiX 的 DLL、NSIS 插件等）。
# 重签别人的组件既没必要，也可能让它们校验失败——只签我们自己的产物。
if ($Path -match '\\wix\\|\\WixUI|\\WixUtil|\\wixca\.dll') {
  Write-Host "跳过第三方工具链文件：$Path"
  exit 0
}

# 用 /sha1 按指纹从证书存储取证书，私钥不出存储、不进命令行
& $signtool.FullName sign /sha1 $thumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $Path

if ($LASTEXITCODE -ne 0) {
  # 内网通常到不了公共时间戳服务器。不带时间戳的签名在证书有效期内同样有效，
  # 但证书过期后已签文件会一并失效——所以换证书的时间点要提前规划（PRD 7.9 / R-21）。
  Write-Warning "带时间戳签名失败（$TimestampUrl 不可达），改为不带时间戳重签"
  & $signtool.FullName sign /sha1 $thumbprint /fd SHA256 $Path
}

if ($LASTEXITCODE -ne 0) {
  Write-Error "签名失败，退出码 $LASTEXITCODE"
  exit $LASTEXITCODE
}

# signtool 把失败信息写 stderr，在 ErrorActionPreference=Stop 下会被当成异常抛出，
# 这里临时放开，自己判退出码。
$prevEap = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
$verifyOut = (& $signtool.FullName verify /pa $Path 2>&1) -join "`n"
$verifyCode = $LASTEXITCODE
$ErrorActionPreference = $prevEap

if ($verifyCode -ne 0) {
  # 自签证书在「没导入受信任根」的机器上校验必然失败，这是 PRD R-05 描述的既有情况，
  # 不代表签名本身有问题。真正的签名损坏（哈希不符/无签名）要单独区分出来。
  if ($verifyOut -match 'not trusted|root certificate|根证书|不受信任') {
    Write-Warning "签名已写入，但本机未信任该自签根证书（属预期）。"
    Write-Warning "目标机器需由 IT 通过 GPO 把公钥证书推到「受信任的根证书颁发机构」，否则安装时仍会提示未知发布者。"
  } else {
    Write-Error "签名校验失败：$verifyOut"
    exit 1
  }
}

Write-Host "已签名：$Path" -ForegroundColor Green
