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

!macro NSIS_HOOK_POSTINSTALL
  ${If} $Studio256UpgradeDir != ""
    !insertmacro MigrateLauncherShortcut "$SMPROGRAMS"
    !insertmacro MigrateLauncherShortcut "$DESKTOP"
    DeleteRegKey SHCTX "${LEGACY_UNINSTALL_KEY}"
    DeleteRegValue SHCTX "${LEGACY_APP_KEY}" ""
    DeleteRegKey /ifempty SHCTX "${LEGACY_APP_KEY}"
  ${EndIf}
!macroend
