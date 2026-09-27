<#
Installer smoke for a *debug* Windows build on a real (CI) Windows session.

Drives the branded NSIS setup (src-tauri/windows/installer.nsi) through its
pages exactly like a user would — welcome → options → install → finish — and
screenshots every page with the real Win32 theming (dark title bar, DarkMode
controls, Segoe UI), which Wine on Linux cannot reproduce. Then it verifies what
the installer left behind (binary, uninstaller, Programs and Features entry,
shortcuts, termoso:// handler), uninstalls through the GUI, repeats the cycle
silently (/S) and passively (/P — what the in-app updater runs), and finally
checks that the WiX .msi still installs/uninstalls and that the setup detects
an MSI install (the migration page). Artifacts land in <Out>.

usage: smoke/windows-installer.ps1 -Setup <Termoso_x_x64-setup.exe> [-Msi <Termoso_x_x64_en-US.msi>] -Out <dir>
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Setup,
  [string]$Msi,
  [Parameter(Mandatory)][string]$Out
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Setup = (Resolve-Path $Setup).Path
if ($Msi) { $Msi = (Resolve-Path $Msi).Path }
$report = Join-Path $Out 'report.txt'
Set-Content -Path $report -Value ''

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class Win {
  public struct RECT { public int Left, Top, Right, Bottom; }
  delegate bool EnumProc(IntPtr h, IntPtr l);

  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);

  public static string Text(IntPtr h) { var sb = new StringBuilder(1024); GetWindowText(h, sb, 1024); return sb.ToString(); }
  public static string Class(IntPtr h) { var sb = new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static uint Pid(IntPtr h) { uint pid; GetWindowThreadProcessId(h, out pid); return pid; }

  public static IntPtr FindTop(string titleContains, uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      if (pid != 0 && Pid(h) != pid) return true;
      if (Text(h).IndexOf(titleContains, StringComparison.OrdinalIgnoreCase) < 0) return true;
      found = h; return false;
    }, IntPtr.Zero);
    return found;
  }

  public static IntPtr FindChild(IntPtr parent, string cls, string textContains) {
    IntPtr found = IntPtr.Zero;
    EnumChildWindows(parent, (h, l) => {
      if (cls != null && Class(h) != cls) return true;
      if (Text(h).Replace("&", "").IndexOf(textContains, StringComparison.OrdinalIgnoreCase) < 0) return true;
      found = h; return false;
    }, IntPtr.Zero);
    return found;
  }

  public static List<string> Children(IntPtr parent) {
    var r = new List<string>();
    EnumChildWindows(parent, (h, l) => { r.Add(Class(h) + " | " + Text(h)); return true; }, IntPtr.Zero);
    return r;
  }

  // Visible frame (no invisible resize borders), falling back to the plain window rect.
  public static RECT Bounds(IntPtr h) {
    RECT r;
    if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) != 0) GetWindowRect(h, out r);
    return r;
  }
}
'@

$WM_COMMAND = 0x0111
$BM_GETCHECK = 0x00F0
$BM_SETCHECK = 0x00F1
$IDOK = 1

$script:failures = 0
function Log([string]$m) {
  $line = '[{0:HH:mm:ss}] {1}' -f (Get-Date), $m
  Add-Content -Path $report -Value $line
  Write-Host $line
}
function Fail([string]$m) { $script:failures++; Log "FAIL: $m"; Write-Host "::error::$m" }
function Check([bool]$ok, [string]$m) { if ($ok) { Log "ok: $m" } else { Fail $m } }

