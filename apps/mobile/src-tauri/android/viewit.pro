# EdDSAEngine checks this JDK-only key type behind a guarded compatibility branch.
-dontwarn sun.security.x509.X509Key

# These interfaces are implemented by separately packaged plugin DEX files.
# Their binary names and member names are a host/plugin ABI and must not be obfuscated.
-keep,allowoptimization interface ai.viewit.app.ViewItDocumentPlugin { *; }
-keep,allowoptimization interface ai.viewit.app.ViewItPlugin { *; }
-keep,allowoptimization interface ai.viewit.app.ArchivePlugin { *; }
-keep,allowoptimization interface ai.viewit.app.PluginProgress { *; }
