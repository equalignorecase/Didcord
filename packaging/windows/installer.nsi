; Didcord Windows Installer Script
; Installs per-user to $LOCALAPPDATA\Programs\Didcord without elevation
; Preserves write permissions for seamless in-app autoupdates

Unicode True
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "x64.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"

!define PRODUCT_NAME "Didcord"
!define PRODUCT_PUBLISHER "Didcord contributors"
!define PRODUCT_WEB_SITE "https://github.com/equalignorecase/Didcord"
!define APP_EXE "didcord.exe"
!define APP_ID "io.github.equalignorecase.didcord"

!ifndef VERSION
  !define VERSION "0.1.0"
!endif

!ifndef DIST_DIR
  !if /FileExists "dist"
    !define DIST_DIR "dist"
  !else
    !define DIST_DIR "..\..\dist"
  !endif
!endif

!ifndef OUTPUT_DIR
  !if /FileExists "dist"
    !define OUTPUT_DIR "dist-installer"
  !else
    !define OUTPUT_DIR "..\..\dist-installer"
  !endif
!endif

Name "${PRODUCT_NAME} ${VERSION}"
OutFile "${OUTPUT_DIR}\didcord-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Didcord"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "InstallLocation"

!if /FileExists "packaging\windows\Didcord.ico"
  !define MUI_ICON "packaging\windows\Didcord.ico"
  !define MUI_UNICON "packaging\windows\Didcord.ico"
!else if /FileExists "${__FILEDIR__}\Didcord.ico"
  !define MUI_ICON "${__FILEDIR__}\Didcord.ico"
  !define MUI_UNICON "${__FILEDIR__}\Didcord.ico"
!else if /FileExists "Didcord.ico"
  !define MUI_ICON "Didcord.ico"
  !define MUI_UNICON "Didcord.ico"
!else
  !define MUI_ICON "${__FILEDIR__}\Didcord.ico"
  !define MUI_UNICON "${__FILEDIR__}\Didcord.ico"
!endif

!define MUI_ABORTWARNING

Var ShowInstallOptions
Var SetupIcon

; Setup pages use the same window and native keyboard navigation.
Page custom ShowSetup
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipInstallDirectory
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
Page custom ShowSetupComplete

; Uninstaller UI Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

Function ShowSetup
  StrCpy $ShowInstallOptions 0
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    SetErrorLevel 2
    Quit
  ${EndIf}

  !insertmacro MUI_HEADER_TEXT "Install ${PRODUCT_NAME}" "Version ${VERSION}"

  ${NSD_CreateIcon} 8u 16u 32u 32u ""
  Pop $0
  ${NSD_SetIconFromInstaller} $0 $SetupIcon
  ${NSD_CreateLabel} 49u 19u -57u 20u "${PRODUCT_NAME} will be installed for your Windows account."
  Pop $0
  ${NSD_CreateLink} 49u 49u -57u 13u "View license"
  Pop $0
  ${NSD_OnClick} $0 OpenSetupLicense
  ${NSD_CreateButton} 49u 76u 68u 17u "&Options..."
  Pop $0
  ${NSD_OnClick} $0 OpenInstallOptions

  GetDlgItem $0 $HWNDPARENT 1
  ${NSD_SetText} $0 "&Install"
  System::Call 'user32::SetFocus(p r0)'
  nsDialogs::Show
  ${NSD_FreeIcon} $SetupIcon
FunctionEnd

Function OpenSetupLicense
  Pop $0
  InitPluginsDir
  ${IfNot} ${FileExists} "$PLUGINSDIR\Didcord-license.txt"
    File /oname=$PLUGINSDIR\Didcord-license.txt "${DIST_DIR}\LICENSE-MIT"
  ${EndIf}
  ClearErrors
  ExecShell "open" "$PLUGINSDIR\Didcord-license.txt"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONEXCLAMATION "Windows could not open the license."
  ${EndIf}
FunctionEnd

Function OpenInstallOptions
  Pop $0
  StrCpy $ShowInstallOptions 1
  SendMessage $HWNDPARENT ${WM_COMMAND} 1 0