function Wait-Top([string]$title, [int]$procId = 0, [int]$seconds = 60) {
  $deadline = (Get-Date).AddSeconds($seconds)
  do {
    $h = [Win]::FindTop($title, [uint32]$procId)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 250
  } while ((Get-Date) -lt $deadline)
  return [IntPtr]::Zero
}
function Wait-Child([IntPtr]$h, [string]$text, [int]$seconds = 60) {
  $deadline = (Get-Date).AddSeconds($seconds)
  do {
    if (-not [Win]::IsWindow($h)) { return [IntPtr]::Zero }
    $c = [Win]::FindChild($h, $null, $text)
    if ($c -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 700; return $c }
    Start-Sleep -Milliseconds 250
  } while ((Get-Date) -lt $deadline)
  return [IntPtr]::Zero
}
function Wait-Gone([IntPtr]$h, [int]$seconds = 120) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $deadline) {
    if (-not [Win]::IsWindow($h)) { return $true }
    Start-Sleep -Milliseconds 250
  }
  return $false
}
function Shot([IntPtr]$h, [string]$name) {
  try {
    if (-not [Win]::IsWindow($h)) { throw 'window is gone' }
    [void][Win]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 400
    $r = [Win]::Bounds($h)
    $w = $r.Right - $r.Left; $ht = $r.Bottom - $r.Top
    if ($w -le 0 -or $ht -le 0) { throw "empty rect $($r.Left),$($r.Top),$($r.Right),$($r.Bottom)" }
    $bmp = New-Object System.Drawing.Bitmap $w, $ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $Out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Log "shot $name (${w}x${ht})"
  } catch {
    Log "shot $name failed: $_"
    Write-Host "::warning::screenshot $name failed: $_"
  }
}
function Dump([IntPtr]$h, [string]$why) {
  Log "controls ($why):"
  foreach ($c in [Win]::Children($h)) { Log "  $c" }
}
# The wizard's Next/Install/Finish button is IDOK of the outer dialog.
function Press-Next([IntPtr]$h) {
  [void][Win]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 300
  [void][Win]::PostMessage($h, $WM_COMMAND, [IntPtr]$IDOK, [IntPtr]::Zero)
}

$instDir = Join-Path $env:LOCALAPPDATA 'Termoso'
$uninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Termoso'
$startMenuLnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Termoso.lnk'
$desktopLnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Termoso.lnk'

function Assert-Installed([string]$phase, [bool]$shortcuts = $true) {
  Check (Test-Path $uninstKey) "$phase: Programs and Features entry"
  if (Test-Path $uninstKey) {
    $reg = Get-ItemProperty $uninstKey
    Log "$phase: DisplayName='$($reg.DisplayName)' DisplayVersion='$($reg.DisplayVersion)' Publisher='$($reg.Publisher)' MainBinaryName='$($reg.MainBinaryName)'"
    Check ($reg.DisplayName -eq 'Termoso') "$phase: DisplayName"
    Check (-not [string]::IsNullOrEmpty($reg.MainBinaryName) -and (Test-Path (Join-Path $instDir $reg.MainBinaryName))) "$phase: main binary $instDir\$($reg.MainBinaryName)"
    Check ($reg.UninstallString -like "*$instDir\uninstall.exe*") "$phase: UninstallString points into $instDir"
  }
  Check (Test-Path (Join-Path $instDir 'uninstall.exe')) "$phase: uninstall.exe"
  Check (Test-Path 'HKCU:\Software\Classes\termoso\shell\open\command') "$phase: termoso:// handler"
  Check ((Test-Path $startMenuLnk) -eq $shortcuts) "$phase: Start Menu shortcut present=$shortcuts"
  Check ((Test-Path $desktopLnk) -eq $shortcuts) "$phase: Desktop shortcut present=$shortcuts"
}
function Wait-Removed([string]$phase, [int]$seconds = 90) {
  $deadline = (Get-Date).AddSeconds($seconds)
  do {
    $gone = -not (Test-Path $uninstKey) -and -not (Test-Path (Join-Path $instDir 'uninstall.exe'))
    if ($gone) { break }
    Start-Sleep -Milliseconds 500
  } while ((Get-Date) -lt $deadline)
  Check (-not (Test-Path $uninstKey)) "$phase: Programs and Features entry removed"
  Check (-not (Test-Path (Join-Path $instDir 'uninstall.exe'))) "$phase: uninstall.exe removed"
  Check (-not (Test-Path 'HKCU:\Software\Classes\termoso')) "$phase: termoso:// handler removed"
  Check (-not (Test-Path $startMenuLnk)) "$phase: Start Menu shortcut removed"
  Check (-not (Test-Path $desktopLnk)) "$phase: Desktop shortcut removed"
}

