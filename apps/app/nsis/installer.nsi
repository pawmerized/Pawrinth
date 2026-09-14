; Pawrinth's Windows setup wizard. It starts from the NSIS template Tauri uses for
; Modrinth App and keeps its technology, with these differences:
; - Pages for install options and a summary, and shortcuts chosen before installing.
; - Files are replaced by a swap, so a stopped install leaves the old copy working.
; - An existing install is updated where it is, so there's never a second copy.
; - .mrpack stays with whichever app owns it, so Modrinth App's handler isn't touched.
; - Removing data is a separate choice with its own warning.

Unicode true
ManifestDPIAware true
; Add in `dpiAwareness` `PerMonitorV2` to manifest for Windows 10 1607+
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  SetCompressor /SOLID "{{compression}}"
!endif

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include nsDialogs.nsh
!include "utils.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define WEBVIEW2INSTALLERARGS "{{webview2_installer_args}}"
!define WEBVIEW2BOOTSTRAPPERPATH "{{webview2_bootstrapper_path}}"
!define WEBVIEW2INSTALLERPATH "{{webview2_installer_path}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define MODPACKEXTENSION ".mrpack"
!define MODPACKCLASS "${PRODUCTNAME}.mrpack"
!define BACKGROUNDCOLOR "16181C"
!define FOREGROUNDCOLOR "FFFFFF"
!define KEPTDATA "instances, worlds, saves, settings, themes and API keys"

Var PassiveMode
Var NoShortcutMode
Var ExistingVersion
Var ExistingLocation
Var VersionComparison
Var InterruptedInstall
Var CreateDesktopShortcut
Var CreateStartMenuShortcut
Var LaunchAfterInstall
Var DesktopCheckbox
Var StartMenuCheckbox
Var LaunchCheckbox
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
Var UninstallFinishText

Name "${PRODUCTNAME}"
Caption "${PRODUCTNAME} ${VERSION} Setup"
UninstallCaption "${PRODUCTNAME} ${VERSION} Uninstall"
BrandingText "${PRODUCTNAME} is an unofficial, community-developed fork of Modrinth App"
OutFile "${OUTFILE}"
ShowInstDetails show
ShowUninstDetails show
AllowSkipFiles off

; Only used so the folder browser appends the product name to whatever is chosen.
!define PLACEHOLDER_INSTALL_DIR "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME} Setup"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!addplugindir "${ADDITIONALPLUGINSPATH}"

!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

RequestExecutionLevel user

!define MUI_ICON "${INSTALLERICON}"
!define MUI_UNICON "${INSTALLERICON}"
!define MUI_ABORTWARNING
!define MUI_ABORTWARNING_TEXT "Setup hasn't changed anything yet. Are you sure you want to quit?"
!define MUI_UNABORTWARNING
!define MUI_BGCOLOR "${BACKGROUNDCOLOR}"
!define MUI_TEXTCOLOR "${FOREGROUNDCOLOR}"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!define MUI_HEADERIMAGE_UNBITMAP "${HEADERIMAGE}"

; 1. Welcome
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!define MUI_WELCOMEPAGE_TITLE "Welcome to ${PRODUCTNAME}"
!define MUI_WELCOMEPAGE_TEXT "This will install ${PRODUCTNAME} ${VERSION}, an unofficial, community-developed fork of Modrinth App for managing your Minecraft mods.$\r$\n$\r$\n${PRODUCTNAME} is separate from Modrinth App. It has its own files, shortcuts and data, and it won't change your Modrinth App installation.$\r$\n$\r$\nAdministrator permission isn't needed. Click Next to continue."
!insertmacro MUI_PAGE_WELCOME

; 2. An existing installation, only when there is one
Page custom PageExisting

; 3. Install location, skipped when updating so there's never a second copy
!define MUI_PAGE_CUSTOMFUNCTION_PRE PreDirectory
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE LeaveDirectory
!define MUI_DIRECTORYPAGE_TEXT_TOP "Setup will install ${PRODUCTNAME} in the folder below. To use another folder, click Browse and choose it. Your own folder works without administrator permission, but Program Files doesn't."
!insertmacro MUI_PAGE_DIRECTORY

; 4. Options
Page custom PageOptions PageLeaveOptions

; 5. Ready to install
Page custom PageReady

