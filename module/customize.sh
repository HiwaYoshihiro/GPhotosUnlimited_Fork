OLD_MODPATH=/data/adb/modules/${MODID:-unlimitedphotos}

# Older versions hid conflicting custom ROM injection apps by creating files
# under `$MODPATH/system`. That changed the mount namespace of every app on the
# device, not just Google Photos, so the module no longer manages a system tree
# at all. Drop the overrides instead of carrying them into the new version.
if [ -d "$OLD_MODPATH/system" ]; then
    ui_print "- Removing system-wide overrides left by an older version"
    rm -rf "$OLD_MODPATH/system"
    ui_print "! A reboot is required for the removal to take effect"
fi

# The new package must not contain a system tree either.
if [ -d "$MODPATH/system" ]; then
    rm -rf "$MODPATH/system"
fi

# Copy any supported custom files to updated module. Skip when the installer
# updates in place, where source and destination are the same directory.
if [ "$OLD_MODPATH" != "$MODPATH" ]; then
    for FILE in custom.fgp.json custom.fgp.prop uninstall.sh; do
        if [ -f "$OLD_MODPATH/$FILE" ]; then
            ui_print "- Restoring $FILE"
            cp -af "$OLD_MODPATH/$FILE" "$MODPATH/$FILE"
        fi
    done
fi

# The ROM app hiding feature is gone. Leave the user's list in place rather than
# deleting their file, but make it clear that nothing reads it any more.
if [ -f "$OLD_MODPATH/custom.app_replace_list.txt" ]; then
    ui_print "! custom.app_replace_list.txt is no longer used and can be deleted"
fi

# Warn if potentially conflicting modules are installed
for MODULE in Pixelify PixelifyPhotos unlimitedgpics; do
    if [ -d /data/adb/modules/$MODULE ]; then
        ui_print "! $MODULE module may cause issues with this module"
    fi
done
