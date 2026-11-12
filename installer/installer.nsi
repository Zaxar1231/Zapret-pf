; Установщик Zapret PF (NSIS 3, Unicode)
;
; Сборка:  makensis -DVERSION=1.0.0 -DAPP_EXE=..\path\ZapretPF.exe -DZAPRET_DIR=..\path\zapret installer.nsi
;
; Ключи командной строки:
;   /S        — тихая установка (используется автообновлением приложения)
;   /UPDATE   — после тихой установки снова запустить приложение
;   /D=путь   — папка установки (последним параметром, без кавычек)

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "1.0.0"
!endif
!ifndef APP_EXE
  !define APP_EXE "..\target\x86_64-pc-windows-gnu\release\ZapretPF.exe"
!endif
!ifndef ZAPRET_DIR
  !define ZAPRET_DIR "..\dist\zapret"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "..\dist\ZapretPF-Setup-${VERSION}.exe"
!endif

!define APP_NAME "Zapret PF"
!define APP_ID "ZapretPF"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

Name "${APP_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$PROGRAMFILES64\${APP_ID}"
InstallDirRegKey HKLM "Software\${APP_ID}" "InstallDir"
RequestExecutionLevel admin
BrandingText "${APP_NAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1049 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1049 "FileDescription" "Установщик ${APP_NAME}"
VIAddVersionKey /LANG=1049 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1049 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1049 "LegalCopyright" "MIT"

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_SMALLDESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\ZapretPF.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Запустить ${APP_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Russian"

; ------------------------------------------------------------------ install

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APP_NAME} работает только в 64-битной Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Section "${APP_NAME}" SecMain
  SectionIn RO
  SetShellVarContext all

  ; закрываем запущенное приложение (при обновлении оно уже закрывается само)
  nsExec::Exec 'taskkill /F /IM ZapretPF.exe'
  Sleep 800

  SetOutPath "$INSTDIR"
  File "/oname=ZapretPF.exe" "${APP_EXE}"

  ; Файлы zapret ставим только при первой установке —
  ; дальше их обновляет само приложение (кнопкой «Обновить»).
  ${IfNot} ${FileExists} "$INSTDIR\zapret\bin\winws.exe"
    SetOutPath "$INSTDIR\zapret"
    File /r "${ZAPRET_DIR}\*.*"
  ${EndIf}
  CreateDirectory "$INSTDIR\data"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; ярлык в меню «Пуск» — только на приложение, профили остаются в папке установки
  CreateShortCut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\ZapretPF.exe" "" "$INSTDIR\ZapretPF.exe" 0

  WriteRegStr HKLM "Software\${APP_ID}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\${APP_ID}" "Version" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Zapret PF"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\ZapretPF.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "https://github.com/zaxar1231/zapret-pf"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Ярлык на рабочем столе" SecDesktop
  SetShellVarContext all
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\ZapretPF.exe" "" "$INSTDIR\ZapretPF.exe" 0
SectionEnd

Section /o "Исключение в Защитнике Windows" SecDefender
  ; Защитник иногда помечает winws.exe / WinDivert как HackTool и удаляет их.
  nsExec::Exec `powershell -NoProfile -ExecutionPolicy Bypass -Command "Add-MpPreference -ExclusionPath '$INSTDIR\zapret'"`
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "Приложение и файлы zapret-discord-youtube (профили, winws.exe, списки)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Ярлык приложения на рабочем столе."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDefender} "Добавить папку zapret в исключения Microsoft Defender, чтобы он не удалял winws.exe."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInstSuccess
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    Exec '"$INSTDIR\ZapretPF.exe"'
  ${EndIf}
FunctionEnd

; ------------------------------------------------------------------ uninstall

Function un.onInit
  SetRegView 64
FunctionEnd

Section "Uninstall"
  SetShellVarContext all

  nsExec::Exec 'taskkill /F /IM ZapretPF.exe'
  nsExec::Exec 'taskkill /F /IM winws.exe'
  nsExec::Exec 'schtasks /Delete /F /TN ZapretPF'
  nsExec::Exec 'net stop zapret'
  nsExec::Exec 'sc delete zapret'
  nsExec::Exec 'net stop WinDivert'
  nsExec::Exec 'sc delete WinDivert'
  nsExec::Exec 'net stop WinDivert14'
  nsExec::Exec 'sc delete WinDivert14'
  Sleep 1000

  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  RMDir /r "$INSTDIR\zapret"
  RMDir /r "$INSTDIR\zapret.old"
  RMDir /r "$INSTDIR\data"
  Delete "$INSTDIR\ZapretPF.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\${APP_ID}"
SectionEnd
