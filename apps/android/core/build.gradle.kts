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
 * Always runs (cargo is incremental itself and tracks the whole workspace); debug
 * libraries are stripped of DWARF with the NDK's llvm-strip so the APK stays small.
 */
abstract class CargoNdkBuild @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val release: Property<Boolean>
    @get:Input @get:Optional abstract val ndkHome: Property<String>
    @get:Internal abstract val workspace: DirectoryProperty
    @get:OutputDirectory abstract val outDir: DirectoryProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun run() {
        val args = mutableListOf("cargo", "ndk")
        abis.get().forEach { args += listOf("-t", it) }
        args += listOf("-o", outDir.get().asFile.absolutePath, "build", "-p", "termoso-mobile")
        if (release.get()) args += "--release"
        exec.exec {
            workingDir = workspace.get().asFile
            ndkHome.orNull?.let { environment("ANDROID_NDK_HOME", it) }
            commandLine(args)
        }
        if (release.get()) return
        val strip = ndkHome.orNull
            ?.let { File(it, "toolchains/llvm/prebuilt").listFiles()?.firstOrNull() }
            ?.let { File(it, "bin/llvm-strip") }
            ?.takeIf { it.canExecute() }
            ?: return
        outDir.get().asFile.walkTopDown().filter { it.isFile && it.extension == "so" }.forEach { so ->
            exec.exec { commandLine(strip.absolutePath, "--strip-debug", so.absolutePath) }
        }
    }
}

/** Kotlin bindings from the compiled library's UniFFI metadata (any ABI works); crates/termoso-mobile/uniffi.toml sets the package. */
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
        }
        val bindgen = tasks.register<UniffiBindgen>("uniffiBindgen$cap") {
            group = "rust"
            description = "Generate Kotlin bindings for termoso-mobile (${variant.name})"
            workspace.set(workspaceDir)
            library.set(cargo.flatMap { it.outDir.file("${rustAbis.first()}/libtermoso_mobile.so") })
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