FunctionEnd

Function SkipInstallDirectory
  ${If} $ShowInstallOptions == 0
    Abort
  ${EndIf}
FunctionEnd

Function ShowSetupComplete
  IfAbort failed
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  !insertmacro MUI_HEADER_TEXT "${PRODUCT_NAME} is ready" "Installation complete"
  ${NSD_CreateLabel} 8u 19u -16u 20u "You can also open ${PRODUCT_NAME} from the Start menu."
  Pop $0
  ${NSD_CreateButton} 8u 48u 92u 18u "&Open ${PRODUCT_NAME}"
  Pop $0
  ${NSD_OnClick} $0 LaunchDidcord
  System::Call 'user32::SetFocus(p r0)'
  GetDlgItem $0 $HWNDPARENT 1
  ${NSD_SetText} $0 "&Close"
  nsDialogs::Show
  Return

  failed:
    Abort
FunctionEnd

Function LaunchDidcord
  Pop $0
  SetOutPath "$INSTDIR"
  ClearErrors
  Exec '"$INSTDIR\${APP_EXE}"'
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONEXCLAMATION "Windows could not open ${PRODUCT_NAME}. Try opening it from the Start menu."
  ${Else}
    SendMessage $HWNDPARENT ${WM_COMMAND} 1 0
  ${EndIf}
FunctionEnd

; The installer uses the x86 NSIS stub, including on x64 and ARM64 Windows.
!if ${NSIS_PTR_SIZE} != 4
  !error "The process-entry layout requires the x86 NSIS installer stub."
!endif

; Returns 0 when absent, 1 when running, 2 when the snapshot cannot be checked.
!macro DidcordProcessCheck PREFIX
Function ${PREFIX}CheckDidcordRunning
  System::Store "s"
  StrCpy $R0 2
  System::Call 'kernel32::CreateToolhelp32Snapshot(i 2, i 0) p .r0'
  ${If} $0 != -1
    System::Alloc 556
    Pop $1
    ${If} $1 != 0
      ; PROCESSENTRY32W: dwSize followed by 32 bytes of fields and WCHAR[260].
      System::Call '*$1(i 556)'
      System::Call 'kernel32::Process32FirstW(p r0, p r1) i .r2 ?e'
      Pop $3
      ${DoWhile} $2 != 0
        System::Call '*$1(&v36, &w260 .r4)'
        ${If} $4 == "${APP_EXE}"
          StrCpy $R0 1
          ${Break}
        ${EndIf}
        System::Call 'kernel32::Process32NextW(p r0, p r1) i .r2 ?e'
        Pop $3
      ${Loop}
      ${If} $2 == 0
      ${AndIf} $3 == 18 ; ERROR_NO_MORE_FILES, not an enumeration failure.
        StrCpy $R0 0
      ${EndIf}
      System::Free $1
    ${EndIf}
    System::Call 'kernel32::CloseHandle(p r0)'
  ${EndIf}
  Push $R0
  System::Store "l"
FunctionEnd
!macroend
!insertmacro DidcordProcessCheck ""
!insertmacro DidcordProcessCheck "un."

Function RegisterNotificationShortcut
  System::Store "s"
  StrCpy $R0 2
  StrCpy $1 0
  ; CreateShortcut initializes COM. Update the saved shortcut's native property store.
  System::Call 'shell32::SHGetPropertyStoreFromParsingName(w "$SMPROGRAMS\${PRODUCT_NAME}.lnk", p 0, i 2, g "${IID_IPropertyStore}", *p .r1) i .r0'
  ${If} $0 >= 0
    StrCpy $4 0
    System::Call 'shlwapi::SHStrDupW(w "${APP_ID}", *p .r4) i .r0'
    ${If} $0 >= 0
      System::Call '*${SYSSTRUCT_PROPERTYKEY}(${PKEY_AppUserModel_ID}) p .r2'
      System::Alloc ${SYSSIZEOF_PROPVARIANT}
      Pop $3
      ${If} $2 != 0
      ${AndIf} $3 != 0
        System::Call '*$3(&i2 ${VT_LPWSTR}, &i6 0, p r4)'
        ${IPropertyStore::SetValue} $1 '($2, $3) .r0'
        ${If} $0 >= 0
          ${IPropertyStore::Commit} $1 '() .r0'
          ${If} $0 >= 0
            StrCpy $R0 0
          ${EndIf}
        ${EndIf}
      ${EndIf}
      System::Free $3
      System::Free $2
    ${EndIf}
    System::Call 'ole32::CoTaskMemFree(p r4)'
    ${IUnknown::Release} $1 ''
  ${EndIf}
  Push $R0
  System::Store "l"
