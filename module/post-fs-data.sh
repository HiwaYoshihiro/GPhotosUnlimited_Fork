MODPATH="${0%/*}"

# Google Photos must not be on the Magisk DenyList, or the module cannot see its
# processes. Fixing that automatically means silently rewriting a root setting
# the user configured, so it only happens when they explicitly opt in by
# creating this file. See the README for the manual command.
if [ -f "$MODPATH/allow_denylist_fix" ] && magisk --denylist status; then
    magisk --denylist rm com.google.android.apps.photos
fi