Log "setup: $Setup"
Log "msi:   $Msi"
Log "os:    $([Environment]::OSVersion.VersionString), screen $([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width)x$([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)"
if (Test-Path $uninstKey) { Fail 'precondition: Termoso already registered on this machine' }

# ---------------------------------------------------------------- 1. GUI install
Log '== GUI install'
$p = Start-Process -FilePath $Setup -PassThru
$h = Wait-Top 'Termoso Setup' $p.Id 60
Check ($h -ne [IntPtr]::Zero) 'setup window appeared'
if ($h -ne [IntPtr]::Zero) {
  $c = Wait-Child $h 'Welcome to Termoso' 30
  Check ($c -ne [IntPtr]::Zero) 'welcome page'
  if ($c -eq [IntPtr]::Zero) { Dump $h 'no welcome' }
  Shot $h '01-welcome'

  Press-Next $h
  $c = Wait-Child $h 'License Agreement' 30
  Check ($c -ne [IntPtr]::Zero) 'license page'
  if ($c -eq [IntPtr]::Zero) { Dump $h 'no license' }
  Shot $h '01b-license'

  Press-Next $h   # I Agree
  $c = Wait-Child $h 'Install location' 30
  Check ($c -ne [IntPtr]::Zero) 'options page'
  if ($c -eq [IntPtr]::Zero) { Dump $h 'no options' }
  Shot $h '02-options'
  $dir = [Win]::FindChild($h, 'Edit', '')
  if ($dir -ne [IntPtr]::Zero) { Log "install location field: '$([Win]::Text($dir))'" }

  Press-Next $h
  # Best effort: the copy is quick, grab whatever frame of the progress page we can.
  for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 150
    if ([Win]::FindChild($h, $null, 'Termoso is ready') -ne [IntPtr]::Zero) { break }
    if ([Win]::FindChild($h, 'msctls_progress32', '') -ne [IntPtr]::Zero) { Shot $h '03-installing'; break }
  }
  $c = Wait-Child $h 'Termoso is ready' 180
  Check ($c -ne [IntPtr]::Zero) 'finish page'
  if ($c -eq [IntPtr]::Zero) { Dump $h 'no finish' }
  Shot $h '04-finish'

  $launch = [Win]::FindChild($h, 'Button', 'Launch Termoso')
  Check ($launch -ne [IntPtr]::Zero) 'finish page has the Launch checkbox'
  if ($launch -ne [IntPtr]::Zero) {
    Check (([int][Win]::SendMessage($launch, $BM_GETCHECK, [IntPtr]::Zero, [IntPtr]::Zero)) -eq 1) 'Launch checkbox is on by default'
    [void][Win]::SendMessage($launch, $BM_SETCHECK, [IntPtr]::Zero, [IntPtr]::Zero)  # don't start the app on the runner
  }
  Press-Next $h
  Check (Wait-Gone $h 30) 'setup closed after Finish'
}
$p.WaitForExit()
Check ($p.ExitCode -eq 0) "setup exit code $($p.ExitCode)"
Assert-Installed 'gui install'

# ---------------------------------------------------------------- 2. GUI uninstall
Log '== GUI uninstall'
$un = Join-Path $instDir 'uninstall.exe'
if (Test-Path $un) {
  $p = Start-Process -FilePath $un -PassThru
  $h = Wait-Top 'Termoso Uninstall' 0 60   # uninstall.exe re-execs itself from %TEMP%, so don't pin the pid
  Check ($h -ne [IntPtr]::Zero) 'uninstall window appeared'
  if ($h -ne [IntPtr]::Zero) {
    $c = Wait-Child $h 'Delete the application data' 30
    Check ($c -ne [IntPtr]::Zero) 'uninstall confirm page (with delete-app-data checkbox)'
    if ($c -eq [IntPtr]::Zero) { Dump $h 'no confirm' }
    Shot $h '05-uninstall-confirm'
    Press-Next $h
    $c = Wait-Child $h 'Completed' 120
    Check ($c -ne [IntPtr]::Zero) 'uninstall completed page'
    Shot $h '06-uninstall-done'
    Press-Next $h
    Check (Wait-Gone $h 30) 'uninstaller closed'
  }
}
Wait-Removed 'gui uninstall'

