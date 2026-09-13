# Keep the UniFFI-generated bindings and JNA intact (reflection / native names).
-keep class com.termoso.core.** { *; }
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-dontwarn java.awt.*
