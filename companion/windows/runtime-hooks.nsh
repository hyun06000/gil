; The launcher lives outside $INSTDIR and is deliberately not replaced by updates.
; Each signed installer adds a new immutable Core directory, then switches one small receipt.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$LOCALAPPDATA\GIL\runtime"
  CreateDirectory "$LOCALAPPDATA\GIL\runtime\versions"
  CreateDirectory "$LOCALAPPDATA\GIL\runtime\versions\${VERSION}"

  IfFileExists "$LOCALAPPDATA\GIL\runtime\versions\${VERSION}\gil.exe" 0 copy_core
  Abort "GIL Core ${VERSION} is already installed; refusing to replace immutable version bytes."
copy_core:
  ClearErrors
  CopyFiles /SILENT "$INSTDIR\bootstrap\gil.exe" "$LOCALAPPDATA\GIL\runtime\versions\${VERSION}"
  IfErrors 0 core_ready
  Abort "Could not install GIL Core ${VERSION}. The previous version remains selected."
core_ready:
  IfFileExists "$LOCALAPPDATA\GIL\runtime\gil-launcher.exe" launcher_ready
  ClearErrors
  CopyFiles /SILENT "$INSTDIR\bootstrap\gil-launcher.exe" "$LOCALAPPDATA\GIL\runtime"
  IfErrors 0 launcher_ready
  Abort "Could not install the stable GIL launcher."
launcher_ready:

  CreateDirectory "$LOCALAPPDATA\GIL\runtime\receipt-stage"
  Delete "$LOCALAPPDATA\GIL\runtime\receipt-stage\current.json"
  ClearErrors
  CopyFiles /SILENT "$INSTDIR\bootstrap\current.json" "$LOCALAPPDATA\GIL\runtime\receipt-stage"
  IfErrors 0 receipt_ready
  Abort "Could not prepare the GIL runtime receipt. The previous version remains selected."
receipt_ready:
  Delete "$LOCALAPPDATA\GIL\runtime\current.next"
  ClearErrors
  Rename "$LOCALAPPDATA\GIL\runtime\receipt-stage\current.json" "$LOCALAPPDATA\GIL\runtime\current.next"
  IfErrors 0 receipt_staged
  Abort "Could not stage the GIL runtime receipt. The previous version remains selected."
receipt_staged:
  RMDir "$LOCALAPPDATA\GIL\runtime\receipt-stage"
  Delete "$LOCALAPPDATA\GIL\runtime\current.previous"
  IfFileExists "$LOCALAPPDATA\GIL\runtime\current.json" 0 switch_current
  ClearErrors
  Rename "$LOCALAPPDATA\GIL\runtime\current.json" "$LOCALAPPDATA\GIL\runtime\current.previous"
  IfErrors 0 switch_current
  Abort "Could not preserve the selected GIL Core. No update was applied."
switch_current:
  ClearErrors
  Rename "$LOCALAPPDATA\GIL\runtime\current.next" "$LOCALAPPDATA\GIL\runtime\current.json"
  IfErrors 0 runtime_ready
  Rename "$LOCALAPPDATA\GIL\runtime\current.previous" "$LOCALAPPDATA\GIL\runtime\current.json"
  Abort "Could not select GIL Core ${VERSION}. The previous version was restored."
runtime_ready:
!macroend

; Uninstalling the Companion must not race or break active Codex conversations. The external
; launcher/Core remains until a separately verified cleanup flow is explicitly requested.
!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "GIL runtime and Project records are preserved. Disconnect Codex before uninstalling."
!macroend