# ---------------------------------------------------------------- 3. silent install / uninstall
Log '== silent (/S)'
$p = Start-Process -FilePath $Setup -ArgumentList '/S' -PassThru -Wait
Check ($p.ExitCode -eq 0) "silent setup exit code $($p.ExitCode)"
Assert-Installed 'silent install'
$p = Start-Process -FilePath $un -ArgumentList '/S' -PassThru -Wait
Check ($p.ExitCode -eq 0) "silent uninstall exit code $($p.ExitCode)"
Wait-Removed 'silent uninstall'

# ---------------------------------------------------------------- 4. passive install (updater path)
Log '== passive (/P, what the updater runs)'
$p = Start-Process -FilePath $Setup -ArgumentList '/P' -PassThru
$h = Wait-Top 'Termoso Setup' $p.Id 60
if ($h -ne [IntPtr]::Zero) { Shot $h '07-passive' } else { Log 'passive: window not caught (install finished too fast)' }
$p.WaitForExit()
Check ($p.ExitCode -eq 0) "passive setup exit code $($p.ExitCode)"
Assert-Installed 'passive install'
$p = Start-Process -FilePath $un -ArgumentList '/S' -PassThru -Wait
Wait-Removed 'passive cleanup'

# ---------------------------------------------------------------- 5. MSI (enterprise deployment) + migration page
if ($Msi) {
  Log '== MSI (msiexec /qn) + setup detecting the MSI install'
  $log = Join-Path $Out 'msi-install.log'
  $p = Start-Process -FilePath msiexec.exe -ArgumentList @('/i', "`"$Msi`"", '/qn', '/norestart', '/l*v', "`"$log`"") -PassThru -Wait
  Check ($p.ExitCode -eq 0) "msiexec /i exit code $($p.ExitCode)"
  $msiKey = Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    Where-Object { ($_ | Get-ItemProperty).DisplayName -eq 'Termoso' -and ($_ | Get-ItemProperty).UninstallString -match 'msiexec' } |
    Select-Object -First 1
  Check ($null -ne $msiKey) 'MSI registered in Programs and Features (HKLM, msiexec UninstallString)'
  if ($msiKey) { Log "msi: $($msiKey.PSChildName) InstallLocation='$(($msiKey | Get-ItemProperty).InstallLocation)'" }

  $p = Start-Process -FilePath $Setup -PassThru
  $h = Wait-Top 'Termoso Setup' $p.Id 60
  Check ($h -ne [IntPtr]::Zero) 'setup window appeared over the MSI install'
  if ($h -ne [IntPtr]::Zero) {
    Start-Sleep -Milliseconds 800
    Press-Next $h   # welcome → license
    $c = Wait-Child $h 'License Agreement' 30
    if ($c -ne [IntPtr]::Zero) { Press-Next $h }   # I Agree → reinstall/migration page
    $c = Wait-Child $h 'Uninstall Termoso' 30
    Check ($c -ne [IntPtr]::Zero) 'migration page offers to uninstall the MSI install'
    if ($c -eq [IntPtr]::Zero) { Dump $h 'no migration page' }
    Shot $h '08-msi-migration'
  }
  if (-not $p.HasExited) { $p.Kill() }   # the actual MSI removal below is silent; don't sit in msiexec's confirm dialog

  $log = Join-Path $Out 'msi-uninstall.log'
  $p = Start-Process -FilePath msiexec.exe -ArgumentList @('/x', "`"$Msi`"", '/qn', '/norestart', '/l*v', "`"$log`"") -PassThru -Wait
  Check ($p.ExitCode -eq 0) "msiexec /x exit code $($p.ExitCode)"
  $left = Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    Where-Object { ($_ | Get-ItemProperty).DisplayName -eq 'Termoso' }
  Check ($null -eq $left) 'MSI entry removed'
}

Log "== done, $($script:failures) failure(s)"
Get-ChildItem $Out -Filter *.png | ForEach-Object { Log "artifact: $($_.Name)" }
if ($script:failures -gt 0) { exit 1 }
