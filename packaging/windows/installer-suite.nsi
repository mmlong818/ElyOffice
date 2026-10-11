; Per-user ElyOffice suite installer. The staged runtime must already be complete.
Unicode true
!include "MUI2.nsh"
!include "StrFunc.nsh"
${StrTrimNewLines}

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef STAGE
  !error "pass -DSTAGE"
!endif
!ifndef OUTFILE
  !error "pass -DOUTFILE"
!endif
!ifndef ICON
  !error "pass -DICON"
!endif
!ifndef UNINSTALL_FILES
  !error "pass -DUNINSTALL_FILES"
!endif
!ifndef UNINSTALL_DIRS
  !error "pass -DUNINSTALL_DIRS"
!endif
!ifndef INSTALL_GUARDS
  !error "pass -DINSTALL_GUARDS"
!endif

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ElyOffice"
!define SUITE_KEY "Software\ElyOffice\Suite"
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
Name "ElyOffice ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\ElyOffice"
RequestExecutionLevel user
SetCompressor /SOLID lzma

Var ElySameSuite
Var ElyProbePath
Var ElyProbeOK

Function CheckInstallDirectoryWritable
  Push $0
  StrCpy $ElyProbePath ""
  StrCpy $ElyProbeOK "0"
  ClearErrors
  CreateDirectory "$INSTDIR"
  IfErrors probe_done

  ClearErrors
  GetTempFileName $ElyProbePath "$INSTDIR"
  IfErrors probe_done
  FileOpen $0 "$ElyProbePath" w
  IfErrors probe_cleanup
  FileWrite $0 "ElyOffice writable-directory check$\r$\n"
  IfErrors probe_close_failed
  FileClose $0
  IfErrors probe_cleanup
  Delete "$ElyProbePath"
  IfErrors probe_done
  StrCpy $ElyProbePath ""
  StrCpy $ElyProbeOK "1"
  Goto probe_done

probe_close_failed:
  FileClose $0
probe_cleanup:
  ClearErrors
  StrCmp $ElyProbePath "" probe_done
  Delete "$ElyProbePath"
  ClearErrors
  StrCpy $ElyProbePath ""
probe_done:
  Pop $0
FunctionEnd

Function ElyDirectoryLeave
  Call CheckInstallDirectoryWritable
  StrCmp $ElyProbeOK "1" directory_leave_done
  IfSilent directory_leave_silent
  MessageBox MB_OK|MB_ICONSTOP "ElyOffice cannot write to $INSTDIR. Choose a folder your account can write to, such as $LOCALAPPDATA\Programs\ElyOffice or D:\ElyOffice, then try again. No administrator permission will be requested."
  Abort
directory_leave_silent:
  SetErrorLevel 5
  Abort
directory_leave_done:
FunctionEnd

Function .onInit
  StrCpy $ElySameSuite "0"
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "UninstallString"
  StrCmp $0 "" no_registered_suite
  ReadRegStr $1 HKCU "${SUITE_KEY}" "InstallDir"
  StrCmp $1 "" invalid_registered_suite
  StrCmp $1 $INSTDIR registered_path_ok invalid_registered_suite
registered_path_ok:
  StrCpy $2 '"$1\uninstall.exe"'
  StrCmp $0 $2 registered_uninstaller_ok invalid_registered_suite
registered_uninstaller_ok:
  IfFileExists "$1\elyoffice.product" registered_marker_file_ok invalid_registered_suite
registered_marker_file_ok:
  ClearErrors
  FileOpen $4 "$1\elyoffice.product" r
  IfErrors invalid_registered_suite
  FileRead $4 $3
  FileClose $4
  ${StrTrimNewLines} $3 "$3"
  StrCmp $3 "ElyOffice product marker v1" registered_marker_ok invalid_registered_suite
registered_marker_ok:
  IfFileExists "$1\payload.manifest.json" registered_manifest_ok invalid_registered_suite
registered_manifest_ok:
  StrCpy $INSTDIR $1
  StrCpy $ElySameSuite "1"
  Goto init_done
invalid_registered_suite:
  MessageBox MB_OK|MB_ICONSTOP "An ElyOffice uninstall registration exists but its install marker is missing. Remove or repair that registration before continuing."
  Abort
no_registered_suite:
  IfFileExists "$INSTDIR\elyoffice.product" unregistered_marker_conflict init_done
unregistered_marker_conflict:
  MessageBox MB_OK|MB_ICONSTOP "An ElyOffice marker exists in the default install directory without a matching per-user registration. The installer will not overwrite it."
  Abort
init_done:
FunctionEnd

!define MUI_PAGE_CUSTOMFUNCTION_LEAVE ElyDirectoryLeave
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetShellVarContext current
  StrCmp $ElySameSuite "1" install_payload_clear
  !include "${INSTALL_GUARDS}"
  Goto install_payload_clear
install_payload_conflict:
  MessageBox MB_OK|MB_ICONSTOP "The default per-user install directory already contains files that are not owned by a registered ElyOffice suite. The installer will not overwrite them."
  Abort
install_payload_clear:
  Call CheckInstallDirectoryWritable
  StrCmp $ElyProbeOK "1" install_directory_writable
  IfSilent install_directory_silent_failure
  MessageBox MB_OK|MB_ICONSTOP "ElyOffice cannot write to $INSTDIR. Choose a folder your account can write to, such as $LOCALAPPDATA\Programs\ElyOffice or D:\ElyOffice."
install_directory_silent_failure:
  SetErrorLevel 5
  Quit
install_directory_writable:
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\ElyOffice"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyDoc.lnk" "$INSTDIR\ely-doc.exe" "" "$INSTDIR\ely-doc.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyTable.lnk" "$INSTDIR\ely-table.exe" "" "$INSTDIR\ely-table.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyShow.lnk" "$INSTDIR\ely-show.exe" "" "$INSTDIR\ely-show.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyPDF.lnk" "$INSTDIR\ely-pdf.exe" "" "$INSTDIR\ely-pdf.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyMark.lnk" "$INSTDIR\ely-mark.exe" "" "$INSTDIR\ely-mark.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyTree.lnk" "$INSTDIR\ely-tree.exe" "" "$INSTDIR\ely-tree.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyMail.lnk" "$INSTDIR\ely-mail.exe" "" "$INSTDIR\ely-mail.ico"
  CreateShortcut "$SMPROGRAMS\ElyOffice\ElyAgent.lnk" "$INSTDIR\ely-agent.exe" "" "$INSTDIR\ely-agent.ico"

  WriteRegStr HKCU "${SUITE_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "ElyOffice"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "ZacharyZhang"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\ely-mark.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  IfFileExists "$INSTDIR\elyoffice.product" marker_ok
  MessageBox MB_OK|MB_ICONSTOP "The ElyOffice install marker is missing. The installer will not remove files from this directory."
  Abort
marker_ok:
  Delete "$INSTDIR\uninstall.exe"
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\payload.manifest.json"
  !include "${UNINSTALL_DIRS}"
  Delete "$SMPROGRAMS\ElyOffice\ElyDoc.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyTable.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyShow.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyPDF.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyMark.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyTree.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyMail.lnk"
  Delete "$SMPROGRAMS\ElyOffice\ElyAgent.lnk"
  RMDir "$SMPROGRAMS\ElyOffice"
  DeleteRegKey HKCU "${SUITE_KEY}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  RMDir "$INSTDIR"
SectionEnd
