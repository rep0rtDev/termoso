import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
}

// Cargo workspace root: apps/android/core -> ../../..
val workspaceDir: File = rootDir.parentFile.parentFile
val rustAbis: List<String> = (findProperty("termoso.abis") as String)
    .split(',')
    .map { it.trim() }
    .filter { it.isNotEmpty() }
val rustNdkHome: String? = System.getenv("ANDROID_NDK_HOME")
    ?: System.getenv("ANDROID_NDK_ROOT")
    ?: (System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT"))
        ?.let { sdk -> File(sdk, "ndk").listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }?.absolutePath }

android {
    namespace = "com.termoso.core"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlin {
        jvmToolchain(17)
    }
}

/**
 * `cargo ndk build` of crates/termoso-mobile for every ABI in `termoso.abis`.
 * Always runs (cargo is incremental itself and tracks the whole workspace). An unstripped
 * copy of the first ABI's library is kept for uniffi-bindgen (it reads the UniFFI metadata
 * from the symbol table); the packaged libraries are stripped with the NDK's llvm-strip —
 * DWARF only for debug, everything for release — so the APK stays small.
 */
abstract class CargoNdkBuild @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val release: Property<Boolean>
    @get:Input @get:Optional abstract val ndkHome: Property<String>
    @get:Internal abstract val workspace: DirectoryProperty
    @get:OutputDirectory abstract val outDir: DirectoryProperty
    @get:OutputFile abstract val metaLibrary: RegularFileProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun run() {
        val args = mutableListOf("cargo", "ndk")
        abis.get().forEach { args += listOf("-t", it) }
        args += listOf("-o", outDir.get().asFile.absolutePath, "build", "-p", "termoso-mobile")
        // The workspace release profile strips the symbol table, which also drops the UniFFI metadata.
        if (release.get()) args += listOf("--release", "--config", "profile.release.strip=\"debuginfo\"")
        exec.exec {
            workingDir = workspace.get().asFile
            ndkHome.orNull?.let { environment("ANDROID_NDK_HOME", it) }
            commandLine(args)
        }
        val libs = outDir.get().asFile.walkTopDown().filter { it.isFile && it.extension == "so" }.toList()
        val meta = metaLibrary.get().asFile
        meta.parentFile.mkdirs()
        libs.first { it.parentFile.name == abis.get().first() }.copyTo(meta, overwrite = true)
        val strip = ndkHome.orNull
            ?.let { File(it, "toolchains/llvm/prebuilt").listFiles()?.firstOrNull() }
            ?.let { File(it, "bin/llvm-strip") }
            ?.takeIf { it.canExecute() }
            ?: return
        val mode = if (release.get()) "--strip-all" else "--strip-debug"
        libs.forEach { so -> exec.exec { commandLine(strip.absolutePath, mode, so.absolutePath) } }
    }
}

/** Kotlin bindings from the compiled library's UniFFI metadata; crates/termoso-mobile/uniffi.toml sets the package. */
abstract class UniffiBindgen @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Internal abstract val workspace: DirectoryProperty
    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val library: RegularFileProperty
    @get:OutputDirectory abstract val outDir: DirectoryProperty

    @TaskAction
    fun run() {
        outDir.get().asFile.deleteRecursively()
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(
                "cargo", "run", "-q", "-p", "uniffi-bindgen", "--",
                "generate", "--library", library.get().asFile.absolutePath,
                "--language", "kotlin", "--no-format",
                "--out-dir", outDir.get().asFile.absolutePath,
            )
        }
    }
}

androidComponents {
    onVariants { variant ->
        val isRelease = variant.buildType == "release"
        val cap = variant.name.replaceFirstChar { it.uppercase() }
        val cargo = tasks.register<CargoNdkBuild>("cargoNdkBuild$cap") {
            group = "rust"
            description = "cargo ndk build of termoso-mobile (${variant.name})"
            abis.set(rustAbis)
            release.set(isRelease)
            ndkHome.set(rustNdkHome)
            workspace.set(workspaceDir)
            outDir.set(layout.buildDirectory.dir("rust/${variant.name}/jniLibs"))
            metaLibrary.set(layout.buildDirectory.file("rust/${variant.name}/meta/libtermoso_mobile.so"))
        }
        val bindgen = tasks.register<UniffiBindgen>("uniffiBindgen$cap") {
            group = "rust"
            description = "Generate Kotlin bindings for termoso-mobile (${variant.name})"
            workspace.set(workspaceDir)
            library.set(cargo.flatMap { it.metaLibrary })
            outDir.set(layout.buildDirectory.dir("rust/${variant.name}/kotlin"))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdkBuild::outDir)
        variant.sources.java?.addGeneratedSourceDirectory(bindgen, UniffiBindgen::outDir)
    }
}

dependencies {
    api(libs.jna) { artifact { type = "aar" } }
    implementation(libs.androidx.annotation)
}