FunctionEnd

Function .onInit
  SetShellVarContext current
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ${Do}
    Call CheckDidcordRunning
    Pop $0
    ${If} $0 == 2
      MessageBox MB_OK|MB_ICONSTOP "Cannot check whether ${PRODUCT_NAME} is running. Setup cannot continue safely." /SD IDOK
      SetErrorLevel 2
      Abort
    ${EndIf}
    ${If} $0 != 0
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "${PRODUCT_NAME} is currently running. Please close Didcord before continuing." /SD IDCANCEL IDRETRY retry_init IDCANCEL cancel_init
      retry_init:
        ${Continue}
      cancel_init:
        SetErrorLevel 1
        Abort
    ${Else}
      ${Break}
    ${EndIf}
  ${Loop}
FunctionEnd

Section "MainSection" SEC01
  SetOutPath "$INSTDIR"
  SetOverwrite on

  ; Copy all package payload files
  File /r /x install-notifications.ps1 /x setup.ps1 "${DIST_DIR}\*.*"
  ; Remove legacy installer utilities when upgrading an existing installation.
  Delete "$INSTDIR\install-notifications.ps1"
  Delete "$INSTDIR\setup.ps1"

  ; Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Write Add/Remove Programs uninstall registry keys
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayIcon" "$INSTDIR\${APP_EXE},0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "EstimatedSize" "$0"

  ; Keep uninstall registration available even if shortcut creation fails.
  ClearErrors
  CreateDirectory "$SMPROGRAMS"
  CreateShortcut "$SMPROGRAMS\${PRODUCT_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  StrCpy $0 2
  ${IfNot} ${Errors}
    Call RegisterNotificationShortcut
    Pop $0
  ${EndIf}
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Cannot register the ${PRODUCT_NAME} notification shortcut. You can remove the incomplete installation from Windows Installed Apps." /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
SectionEnd

Function un.onInit
  SetShellVarContext current
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ${Do}
    Call un.CheckDidcordRunning
    Pop $0
    ${If} $0 == 2
      MessageBox MB_OK|MB_ICONSTOP "Cannot check whether ${PRODUCT_NAME} is running. Uninstall cannot continue safely." /SD IDOK
      SetErrorLevel 2
      Abort
    ${EndIf}
    ${If} $0 != 0
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "${PRODUCT_NAME} is currently running. Please close Didcord before uninstalling." /SD IDCANCEL IDRETRY retry_uninit IDCANCEL cancel_uninit
      retry_uninit:
        ${Continue}
      cancel_uninit:
        SetErrorLevel 1
        Abort
    ${Else}
      ${Break}
    ${EndIf}
  ${Loop}
FunctionEnd

Section "Uninstall"
  ; Remove Start Menu shortcut
  Delete "$SMPROGRAMS\${PRODUCT_NAME}.lnk"

  ; Remove Desktop shortcut if present
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  ; Remove Run registry key if startup was configured
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCT_NAME}"

  ; Remove Uninstall registry keys
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"

  ; Remove installed files
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.md"
  Delete "$INSTDIR\install-notifications.ps1"
  Delete "$INSTDIR\setup.ps1"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\docs"
  RMDir /r "$INSTDIR\licenses"
  RMDir /r "$INSTDIR\source"

  ; Remove installation directory if empty or leftover update staging
  RMDir /r "$INSTDIR"
SectionEnd
