# エラーは止める
$ErrorActionPreference = 'Stop'

# 文字コードはUTF-8固定、コンソールも合わせる
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

# 改行・Gitまわり
git config --global core.autocrlf false
git config --global core.longpaths true   # 260文字問題
$env:DOTNET_CLI_UI_LANGUAGE = "en"        # CLI出力を英語に固定（解析しやすく）

# 長いパスをOS側でも許容（管理者 Powershell で一度だけ実施）
# Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name LongPathsEnabled -Value 1

# 実行ポリシー（ユーザー範囲）
# Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned -Force
