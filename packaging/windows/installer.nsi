; Per-user installer; pass APP_NAME, APP_SLUG, VERSION, EXE, ICON, TYPES.
Unicode true
!include "MUI2.nsh"

!ifndef APP_NAME
  !error "pass -DAPP_NAME"
!endif
!ifndef APP_SLUG
  !error "pass -DAPP_SLUG"
!endif
!ifndef VERSION
  !error "pass -DVERSION"
!endif
!ifndef EXE
  !error "pass -DEXE"
!endif
!ifndef ICON
  !error "pass -DICON"
!endif
!ifndef TYPES
  !error "pass -DTYPES, from packaging/windows/types.sh"
!endif
!include "${TYPES}"
!ifndef OUTFILE
  !define OUTFILE "${APP_SLUG}-${VERSION}-setup.exe"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Ely Office ${APP_NAME}"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
Name "${APP_NAME}"
OutFile "${OUTFILE}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\Ely Office\${APP_NAME}"
InstallDirRegKey HKCU "Software\Ely Office\${APP_NAME}" "InstallDir"
SetCompressor /SOLID lzma

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

LangString DeleteData ${LANG_SIMPCHINESE} "同时删除 ${APP_NAME} 的缓存、账号与本地历史吗？你的文档不受影响。"
LangString DeleteData ${LANG_ENGLISH} "Also delete the cache, accounts and local history of ${APP_NAME}? Your documents stay."

Section "Install"
  SetOutPath "$INSTDIR"
  File "/oname=${APP_SLUG}.exe" "${EXE}"
  File "/oname=${APP_SLUG}.ico" "${ICON}"
!ifdef GIT_DIR
  SetOutPath "$INSTDIR\git"
  File /r "${GIT_DIR}\*.*"
!endif
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_SLUG}.exe" "" "$INSTDIR\${APP_SLUG}.ico"
  WriteRegStr HKCU "Software\Ely Office\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "ZacharyZhang"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://zacharyzhang.com"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_SLUG}.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  !insertmacro ElyTypesInstall
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_SLUG}.exe"
  Delete "$INSTDIR\${APP_SLUG}.ico"
  Delete "$INSTDIR\uninstall.exe"
!ifdef GIT_DIR
  RMDir /r "$INSTDIR\git"
!endif
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  DeleteRegKey HKCU "Software\Ely Office\${APP_NAME}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  !insertmacro ElyTypesUninstall
  MessageBox MB_YESNO|MB_DEFBUTTON2 "$(DeleteData)" IDNO done
  RMDir /r "$LOCALAPPDATA\ElyOffice\${APP_SLUG}"
done:
SectionEnd
