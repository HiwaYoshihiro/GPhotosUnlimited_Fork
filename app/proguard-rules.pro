# EntryPoint is loaded by name from the dex that the Zygisk module injects.
-keep class com.rev4n.unlimitedphotos.EntryPoint { public <methods>; }

# CustomProvider registers "KeyStore.AndroidKeyStore" with the class name of
# CustomKeyStoreSpi, and the JCA framework instantiates that class and calls its
# engine* methods reflectively. `-keepclassmembers` with no member pattern kept
# nothing, so R8 replaced the class with an empty abstract one and every
# AndroidKeyStore operation failed at runtime.
-keep class com.rev4n.unlimitedphotos.CustomKeyStoreSpi { *; }

# Installed through Security.insertProviderAt(); Provider.Service lookups reach
# its overrides through the Provider base class.
-keep class com.rev4n.unlimitedphotos.CustomProvider { *; }

# Assigned to PackageInfo.CREATOR and invoked through Parcelable.Creator.
-keep class com.rev4n.unlimitedphotos.CustomPackageInfoCreator { *; }

# Used as the InvocationHandler of a dynamic Proxy over IPackageManager.
-keep class com.rev4n.unlimitedphotos.CustomPackageManager { *; }
