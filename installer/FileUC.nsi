; ==============================================================================
; FileUC - 1-Click Fast Windows Installer (NSIS)
; Modern User-Level Installation (No UAC / Admin required)
; ==============================================================================

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

; --- Product Details ---
!define PRODUCT_NAME "FileUC"
!define PRODUCT_VERSION "0.1.2"
!define PRODUCT_PUBLISHER "FileUC Team"
!define PRODUCT_WEB_SITE "https://github.com/Venkateshwar-T/fileuc"
!define PRODUCT_DIR_REGKEY "Software\Microsoft\Windows\CurrentVersion\App Paths\fileuc.exe"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKCU"

!ifndef ROOT_DIR
  !define ROOT_DIR ".."
!endif

Name "${PRODUCT_NAME}"
OutFile "${ROOT_DIR}\target\FileUC-Setup.exe"
InstallDir "$LOCALAPPDATA\FileUC"
InstallDirRegKey HKCU "${PRODUCT_DIR_REGKEY}" ""
RequestExecutionLevel user

; Compression
SetCompressor /SOLID lzma

; 1-Click Fast UI Configuration
!define MUI_ICON "${ROOT_DIR}\assets\icon.ico"
!define MUI_UNICON "${ROOT_DIR}\assets\icon.ico"

; Minimal fast installation window
ShowInstDetails nevershow
ShowUninstDetails nevershow
AutoCloseWindow true

; Pages: Only the single fast installation progress page
!insertmacro MUI_PAGE_INSTFILES

; Uninstaller Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; --- Installer Section ---
Section "MainSection" SEC01
    ; Gracefully stop running instance if updating
    nsExec::Exec 'taskkill /F /IM fileuc.exe'
    Sleep 500

    ; Set target directory and extract files
    SetOutPath "$INSTDIR"
    SetOverwrite on

    File "${ROOT_DIR}\target\release\fileuc.exe"
    File "${ROOT_DIR}\pdfium.dll"
    File "${ROOT_DIR}\assets\icon.ico"

    ; Create Shortcuts (Desktop & Start Menu)
    CreateDirectory "$SMPROGRAMS\FileUC"
    CreateShortcut "$SMPROGRAMS\FileUC\FileUC.lnk" "$INSTDIR\fileuc.exe" "" "$INSTDIR\icon.ico" 0
    CreateShortcut "$SMPROGRAMS\FileUC\Uninstall FileUC.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\icon.ico" 0
    CreateShortcut "$DESKTOP\FileUC.lnk" "$INSTDIR\fileuc.exe" "" "$INSTDIR\icon.ico" 0

    ; Create Uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Register in Windows Registry (Add/Remove Programs)
    WriteRegStr HKCU "${PRODUCT_DIR_REGKEY}" "" "$INSTDIR\fileuc.exe"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\icon.ico"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
    WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoRepair" 1

    ; Launch FileUC immediately upon installation
    Exec '"$INSTDIR\fileuc.exe"'
SectionEnd

; --- Uninstaller Section ---
Section Uninstall
    ; Terminate running FileUC instances
    nsExec::Exec 'taskkill /F /IM fileuc.exe'
    Sleep 500

    ; Remove shortcuts
    Delete "$DESKTOP\FileUC.lnk"
    Delete "$SMPROGRAMS\FileUC\FileUC.lnk"
    Delete "$SMPROGRAMS\FileUC\Uninstall FileUC.lnk"
    RMDir "$SMPROGRAMS\FileUC"

    ; Remove installed files and directories
    Delete "$INSTDIR\fileuc.exe"
    Delete "$INSTDIR\pdfium.dll"
    Delete "$INSTDIR\icon.ico"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir /r "$INSTDIR\bin"
    RMDir "$INSTDIR"

    ; Remove registry keys
    DeleteRegKey HKCU "${PRODUCT_UNINST_KEY}"
    DeleteRegKey HKCU "${PRODUCT_DIR_REGKEY}"
SectionEnd