; 6. Installing
!define MUI_INSTFILESPAGE_FINISHHEADER_TEXT "Installation complete"
!define MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT "${PRODUCTNAME} is ready to use."
!define MUI_INSTFILESPAGE_ABORTHEADER_TEXT "Installation failed"
!define MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT "${PRODUCTNAME} wasn't installed. The details below say why."
!insertmacro MUI_PAGE_INSTFILES

; 7. Finished
!define MUI_FINISHPAGE_NOAUTOCLOSE
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!define MUI_PAGE_CUSTOMFUNCTION_SHOW ShowFinish
!define MUI_FINISHPAGE_TITLE "${PRODUCTNAME} is installed"
!define MUI_FINISHPAGE_TEXT "${PRODUCTNAME} ${VERSION} was installed successfully.$\r$\n$\r$\nClick Finish to close Setup."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${PRODUCTNAME}"
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
!define MUI_UNCONFIRMPAGE_TEXT_TOP "${PRODUCTNAME} will be removed from the folder below. Your data stays unless you choose otherwise, so you can install ${PRODUCTNAME} again later and carry on. Modrinth App isn't affected."
!insertmacro MUI_UNPAGE_CONFIRM
!define MUI_UNFINISHPAGE_NOAUTOCLOSE
!define /redef MUI_INSTFILESPAGE_FINISHHEADER_TEXT "Uninstall complete"
!define /redef MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT "${PRODUCTNAME} has been removed."
!define /redef MUI_INSTFILESPAGE_ABORTHEADER_TEXT "Uninstall didn't finish"
!define /redef MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT "Some of ${PRODUCTNAME} is still installed. The details below say why."
!insertmacro MUI_UNPAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!define MUI_FINISHPAGE_TITLE "${PRODUCTNAME} has been removed"
!define MUI_FINISHPAGE_TEXT "$UninstallFinishText"
!insertmacro MUI_UNPAGE_FINISH

