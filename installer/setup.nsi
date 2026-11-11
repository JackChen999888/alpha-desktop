; Alpha per-user installer; no administrator access, no embedded account data.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!ifndef APP_ID
  !define APP_ID "AlphaDesktop"
!endif
!ifndef APP_NAME
  !define APP_NAME "Alpha"
!endif
!ifndef APP_MUTEX
  !define APP_MUTEX "Local\DesktopIslandPro.SingleInstance"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
Name "${APP_NAME}"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Alpha"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
RequestExecutionLevel user
ManifestDPIAware true
SetCompressor /SOLID lzma
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=2052 "ProductName" "Alpha Desktop"
VIAddVersionKey /LANG=2052 "FileDescription" "Alpha Desktop 安装程序"
VIAddVersionKey /LANG=2052 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=2052 "LegalCopyright" "Alpha and Daozai contributors"
!define MUI_ICON "${PACKAGE_DIR}\Alpha.ico"
!define MUI_UNICON "${PACKAGE_DIR}\Alpha.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "安装 Alpha 桌面信息岛。$\r$\n$\r$\n无需 Rust、Git、Visual Studio 或管理员权限。$\r$\n升级保留设置、笔记和账号数据。$\r$\n$\r$\n请先从托盘退出正在运行的 Alpha。"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PACKAGE_DIR}\LICENSE"
!define MUI_DIRECTORYPAGE_VERIFYONLEAVE
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE ValidateDirectory
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\Alpha.exe"
!define MUI_FINISHPAGE_RUN_PARAMETERS "--pro"
!define MUI_FINISHPAGE_RUN_TEXT "打开 Alpha"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "卸载程序和快捷方式。用户设置、笔记、账号凭据和其他个人数据将保留。若曾配置 AI Hooks，请先在对应客户端移除 Alpha 的 Hook。"
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
BrandingText "Alpha Desktop · MIT"

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "需要 Windows 10/11 x64。" /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_OK|MB_ICONSTOP "需要 Windows 10/11。" /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
FunctionEnd

Function ValidateDirectory
  ; Reject drive/share roots and relative paths before any writes.
  ${GetRoot} "$INSTDIR" $0
  ${If} $0 == ""
    Goto invalid
  ${EndIf}
  StrLen $1 $0
  StrCpy $2 "$INSTDIR" 1 $1
  ${If} $2 != "\"
    Goto invalid
  ${EndIf}
  System::Call 'kernel32::GetFullPathNameW(w "$INSTDIR", i ${NSIS_MAX_STRLEN}, w .r1, p 0) i .r0'
  ${If} $0 == 0
  ${OrIf} $0 >= ${NSIS_MAX_STRLEN}
    Goto invalid
  ${EndIf}
  System::Call 'shlwapi::PathIsRootW(w "$1") i .r0'
  ${If} $0 != 0
    Goto invalid
  ${EndIf}
  StrCpy $INSTDIR $1
  ClearErrors
  CreateDirectory "$INSTDIR"
  IfErrors invalid
  GetTempFileName $0 "$INSTDIR"
  IfErrors invalid
  Delete "$0"
  Return
  invalid:
    MessageBox MB_OK|MB_ICONSTOP "请选择可写入的独立应用文件夹，不可使用磁盘根目录。" /SD IDOK
    SetErrorLevel 1
    Abort
FunctionEnd

!macro ClosedCheck Prefix
Function ${Prefix}CheckClosed
  retry:
  System::Call 'kernel32::OpenMutexW(i 0x100000, i 0, w "${APP_MUTEX}") p .r0'
  ${If} $0 != 0
    System::Call 'kernel32::CloseHandle(p r0)'
    MessageBox MB_RETRYCANCEL|MB_ICONINFORMATION "请先从托盘退出 Alpha，然后重试。" /SD IDCANCEL IDRETRY retry
    SetErrorLevel 2
    Abort
  ${EndIf}
FunctionEnd
!macroend
!insertmacro ClosedCheck ""
!insertmacro ClosedCheck "un."

Section "安装"
  Call CheckClosed
  Call ValidateDirectory
  SetOutPath "$INSTDIR"
  SetOverwrite on
  ClearErrors
  File "${PACKAGE_DIR}\Alpha.exe"
  File "${PACKAGE_DIR}\desktop-island-event.exe"
  File "${PACKAGE_DIR}\Alpha.ico"
  File "${PACKAGE_DIR}\LICENSE"
  File "${PACKAGE_DIR}\LICENSE-Fly-Island"
  File "${PACKAGE_DIR}\THIRD_PARTY_NOTICES.txt"
  File "${PACKAGE_DIR}\README.txt"
  IfErrors failed
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  IfErrors failed
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\Alpha.exe" "--pro" "$INSTDIR\Alpha.ico"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\Alpha.exe" "--pro" "$INSTDIR\Alpha.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME} Desktop"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "chennengjian / Alpha contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\Alpha.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/chennengjian/alpha-desktop"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ; Only count package-owned files, not unrelated files or user data.
  ${GetSize} "$INSTDIR" "/M=Alpha.exe /S=0K /G=0" $0 $1 $2
  ${GetSize} "$INSTDIR" "/M=desktop-island-event.exe /S=0K /G=0" $3 $1 $2
  IntOp $0 $0 + $3
  ${GetSize} "$INSTDIR" "/M=THIRD_PARTY_NOTICES.txt /S=0K /G=0" $3 $1 $2
  IntOp $0 $0 + $3
  IntOp $0 $0 + 256
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" $0
  Goto done
  failed:
    MessageBox MB_OK|MB_ICONSTOP "文件写入失败，请退出 Alpha 后重新运行安装程序。" /SD IDOK
    SetErrorLevel 3
    Abort
  done:
SectionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 64
FunctionEnd

Section "Uninstall"
  Call un.CheckClosed
  ; Do not recursively remove the install directory or any data directory.
  ClearErrors
  Delete "$INSTDIR\Alpha.exe"
  Delete "$INSTDIR\desktop-island-event.exe"
  IfErrors failed
  Delete "$INSTDIR\Alpha.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\LICENSE-Fly-Island"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\README.txt"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DesktopIslandPro"
  ${If} $0 == '$\"$INSTDIR\Alpha.exe$\"'
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DesktopIslandPro"
  ${EndIf}
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Goto done
  failed:
    MessageBox MB_OK|MB_ICONSTOP "程序文件仍在使用，请退出 Alpha 后重新卸载。" /SD IDOK
    SetErrorLevel 3
    Abort
  done:
SectionEnd
