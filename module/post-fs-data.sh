# shellcheck shell=sh
MODPATH="${0%/*}"

# Google Photos must not be on the Magisk DenyList, or the module cannot see its
# processes. Fixing that automatically means silently rewriting a root setting
# the user configured, so it only happens when they explicitly opt in by
# creating this file. See the README for the manual command.
[ -f "$MODPATH/allow_denylist_fix" ] || exit 0

# post-fs-data.sh also runs under KernelSU/APatch, where there is no `magisk`
# binary. Don't emit errors there; the manual command in the README is the
# supported path for those managers.
command -v magisk >/dev/null 2>&1 || exit 0
magisk --denylist status >/dev/null 2>&1 || exit 0

magisk --denylist rm com.google.android.apps.photos >/dev/null 2>&1 || true
