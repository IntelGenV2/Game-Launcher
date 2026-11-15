!define LEGACY_PRODUCT "IntelGen Game Launcher"
!define LEGACY_APP_KEY "Software\IntelGen\${LEGACY_PRODUCT}"
!define LEGACY_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LEGACY_PRODUCT}"

Var Studio256UpgradeDir
Var Studio256ShortcutTarget
Var Studio256FinishDesktopLink

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

!define STUDIO256_SHORTCUT_ICON "studio-256-cartridge-v3.ico"

; Edit the loaded link so taskbar pins retain their arguments and AppUserModelID.
Function Studio256RefreshLink
  Pop $R6
  IfFileExists "$R6" 0 done
  !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 ".r3"
  StrCmp $3 0 0 done
  ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}", .r1)i.r3'
  StrCmp $3 0 0 releaseLink
  ${IPersistFile::Load} $1 '(w "$R6", ${STGM_READWRITE})i.r3'
  StrCmp $3 0 0 releaseFile
  ${IShellLink::GetPath} $0 '(w .r2, i ${NSIS_MAX_STRLEN}, p 0, i 0)i.r3'
  StrCmp $3 0 0 releaseFile
  StrCmp $2 $Studio256ShortcutTarget 0 releaseFile
  ${IShellLink::SetIconLocation} $0 '(w "$INSTDIR\${STUDIO256_SHORTCUT_ICON}", i 0)i.r3'
  StrCmp $3 0 0 releaseFile
  ${IPersistFile::Save} $1 '(p 0, i 1)i.r3'
  StrCmp $3 0 0 releaseFile
  ; SHCNE_UPDATEITEM, SHCNF_PATHW | SHCNF_FLUSHNOWAIT.
  System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x2005, w "$R6", p 0)'
  releaseFile:
  ${IUnknown::Release} $1 ""
  releaseLink:
  ${IUnknown::Release} $0 ""
  done:
FunctionEnd

Function Studio256RefreshFolder
  Pop $R7
  FindFirst $R8 $R9 "$R7\*.lnk"
  StrCmp $R9 "" done
  ${DoWhile} $R9 != ""
    Push "$R7\$R9"
    Call Studio256RefreshLink
    FindNext $R8 $R9
  ${Loop}
  FindClose $R8
  done:
FunctionEnd

; Windows groups Start menu links and implicit taskbar pins in subdirectories.
; Stay within the shell folder, including when a user has placed junctions there.
Function Studio256RefreshTree
  Exch $R3
  Push $R4
  Push $R5
  Push "$R3"
  Call Studio256RefreshFolder
  FindFirst $R4 $R5 "$R3\*"
  StrCmp $R5 "" done
  ${DoWhile} $R5 != ""
    ${If} $R5 != "."
    ${AndIf} $R5 != ".."
      System::Call 'kernel32::GetFileAttributesW(w "$R3\$R5")i.r2'
      IntOp $2 $2 & 0x410 ; FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT
      ${If} $2 = 0x10
        Push "$R3\$R5"
        Call Studio256RefreshTree
      ${EndIf}
    ${EndIf}
    FindNext $R4 $R5
  ${Loop}
  FindClose $R4
  done:
  Pop $R5
  Pop $R4
  Pop $R3
FunctionEnd

; Tauri's Finish button can create or replace the desktop shortcut after POSTINSTALL.
Function .onGUIEnd
  ${If} $Studio256FinishDesktopLink != ""
    Push "$Studio256FinishDesktopLink"
    Call Studio256RefreshLink
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  ${If} $Studio256UpgradeDir != ""
    !insertmacro MigrateLauncherShortcut "$SMPROGRAMS"
    !insertmacro MigrateLauncherShortcut "$DESKTOP"
    DeleteRegKey SHCTX "${LEGACY_UNINSTALL_KEY}"
    DeleteRegValue SHCTX "${LEGACY_APP_KEY}" ""
    DeleteRegKey /ifempty SHCTX "${LEGACY_APP_KEY}"
  ${EndIf}
  StrCpy $Studio256ShortcutTarget "$INSTDIR\${MAINBINARYNAME}.exe"
  ; Custom shortcuts outside the shell folders can still reference earlier icon paths.
  System::Call 'kernel32::CopyFileW(w "$INSTDIR\${STUDIO256_SHORTCUT_ICON}", w "$INSTDIR\studio-256-cartridge.ico", i 0)'
  System::Call 'kernel32::CopyFileW(w "$INSTDIR\${STUDIO256_SHORTCUT_ICON}", w "$INSTDIR\studio-256-cartridge-v2.ico", i 0)'
  System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x2005, w "$INSTDIR\studio-256-cartridge.ico", p 0)'
  System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x2005, w "$INSTDIR\studio-256-cartridge-v2.ico", p 0)'
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" '$\"$INSTDIR\${STUDIO256_SHORTCUT_ICON}$\",0'
  System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x2005, w "$Studio256ShortcutTarget", p 0)'
  System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x2005, w "$INSTDIR\uninstall.exe", p 0)'
  Push "$SMPROGRAMS"
  Call Studio256RefreshTree
  Push "$DESKTOP"
  Call Studio256RefreshFolder
  Push "$APPDATA\Microsoft\Internet Explorer\Quick Launch"
  Call Studio256RefreshFolder
  Push "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"
  Call Studio256RefreshFolder
  Push "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\ImplicitAppShortcuts"
  Call Studio256RefreshTree
  ${IfNot} ${Silent}
  ${AndIf} $PassiveMode != 1
    StrCpy $Studio256FinishDesktopLink "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\studio-256-cartridge.ico"
  Delete "$INSTDIR\studio-256-cartridge-v2.ico"
!macroend