{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
{{#each language_files}}
  !include "{{this}}"
{{/each}}

LangString ^FileError ${LANG_ENGLISH} "Setup couldn't write this file:$\r$\n$\r$\n$0$\r$\n$\r$\nCheck that ${PRODUCTNAME} is closed and that the folder isn't read-only. Click Retry to try again, or Abort to stop. Your data isn't affected."
LangString ^FileError_NoIgnore ${LANG_ENGLISH} "Setup couldn't write this file:$\r$\n$\r$\n$0$\r$\n$\r$\nCheck that ${PRODUCTNAME} is closed and that the folder isn't read-only. Click Retry to try again, or Cancel to stop. Your data isn't affected."

!macro CloseRunningApp
  nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 0
    ${IfNot} ${Silent}
    ${AndIf} $PassiveMode <> 1
      MessageBox MB_OKCANCEL|MB_ICONINFORMATION "${PRODUCTNAME} is open and has to close first.$\r$\n$\r$\nClick OK to close it now. Any downloads it's doing will stop." IDOK +2
      Abort "${PRODUCTNAME} is still open, so nothing was changed."
    ${EndIf}
    DetailPrint "Closing ${PRODUCTNAME}..."
    nsis_tauri_utils::KillProcessCurrentUser "${MAINBINARYNAME}.exe"
    Pop $R0
    Sleep 1000
    ${If} $R0 <> 0
      Abort "${PRODUCTNAME} couldn't be closed, so nothing was changed."
    ${EndIf}
  ${EndIf}
!macroend

Function ReadExistingInstall
  ReadRegStr $ExistingVersion SHCTX "${UNINSTKEY}" "DisplayVersion"
  ReadRegStr $ExistingLocation SHCTX "${UNINSTKEY}" "InstallLocation"
  ReadRegStr $InterruptedInstall HKCU "${MANUPRODUCTKEY}" "InstallInProgress"
  ; The location is stored in quotes for Windows' list of apps
  StrCpy $0 $ExistingLocation 1
  ${If} $0 == '"'
    StrCpy $ExistingLocation $ExistingLocation "" 1
    StrCpy $ExistingLocation $ExistingLocation -1
  ${EndIf}
  ${If} $ExistingVersion != ""
  ${AndIf} $ExistingLocation != ""
    nsis_tauri_utils::SemverCompare "${VERSION}" "$ExistingVersion"
    Pop $VersionComparison
  ${Else}
    StrCpy $ExistingVersion ""
    StrCpy $ExistingLocation ""
  ${EndIf}
FunctionEnd

Function .onInit
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NS" $NoShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  !insertmacro SetContext

  StrCpy $CreateDesktopShortcut 1
  StrCpy $CreateStartMenuShortcut 1
  StrCpy $LaunchAfterInstall 1
  ${If} $NoShortcutMode = 1
    StrCpy $CreateDesktopShortcut 0
    StrCpy $CreateStartMenuShortcut 0
  ${EndIf}

  Call ReadExistingInstall

  ${If} $ExistingLocation != ""
    StrCpy $INSTDIR $ExistingLocation
  ${ElseIf} $INSTDIR == "${PLACEHOLDER_INSTALL_DIR}"
    StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
  ${EndIf}
FunctionEnd

Function PageExisting
  ${If} $ExistingVersion == ""
  ${OrIf} $PassiveMode = 1
    Abort
  ${EndIf}

  ${If} $VersionComparison = -1
    !insertmacro MUI_HEADER_TEXT "A newer version is installed" "This Setup can't continue."
    StrCpy $R1 "${PRODUCTNAME} $ExistingVersion is already installed in:$\r$\n$ExistingLocation$\r$\n$\r$\nThis Setup is for the older version ${VERSION}, and your data may not open in an older version. To install it anyway, uninstall ${PRODUCTNAME} first from Windows Settings. Your data stays."
  ${ElseIf} $VersionComparison = 0
    !insertmacro MUI_HEADER_TEXT "${PRODUCTNAME} is already installed" "Setup will reinstall it in the same place."
    StrCpy $R1 "${PRODUCTNAME} ${VERSION} is already installed in:$\r$\n$ExistingLocation$\r$\n$\r$\nSetup will reinstall it there. Your ${KEPTDATA} are kept.$\r$\n$\r$\nTo move it to another folder, uninstall it first, then run this Setup again."
  ${Else}
    !insertmacro MUI_HEADER_TEXT "${PRODUCTNAME} is already installed" "Setup will update it in the same place."
    StrCpy $R1 "${PRODUCTNAME} $ExistingVersion is installed in:$\r$\n$ExistingLocation$\r$\n$\r$\nSetup will update it to ${VERSION}. Your ${KEPTDATA} are kept.$\r$\n$\r$\nTo move it to another folder, uninstall it first, then run this Setup again."
  ${EndIf}
  ${If} $InterruptedInstall != ""
    StrCpy $R1 "$R1$\r$\n$\r$\nThe last install didn't finish, so this will also repair it."
  ${EndIf}

  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 100% "$R1"
  Pop $0
  ${If} $VersionComparison = -1
    GetDlgItem $0 $HWNDPARENT 1
    EnableWindow $0 0
  ${EndIf}
  nsDialogs::Show
FunctionEnd

Function PreDirectory
  ${If} $PassiveMode = 1
  ${OrIf} $ExistingLocation != ""
    Abort
  ${EndIf}
FunctionEnd

Function LeaveDirectory
  ${StrCase} $1 "$INSTDIR\" "L"
  ${StrCase} $2 "$LOCALAPPDATA\Modrinth App\" "L"
  ${StrCase} $3 "$APPDATA\ModrinthApp\" "L"
  ${StrLoc} $0 $1 $2 ">"
  ${StrLoc} $4 $1 $3 ">"
  ${StrLoc} $5 $1 "\com.modrinth.theseus\" ">"
  ${If} $0 == "0"
  ${OrIf} $4 == "0"
  ${OrIf} $5 != ""
    MessageBox MB_OK|MB_ICONEXCLAMATION "That folder belongs to Modrinth App. ${PRODUCTNAME} has to go in a folder of its own."
    Abort
  ${EndIf}

  ${If} ${FileExists} "$INSTDIR\*.*"
    StrCpy $6 1
  ${Else}
    StrCpy $6 0
  ${EndIf}
  ClearErrors
  CreateDirectory "$INSTDIR"
  FileOpen $0 "$INSTDIR\.pawrinth-setup-check" w
  ${If} ${Errors}
    RMDir "$INSTDIR"
    MessageBox MB_OK|MB_ICONEXCLAMATION "Setup can't write to this folder without administrator permission:$\r$\n$\r$\n$INSTDIR$\r$\n$\r$\nChoose a folder you own, such as the one Setup suggested."
    Abort
  ${EndIf}
  FileClose $0
  Delete "$INSTDIR\.pawrinth-setup-check"
  ${If} $6 = 0
    RMDir "$INSTDIR"
  ${EndIf}

  ${If} $6 = 1
  ${AndIfNot} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    ${DirState} "$INSTDIR" $0
    ${If} $0 = 1
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "This folder already has other files in it:$\r$\n$\r$\n$INSTDIR$\r$\n$\r$\nInstall here anyway? Those files won't be changed." IDYES +2
      Abort
    ${EndIf}
  ${EndIf}
FunctionEnd

Function PageOptions
  ${If} $PassiveMode = 1
  ${OrIf} $VersionComparison = -1
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Installation options" "Choose how you'd like to open ${PRODUCTNAME}."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateCheckbox} 0 8u 100% 12u "Create a desktop shortcut"
  Pop $DesktopCheckbox
  ${NSD_SetState} $DesktopCheckbox $CreateDesktopShortcut
  ${NSD_CreateCheckbox} 0 28u 100% 12u "Create a Start Menu shortcut"
  Pop $StartMenuCheckbox
  ${NSD_SetState} $StartMenuCheckbox $CreateStartMenuShortcut
  ${NSD_CreateCheckbox} 0 48u 100% 12u "Launch ${PRODUCTNAME} after installation"
  Pop $LaunchCheckbox
  ${NSD_SetState} $LaunchCheckbox $LaunchAfterInstall
  nsDialogs::Show
FunctionEnd

Function PageLeaveOptions
  ${NSD_GetState} $DesktopCheckbox $CreateDesktopShortcut
  ${NSD_GetState} $StartMenuCheckbox $CreateStartMenuShortcut
  ${NSD_GetState} $LaunchCheckbox $LaunchAfterInstall
FunctionEnd

!macro YesOrNo value out
  ${If} ${value} = 1
    StrCpy ${out} "Yes"
  ${Else}
    StrCpy ${out} "No"
  ${EndIf}
!macroend

Function ShowFinish
  ; Windows draws themed checkboxes in black, which can't be read on the dark page
  System::Call 'uxtheme::SetWindowTheme(p $mui.FinishPage.Run, w " ", w " ")'
  SetCtlColors $mui.FinishPage.Run "${FOREGROUNDCOLOR}" "${BACKGROUNDCOLOR}"
  ${NSD_SetState} $mui.FinishPage.Run $LaunchAfterInstall
FunctionEnd

Function RunMainBinary
  nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
FunctionEnd

Function un.ConfirmShow
  ; $1 inner dialog, $2 DPI, $3 style, $4 x, $5 y, $6 width, $7 height
  FindWindow $1 "#32770" "" $HWNDPARENT
  System::Call "user32::GetDpiForWindow(p r1) i .r2"
  StrCpy $3 "${__NSD_CheckBox_EXSTYLE}"
  IntOp $4 0 * $2
  IntOp $5 96 * $2
  IntOp $6 430 * $2
  IntOp $7 24 * $2
  IntOp $4 $4 / 96
  IntOp $5 $5 / 96
  IntOp $6 $6 / 96
  IntOp $7 $7 / 96
  System::Call 'user32::CreateWindowEx(i r3, w "${__NSD_CheckBox_CLASS}", w "Also delete my ${PRODUCTNAME} data", i ${__NSD_CheckBox_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
  Pop $DeleteAppDataCheckbox
  SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $R9
  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $R9 1

  IntOp $5 122 * $2
  IntOp $7 60 * $2
  IntOp $5 $5 / 96
  IntOp $7 $7 / 96
  System::Call 'user32::CreateWindowEx(i 0, w "${__NSD_Label_CLASS}", w "Permanently deletes your ${KEPTDATA} from this computer. Leave this off to keep them. Modrinth App$\'s data is never touched.", i ${__NSD_Label_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
  Pop $0
  SendMessage $0 ${WM_SETFONT} $R9 1
FunctionEnd

Function un.ConfirmLeave
  SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
  ${If} $DeleteAppDataCheckboxState = 1
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "This permanently deletes your ${KEPTDATA}. It can't be undone.$\r$\n$\r$\nDelete your ${PRODUCTNAME} data?" IDYES +3
    SendMessage $DeleteAppDataCheckbox ${BM_SETCHECK} 0 0
    Abort
  ${EndIf}
FunctionEnd

Function .onInstFailed
  ; Setup stopped on purpose here, so this isn't an interrupted install to repair later
  DeleteRegValue HKCU "${MANUPRODUCTKEY}" "InstallInProgress"
  DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
  DeleteRegKey /ifempty HKCU "${MANUKEY}"
  ${IfNot} ${Silent}
  ${AndIf} $PassiveMode <> 1
    MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME} wasn't installed. The details in the window say why.$\r$\n$\r$\nYour data wasn't touched. If ${PRODUCTNAME} was already installed, that copy still works."
  ${EndIf}
FunctionEnd

Section EarlyChecks
  ${If} $VersionComparison = -1
    ${If} ${Silent}
      SetErrorLevel 3
    ${EndIf}
    Abort "A newer version of ${PRODUCTNAME} is installed. Uninstall it first to install this one."
  ${EndIf}
SectionEnd

Section WebView2
  ; Check if Webview2 is already installed and skip this section
  ${If} ${RunningX64}
    ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${Else}
    ReadRegStr $4 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ${If} $4 == ""
    ReadRegStr $4 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}

  ${If} $4 == ""
    !if "${INSTALLWEBVIEW2MODE}" == "downloadBootstrapper"
      Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
      DetailPrint "$(webview2Downloading)"
      NSISdl::download "https://go.microsoft.com/fwlink/p/?LinkId=2124703" "$TEMP\MicrosoftEdgeWebview2Setup.exe"
      Pop $0
      ${If} $0 == "success"
        DetailPrint "$(webview2DownloadSuccess)"
      ${Else}
        DetailPrint "$(webview2DownloadError)"
        Abort "$(webview2AbortError)"
      ${EndIf}
      StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
      Goto install_webview2
    !endif

    !if "${INSTALLWEBVIEW2MODE}" == "embedBootstrapper"
      Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
      File "/oname=$TEMP\MicrosoftEdgeWebview2Setup.exe" "${WEBVIEW2BOOTSTRAPPERPATH}"
      DetailPrint "$(installingWebview2)"
      StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
      Goto install_webview2
    !endif

    !if "${INSTALLWEBVIEW2MODE}" == "offlineInstaller"
      Delete "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
      File "/oname=$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe" "${WEBVIEW2INSTALLERPATH}"
      DetailPrint "$(installingWebview2)"
      StrCpy $6 "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
      Goto install_webview2
    !endif

    Goto webview2_done

    install_webview2:
      DetailPrint "$(installingWebview2)"
      ; $6 holds the path to the webview2 installer
      ExecWait "$6 ${WEBVIEW2INSTALLERARGS} /install" $1
      ${If} $1 = 0
        DetailPrint "$(webview2InstallSuccess)"
      ${Else}
        DetailPrint "$(webview2InstallError)"
        Abort "$(webview2AbortError)"
      ${EndIf}
    webview2_done:
  ${Else}
    !if "${MINIMUMWEBVIEW2VERSION}" != ""
      ${VersionCompare} "${MINIMUMWEBVIEW2VERSION}" "$4" $R0
      ${If} $R0 = 1
        update_webview:
          DetailPrint "$(installingWebview2)"
          ${If} ${RunningX64}
            ReadRegStr $R1 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate" "path"
          ${Else}
            ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\EdgeUpdate" "path"
          ${EndIf}
          ${If} $R1 == ""
            ReadRegStr $R1 HKCU "SOFTWARE\Microsoft\EdgeUpdate" "path"
          ${EndIf}
          ${If} $R1 != ""
            ExecWait `"$R1" /install appguid=${WEBVIEW2APPGUID}&needsadmin=true` $1
            ${If} $1 = 0
              DetailPrint "$(webview2InstallSuccess)"
            ${Else}
              MessageBox MB_ICONEXCLAMATION|MB_ABORTRETRYIGNORE "$(webview2InstallError)" IDIGNORE ignore IDRETRY update_webview
              Quit
              ignore:
            ${EndIf}
          ${EndIf}
      ${EndIf}
    !endif
  ${EndIf}
SectionEnd

Section Install SEC_INSTALL
  ClearErrors
  CreateDirectory "$INSTDIR"
  ${If} ${Errors}
    Abort "Setup couldn't create $INSTDIR. Choose a folder you own, or run Setup again."
  ${EndIf}
  SetOutPath $INSTDIR

  !insertmacro CloseRunningApp

  WriteRegStr HKCU "${MANUPRODUCTKEY}" "InstallInProgress" "${VERSION}"

  ; The old program stays in place until the new one is fully written, so stopping here
  ; at any point leaves whatever was installed still working.
  DetailPrint "Copying ${PRODUCTNAME}..."
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.new"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.old"
  ClearErrors
  File "/oname=${MAINBINARYNAME}.exe.new" "${MAINBINARYSRCPATH}"
  ${If} ${Errors}
    Abort "Setup couldn't write to $INSTDIR. Check there's enough free space and try again."
  ${EndIf}

  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    ClearErrors
    Rename "$INSTDIR\${MAINBINARYNAME}.exe" "$INSTDIR\${MAINBINARYNAME}.exe.old"
    ${If} ${Errors}
      Delete "$INSTDIR\${MAINBINARYNAME}.exe.new"
      Abort "Something is using ${MAINBINARYNAME}.exe. Close it and run Setup again."
    ${EndIf}
  ${EndIf}
  ClearErrors
  Rename "$INSTDIR\${MAINBINARYNAME}.exe.new" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${If} ${Errors}
    Rename "$INSTDIR\${MAINBINARYNAME}.exe.old" "$INSTDIR\${MAINBINARYNAME}.exe"
    Abort "Setup couldn't replace ${MAINBINARYNAME}.exe, so the existing copy was kept."
  ${EndIf}
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.old"

  ; Copy resources
  {{#each resources_dirs}}
    CreateDirectory "$INSTDIR\\{{this}}"
  {{/each}}
  {{#each resources}}
    File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
  {{/each}}

  ; Copy external binaries
  {{#each binaries}}
    File /a "/oname={{this}}" "{{no-escape @key}}"
  {{/each}}

  ; Offer to open modpacks, and only become the default if nothing else is
  DetailPrint "Registering ${PRODUCTNAME}..."
  WriteRegStr SHCTX "Software\Classes\${MODPACKCLASS}" "" "Modrinth modpack"
  WriteRegStr SHCTX "Software\Classes\${MODPACKCLASS}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr SHCTX "Software\Classes\${MODPACKCLASS}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  WriteRegStr SHCTX "Software\Classes\${MODPACKEXTENSION}\OpenWithProgids" "${MODPACKCLASS}" ""
  ReadRegStr $R0 SHCTX "Software\Classes\${MODPACKEXTENSION}" ""
  ${If} $R0 == ""
    WriteRegStr SHCTX "Software\Classes\${MODPACKEXTENSION}" "" "${MODPACKCLASS}"
  ${EndIf}

  ; Register deep links
  {{#each deep_link_protocols as |protocol| ~}}
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "URL Protocol" ""
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "" "URL:${BUNDLEID} protocol"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  {{/each}}

  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR
  WriteRegStr SHCTX "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"

  ${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
  IntOp $0 $0 + ${ESTIMATEDSIZE}
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "$0"

  !if "${HOMEPAGE}" != ""
    WriteRegStr SHCTX "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
    WriteRegStr SHCTX "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
  !endif

  ; Shortcuts follow the choices made, and only ever touch ones that lead to this install
  DetailPrint "Creating shortcuts..."
  ${If} $CreateStartMenuShortcut = 1
    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${Else}
    !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}
  ${If} $CreateDesktopShortcut = 1
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${Else}
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}

  DeleteRegValue HKCU "${MANUPRODUCTKEY}" "InstallInProgress"
  DetailPrint "${PRODUCTNAME} ${VERSION} is installed."

  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Function PageReady
  ${If} $PassiveMode = 1
  ${OrIf} $VersionComparison = -1
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Ready to install" "Check your choices, then click Install."
  SectionGetSize ${SEC_INSTALL} $R0
  IntOp $R0 $R0 / 1024
  !insertmacro YesOrNo $CreateDesktopShortcut $R1
  !insertmacro YesOrNo $CreateStartMenuShortcut $R2
  !insertmacro YesOrNo $LaunchAfterInstall $R3
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 4u 100% 92u "Version: ${VERSION}$\r$\n$\r$\nInstall location: $INSTDIR$\r$\n$\r$\nSpace needed: about $R0 MB$\r$\n$\r$\nDesktop shortcut: $R1$\r$\nStart Menu shortcut: $R2$\r$\nLaunch after installation: $R3"
  Pop $0
  ${NSD_CreateLabel} 0 104u 100% 24u "Click Back to change any of this. Your data is stored separately, and Modrinth App won't be changed."
  Pop $0
  GetDlgItem $0 $HWNDPARENT 1
  SendMessage $0 ${WM_SETTEXT} 0 "STR:Install"
  nsDialogs::Show
FunctionEnd

Function .onInstSuccess
  ; The Finish page handles launching unless there isn't one
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
    ${EndIf}
  ${EndIf}
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1 ${|} Abort ${|}
FunctionEnd

Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1 ${|} Abort ${|}
FunctionEnd

Function un.onInit
  !insertmacro SetContext
  !insertmacro MUI_UNGETLANGUAGE

  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}
  StrCpy $UninstallFinishText "${PRODUCTNAME} was removed from this computer.$\r$\n$\r$\nYour ${KEPTDATA} were kept, so they'll be there if you install ${PRODUCTNAME} again."
FunctionEnd

Section Uninstall
  !insertmacro CloseRunningApp

  DetailPrint "Removing ${PRODUCTNAME}..."
  ClearErrors
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"
  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    Abort "Something is using ${MAINBINARYNAME}.exe. Close it and try again."
  ${EndIf}
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.new"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.old"

  {{#each resources}}
    Delete "$INSTDIR\\{{this.[1]}}"
  {{/each}}

  {{#each binaries}}
    Delete "$INSTDIR\\{{this}}"
  {{/each}}

  ; Only Pawrinth's own entries. Modrinth App's handler for modpacks stays as it was.
  DeleteRegKey SHCTX "Software\Classes\${MODPACKCLASS}"
  DeleteRegValue SHCTX "Software\Classes\${MODPACKEXTENSION}\OpenWithProgids" "${MODPACKCLASS}"
  DeleteRegKey /ifempty SHCTX "Software\Classes\${MODPACKEXTENSION}\OpenWithProgids"
  ReadRegStr $R0 SHCTX "Software\Classes\${MODPACKEXTENSION}" ""
  ${If} $R0 == "${MODPACKCLASS}"
    DeleteRegValue SHCTX "Software\Classes\${MODPACKEXTENSION}" ""
    DeleteRegKey /ifempty SHCTX "Software\Classes\${MODPACKEXTENSION}"
  ${EndIf}

  {{#each deep_link_protocols as |protocol| ~}}
    ReadRegStr $R7 SHCTX "Software\Classes\\{{protocol}}\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey SHCTX "Software\Classes\\{{protocol}}"
    ${EndIf}
  {{/each}}

  Delete "$INSTDIR\uninstall.exe"

  {{#each resources_ancestors}}
  RMDir /REBOOTOK "$INSTDIR\\{{this}}"
  {{/each}}
  RMDir "$INSTDIR"
  ${If} ${FileExists} "$INSTDIR\*.*"
    DetailPrint "$INSTDIR was left in place because it has other files in it."
  ${EndIf}

  !insertmacro DeleteAppUserModelId

  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}

  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}

  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
  DeleteRegKey /ifempty SHCTX "${MANUKEY}"

  ${If} $DeleteAppDataCheckboxState = 1
    SetShellVarContext current
    DetailPrint "Deleting your ${PRODUCTNAME} data..."
    RMDir /r "$APPDATA\${BUNDLEID}"
    RMDir /r "$LOCALAPPDATA\${BUNDLEID}"
    RMDir /r "$LOCALAPPDATA\${PRODUCTNAME} Backups"
    nsExec::Exec 'cmdkey /delete:curseforge-api-key.${BUNDLEID}'
    Pop $0
    nsExec::Exec 'cmdkey /delete:mineskin-api-key.${BUNDLEID}'
    Pop $0
    StrCpy $UninstallFinishText "${PRODUCTNAME} and its data were removed from this computer.$\r$\n$\r$\nModrinth App wasn't changed."
  ${Else}
    DetailPrint "Your data was kept."
  ${EndIf}

  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd
