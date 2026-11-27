Unicode true
!include "MUI2.nsh"
!ifndef VERSION
!error "Define VERSION"
!endif
!ifndef PAYLOAD
!error "Define PAYLOAD"
!endif
!ifndef OUTPUT
!error "Define OUTPUT"
!endif
!define APP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DZIsland"
Name "DZ Island"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\DZ Island"
InstallDirRegKey HKCU "${APP_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1046 "ProductName" "DZ Island"
VIAddVersionKey /LANG=1046 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1046 "FileDescription" "Instalador DZ Island"
VIAddVersionKey /LANG=1046 "FileVersion" "${VERSION}"
!define MUI_ICON "src-tauri\icons\icon.ico"
!define MUI_UNICON "src-tauri\icons\icon.ico"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\LICENSE.txt"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

Section "DZ Island"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "${PAYLOAD}\DZ-Island.exe"
  File "${PAYLOAD}\DZUpdater.exe"
  File "${PAYLOAD}\WebView2Loader.dll"
  File "${PAYLOAD}\LICENSE.txt"
  File "${PAYLOAD}\LEIA-ME.txt"
  File "${PAYLOAD}\Perguntar-Na-Ilha.ps1"
  File "${PAYLOAD}\PERGUNTAS-NA-ILHA.txt"
  SetOutPath "$INSTDIR\Battery"
  File "${PAYLOAD}\Battery\BatteryEngine.exe"
  File "${PAYLOAD}\Battery\headsetcontrol.exe"
  File "${PAYLOAD}\Battery\LICENSE.txt"
  File "${PAYLOAD}\Battery\THIRD_PARTY_NOTICES.txt"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$SMPROGRAMS\DZ Island.lnk" "$INSTDIR\DZ-Island.exe"
  WriteRegStr HKCU "${APP_KEY}" "DisplayName" "DZ Island"
  WriteRegStr HKCU "${APP_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${APP_KEY}" "Publisher" "DZ Island"
  WriteRegStr HKCU "${APP_KEY}" "DisplayIcon" "$INSTDIR\DZ-Island.exe"
  WriteRegStr HKCU "${APP_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${APP_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${APP_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${APP_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\DZ Island.lnk"
  Delete "$INSTDIR\DZ-Island.exe"
  Delete "$INSTDIR\DZUpdater.exe"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\LEIA-ME.txt"
  Delete "$INSTDIR\Perguntar-Na-Ilha.ps1"
  Delete "$INSTDIR\PERGUNTAS-NA-ILHA.txt"
  Delete "$INSTDIR\Battery\BatteryEngine.exe"
  Delete "$INSTDIR\Battery\headsetcontrol.exe"
  Delete "$INSTDIR\Battery\LICENSE.txt"
  Delete "$INSTDIR\Battery\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR\Battery"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${APP_KEY}"
SectionEnd
