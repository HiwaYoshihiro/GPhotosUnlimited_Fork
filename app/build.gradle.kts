import java.io.File
import java.util.Properties

plugins {
    id("com.android.application")
}

// module.prop is the single source of truth for the module version. The APK's
// version fields used to be an unrelated 1/1.0, which made crash reports and
// artifacts impossible to match to a module release.
val moduleProperties = Properties().apply {
    rootProject.file("module/module.prop").inputStream().use { load(it) }
}
val moduleVersionName = moduleProperties.getProperty("version").removePrefix("v")
val moduleVersionCode = moduleProperties.getProperty("versionCode").toInt()

// verifyDex reads dexdump from the same build-tools release as buildToolsVersion
// below. The task resolves it across every SDK root (ANDROID_HOME,
// ANDROID_SDK_ROOT, sdk.dir), because AGP may pick a different root than the
// first environment variable that happens to be set.
val androidBuildToolsVersion = "37.0.0"

android {
    namespace = "com.rev4n.unlimitedphotos"
    compileSdk = 37
    ndkVersion = "30.0.16248370"
    buildToolsVersion = androidBuildToolsVersion

    buildFeatures {
        prefab = true
    }

    packaging {
        jniLibs {
            // Supplied by the platform at runtime; packaging it would shadow the
            // device's copy.
            excludes += "**/liblog.so"
        }
    }

    defaultConfig {
        applicationId = "com.rev4n.unlimitedphotos"
        minSdk = 26
        targetSdk = 37
        versionCode = moduleVersionCode
        versionName = moduleVersionName

        ndk {
            abiFilters += "armeabi-v7a"
            abiFilters += "arm64-v8a"
        }

        externalNativeBuild {
            cmake {
                arguments += "-DCMAKE_BUILD_TYPE=Release"
                // `-PfgoBuildStd=false` links the prebuilt std instead of the
                // size-optimised one built from rust-src (see CMakeLists.txt).
                providers.gradleProperty("fgoBuildStd").orNull?.let {
                    arguments += "-DFGP_BUILD_STD=${if (it.toBoolean()) "ON" else "OFF"}"
                }
                // No C++ is compiled any more (the native module is Rust and
                // shadowhook is C), so there is no STL to select. The previous
                // -fno-exceptions/-fno-rtti/-fvisibility flags moved into
                // Cargo.toml and the version script.
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }
}

dependencies {
    implementation("org.lsposed.hiddenapibypass:hiddenapibypass:6.2")
    // org.lsposed.libcxx:libcxx-full was only needed to link the C++ module with
    // -DANDROID_STL=none. Rust links against bionic directly.
    testImplementation("junit:junit:4.13.2")
}

tasks.register("copyFiles") {
    // Without these, `./gradlew zip` on a clean tree copied from paths that were
    // never built: it failed loudly when the dex was missing, but silently
    // produced a zip without the .so files when only the strip output was gone.
    dependsOn("minifyReleaseWithR8", "stripReleaseDebugSymbols")

    val moduleFolder = project.rootDir.resolve("module")
    val dexFile = project.layout.buildDirectory.get().asFile.resolve("intermediates/dex/release/minifyReleaseWithR8/classes.dex")
    val soDir = project.layout.buildDirectory.get().asFile.resolve("intermediates/stripped_native_libs/release/stripReleaseDebugSymbols/out/lib")

    doLast {
        // Clear outputs from earlier builds. A stale ABI or dex otherwise
        // survives when the current build stops producing it.
        val zygiskFolder = moduleFolder.resolve("zygisk")
        zygiskFolder.deleteRecursively()
        moduleFolder.resolve("classes.dex").delete()

        dexFile.copyTo(moduleFolder.resolve("classes.dex"), overwrite = true)

        soDir.walk().filter { it.isFile && it.extension == "so" }.forEach { soFile ->
            val abiFolder = soFile.parentFile.name
            val destination = zygiskFolder.resolve("$abiFolder.so")
            destination.parentFile.mkdirs()
            soFile.copyTo(destination, overwrite = true)
        }
    }
}

/**
 * R8 is allowed to rename or merge anything that is only reached through
 * reflection. CustomKeyStoreSpi is exactly that: the JCA framework instantiates
 * it by the name CustomProvider registers. A broken keep rule once produced an
 * empty abstract class and a dex that installed fine but failed at runtime, so
 * verify the shipped dex, not just the source rules.
 */
val verifyDex = tasks.register("verifyDex") {
    group = "verification"
    description = "Checks that R8 kept the classes the module loads reflectively."
    dependsOn("copyFiles")

    val moduleFolder = project.rootDir.resolve("module")
    val dexFile = moduleFolder.resolve("classes.dex")
    // AGP picks one of these roots; dexdump has to come from the one that
    // actually has the pinned build-tools revision, not simply the first env
    // var set. Otherwise ANDROID_HOME pointing at a second SDK made the task
    // fail with "dexdump not found" even though the build itself succeeded.
    val sdkDirs = listOfNotNull(
        System.getenv("ANDROID_HOME"),
        System.getenv("ANDROID_SDK_ROOT"),
        project.rootDir.resolve("local.properties")
            .takeIf { it.isFile }
            ?.let { propertiesFile ->
                Properties().apply {
                    propertiesFile.inputStream().use { load(it) }
                }.getProperty("sdk.dir")
            },
    ).map { File(it) }.filter { it.isDirectory }
    val dexdumpName = if (System.getProperty("os.name").lowercase().contains("win")) "dexdump.exe" else "dexdump"
    val dexdump = sdkDirs
        .map { it.resolve("build-tools/$androidBuildToolsVersion/$dexdumpName") }
        .firstOrNull { it.isFile }
        ?: sdkDirs
            .flatMap { sdk ->
                sdk.resolve("build-tools").listFiles()
                    ?.filter { it.isDirectory }
                    ?.sortedByDescending { it.name }
                    ?: emptyList()
            }
            .map { it.resolve(dexdumpName) }
            .firstOrNull { it.isFile }

    doLast {
        check(dexFile.isFile) { "classes.dex is missing; run assembleRelease first" }
        checkNotNull(dexdump) {
            "Android SDK/dexdump not found; set ANDROID_HOME/ANDROID_SDK_ROOT or sdk.dir in local.properties"
        }
        check(dexdump.isFile) { "dexdump not found at $dexdump" }

        // ProcessBuilder rather than providers.exec(): the latter reaches
        // through Project and is not configuration-cache safe.
        // `-d` (disassembly) is not needed: the checks below only look for the
        // class descriptor and method name lines, which plain dexdump prints.
        // The full dump was 3.3x larger for no extra information.
        val process = ProcessBuilder(dexdump.absolutePath, dexFile.absolutePath)
            .redirectErrorStream(true)
            .start()
        val dump = process.inputStream.bufferedReader().use { it.readText() }
        val exitCode = process.waitFor()
        check(exitCode == 0) { "dexdump failed with exit code $exitCode:\n$dump" }

        fun classSection(descriptor: String): String {
            val marker = "Class descriptor  : '$descriptor'"
            val start = dump.indexOf(marker)
            check(start >= 0) { "$descriptor is missing from classes.dex; R8 stripped it" }
            val next = dump.indexOf("Class descriptor  :", start + marker.length)
            return if (next < 0) dump.substring(start) else dump.substring(start, next)
        }

        val keyStoreSpi = classSection("Lcom/rev4n/unlimitedphotos/CustomKeyStoreSpi;")
        for (method in listOf("engineGetKey", "engineGetCertificateChain", "engineLoad", "engineStore")) {
            check(keyStoreSpi.contains("name          : '$method'")) {
                "CustomKeyStoreSpi.$method was stripped by R8"
            }
        }

        val provider = classSection("Lcom/rev4n/unlimitedphotos/CustomProvider;")
        check(provider.contains("name          : '<init>'")) { "CustomProvider constructor was stripped by R8" }
        check(provider.contains("name          : 'getService'")) { "CustomProvider.getService was stripped by R8" }

        classSection("Lcom/rev4n/unlimitedphotos/CustomPackageInfoCreator;")
        classSection("Lcom/rev4n/unlimitedphotos/CustomPackageManager;")

        logger.lifecycle("DEX smoke test passed: reflectively loaded classes and methods are present")
    }
}

tasks.register<Zip>("zip") {
    dependsOn("copyFiles", verifyDex)

    archiveFileName.set("GPhotosUnlimited.zip")
    destinationDirectory.set(project.rootDir.resolve("out"))

    from(project.rootDir.resolve("module"))
}

afterEvaluate {
    tasks["assembleRelease"].finalizedBy("copyFiles", verifyDex, "zip")
}
