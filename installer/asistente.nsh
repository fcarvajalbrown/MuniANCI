; asistente.nsh — ganchos del instalador NSIS para el sidecar del Asistente.
;
; Se engancha via bundle.windows.nsis.installerHooks (ver gui\tauri.asistente.conf.json).
; NSIS_HOOK_PREINSTALL corre ANTES de copiar archivos, que es el unico momento en que
; esto sirve.
;
; Por que existe: tauri-apps/tauri#15134 reporta que el instalador NSIS no reemplaza un
; ejecutable sin recursos de version -- y un binario de PyInstaller no los lleva -- y que
; si el sidecar esta corriendo durante la instalacion la copia falla en silencio. Ese
; reporte no tiene respuesta de un mantenedor y esta etiquetado ai-slop, asi que se toma
; como riesgo a comprobar y no como hecho; segun el propio reporte afecta a externalBin
; y no a bundle.resources, que es lo que este producto usa. Da igual: matar el proceso y
; borrar la carpeta anterior cuesta cuatro lineas y cubre las dos mitades del reporte sin
; depender de que el diagnostico sea correcto.
;
; taskkill se usa con /F y sin /T a proposito por proceso: llama-server es hijo del
; sidecar, pero si el host murio mal puede haber quedado huerfano, asi que se nombra
; aparte.

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Cerrando el Asistente si esta corriendo..."
  nsExec::Exec 'taskkill /F /IM munigpt-backend.exe'
  nsExec::Exec 'taskkill /F /IM llama-server.exe'
  Sleep 1000
  ; La carpeta del sidecar se reemplaza entera: un _internal a medio actualizar es peor
  ; que uno ausente, porque arranca y falla en la primera consulta.
  RMDir /r "$INSTDIR\backend"
!macroend
