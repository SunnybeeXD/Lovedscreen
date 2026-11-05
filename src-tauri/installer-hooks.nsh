!macro NSIS_HOOK_POSTINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="Lovedscreen"'
  nsExec::Exec 'netsh advfirewall firewall add rule name="Lovedscreen" dir=in action=allow program="$INSTDIR\uxplay\uxplay.exe" profile=private enable=yes'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="Lovedscreen"'
!macroend
