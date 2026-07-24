# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# Plugin system: these names are a runtime ABI for dynamically loaded DEX plugins.
-keep,allowoptimization interface ai.viewit.app.ViewItPlugin { *; }
-keep,allowoptimization interface ai.viewit.app.ViewItDocumentPlugin { *; }
-keep,allowoptimization interface ai.viewit.app.PluginProgress { *; }
-keep class * implements ai.viewit.app.ViewItPlugin { *; }
-keep class * implements ai.viewit.app.ViewItDocumentPlugin { *; }
-keep class ai.viewit.app.PluginManifest { *; }
-keep class ai.viewit.app.InstalledPlugin { *; }

# WebView JS bridge
-keepclassmembers class ai.viewit.app.MainActivity$AndroidBridge {
    @android.webkit.JavascriptInterface <methods>;
}
