; NSIS hooks for the codex-barbar per-user installer.
;
; Install dir: Tauri's currentUser default is %LOCALAPPDATA%\codex-barbar,
; which collides with the app's data root. Relocate only the default install
; directory to %LOCALAPPDATA%\Programs\codex-barbar so the explicit
; --purge-user-data mode can delete the data root without deleting the
; running executable or uninstaller. User-chosen custom directories are
; preserved.
!macro NSIS_HOOK_PREINSTALL
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\${PRODUCTNAME}"
    ; The template already ran SetOutPath against the old default; re-point it
    ; before the first File command so the payload lands in the new location.
    SetOutPath $INSTDIR
  ${EndIf}
!macroend

; Default uninstall preserves user data. The uninstaller only asks before the
; app files are removed (PREUNINSTALL) so the installed binary still exists
; and can be invoked with the fixed internal purge mode. An explicit Yes runs
; the purge; a failure leaves the data in place and tells the user why.
; /SD IDNO pins silent uninstalls to the data-preserving default: NSIS
; MessageBox otherwise returns IDYES in silent mode.
!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Delete local codex-barbar accounts and cache?" /SD IDNO IDYES purge_yes IDNO purge_no
purge_yes:
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --purge-user-data' $0
  ${If} $0 <> 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "codex-barbar could not delete its local data. Close codex-barbar, then uninstall again and choose Yes."
  ${EndIf}
  Goto purge_done
purge_no:
  Nop
purge_done:
!macroend
