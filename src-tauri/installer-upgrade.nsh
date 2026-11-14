!define LEGACY_PRODUCT "IntelGen Game Launcher"
!define LEGACY_APP_KEY "Software\IntelGen\${LEGACY_PRODUCT}"
!define LEGACY_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LEGACY_PRODUCT}"

Var Studio256UpgradeDir

!macro NSIS_HOOK_PREINSTALL
  ; Tauri derives the NSIS registration from the product name. Reuse a verified
  ; previous install on upgrades so existing shortcuts and startup entries work.
  ReadRegStr $R0 SHCTX "${LEGACY_APP_KEY}" ""
  ReadRegStr $R1 SHCTX "${LEGACY_UNINSTALL_KEY}" "InstallLocation"
  ${If} $R0 != ""
  ${AndIf} $R1 == '$\"$R0$\"'
  ${AndIf} ${FileExists} "$R0\${MAINBINARYNAME}.exe"
  ${AndIf} ${FileExists} "$R0\uninstall.exe"
    ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
    ${OrIf} $INSTDIR == $R0
      StrCpy $Studio256UpgradeDir $R0
      StrCpy $INSTDIR $R0
      SetOutPath $INSTDIR
    ${EndIf}
  ${EndIf}
!macroend

!macro MigrateLauncherShortcut folder
  !insertmacro IsShortcutTarget "${folder}\${LEGACY_PRODUCT}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 1
    ${IfNot} ${FileExists} "${folder}\${PRODUCTNAME}.lnk"
      Rename "${folder}\${LEGACY_PRODUCT}.lnk" "${folder}\${PRODUCTNAME}.lnk"
    ${Else}
      !insertmacro IsShortcutTarget "${folder}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      Pop $R0
      ${If} $R0 = 1
        Delete "${folder}\${LEGACY_PRODUCT}.lnk"
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

; Keep the shortcut's AppUserModelID and arguments, including on existing pins.
!macro RefreshCartridgeIcon shortcut
  ${If} ${FileExists} "${shortcut}"
    !insertmacro IsShortcutTarget "${shortcut}" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $R0
    ${If} $R0 = 1
      !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 ""
      ${If} $0 P<> 0
        ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}", .r1)'
        ${If} $1 P<> 0
          ${IPersistFile::Load} $1 '("${shortcut}", ${STGM_READWRITE})'
          ${IShellLink::SetIconLocation} $0 '(w "$INSTDIR\studio-256-cartridge.ico", i 0)'
          ${IPersistFile::Save} $1 '("${shortcut}", 1)'
          ${IUnknown::Release} $1 ""
          System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x1005, w "${shortcut}", p 0)'
        ${EndIf}
        ${IUnknown::Release} $0 ""
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $Studio256UpgradeDir != ""
    !insertmacro MigrateLauncherShortcut "$SMPROGRAMS"
    !insertmacro MigrateLauncherShortcut "$DESKTOP"
    DeleteRegKey SHCTX "${LEGACY_UNINSTALL_KEY}"
    DeleteRegValue SHCTX "${LEGACY_APP_KEY}" ""
    DeleteRegKey /ifempty SHCTX "${LEGACY_APP_KEY}"
  ${EndIf}
  !insertmacro RefreshCartridgeIcon "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  !insertmacro RefreshCartridgeIcon "$DESKTOP\${PRODUCTNAME}.lnk"
  ; Older pins can have a custom filename. Match the target before writing.
  FindFirst $R8 $R9 "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk"
  ${DoWhile} $R9 != ""
    !insertmacro RefreshCartridgeIcon "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\$R9"
    FindNext $R8 $R9
  ${Loop}
  FindClose $R8
!macroend
