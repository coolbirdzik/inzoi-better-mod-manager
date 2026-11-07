; Keep ordinary per-user installs unelevated, but support existing installs
; in protected locations selected through the setup's directory page.
Var InzoiWriteProbe
Var InzoiInstallerArgs
Var InzoiElevationAttempted

!macro NSIS_HOOK_PREINSTALL
  ClearErrors
  GetTempFileName $InzoiWriteProbe "$INSTDIR"
  ${If} ${Errors}
    ; Retry at most once. Permission errors on an elevated retry (e.g. a
    ; read-only drive) must stop instead of repeatedly opening UAC prompts.
    ClearErrors
    ${GetOptions} $CMDLINE "/INZOIMM_ELEVATED" $InzoiElevationAttempted
    ${IfNot} ${Errors}
      Abort "Setup cannot write to $INSTDIR even after requesting administrator permission."
    ${EndIf}

    ; NSIS removes /D from $CMDLINE. Preserve the remaining arguments,
    ; including Tauri's /P /UPDATE /R /ARGS, then put /D last and unquoted.
    ; Put our marker before /ARGS so it is never forwarded to the app.
    ${GetParameters} $InzoiInstallerArgs
    SetOutPath "$TEMP"
    ClearErrors
    ExecShell "runas" "$EXEPATH" "/INZOIMM_ELEVATED $InzoiInstallerArgs /D=$INSTDIR" SW_SHOWNORMAL
    ${If} ${Errors}
      Abort "Administrator permission was not granted. Run setup again and allow the Windows permission prompt."
    ${EndIf}
    Quit
  ${Else}
    Delete "$InzoiWriteProbe"
    ClearErrors
  ${EndIf}
!macroend
