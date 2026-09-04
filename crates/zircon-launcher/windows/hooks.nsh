; Zircon Launcher - NSIS Installer & Uninstaller Hooks
; Executed during setup and uninstallation phases.

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Ask the user if they want to clean up local instance files and user data.
  MessageBox MB_YESNO|MB_ICONQUESTION "Would you like to remove all Zircon data and local instance files (including .zircon and .mcmanager in your user profile)?" IDNO skip_zircon_cleanup
  
  ; Remove .zircon (instances, configs)
  RMDir /r "$PROFILE\.zircon"
  
  ; Remove .mcmanager (auth cache, launcher metadata, skins, settings)
  RMDir /r "$PROFILE\.mcmanager"
  
  ; Remove LocalAppData launcher caches if present
  RMDir /r "$LOCALAPPDATA\com.zircon.launcher"
  RMDir /r "$APPDATA\com.zircon.launcher"

  skip_zircon_cleanup:
!macroend
