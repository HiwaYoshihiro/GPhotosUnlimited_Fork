plugins {
    id("com.android.application")
}

android {
    namespace = "com.rev4n.unlimitedphotos"
    compileSdk = 37
    ndkVersion = "30.0.16248370"
    buildToolsVersion = "37.0.0"

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
        versionCode = 1
        versionName = "1.0"

        ndk {
            abiFilters += "armeabi-v7a"
            abiFilters += "arm64-v8a"
        }

        externalNativeBuild {
            cmake {
                arguments += "-DCMAKE_BUILD_TYPE=Release"
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
}

tasks.register("copyFiles") {
    val moduleFolder = project.rootDir.resolve("module")
    val dexFile = project.layout.buildDirectory.get().asFile.resolve("intermediates/dex/release/minifyReleaseWithR8/classes.dex")
    val soDir = project.layout.buildDirectory.get().asFile.resolve("intermediates/stripped_native_libs/release/stripReleaseDebugSymbols/out/lib")

    doLast {
        dexFile.copyTo(moduleFolder.resolve("classes.dex"), overwrite = true)

        soDir.walk().filter { it.isFile && it.extension == "so" }.forEach { soFile ->
            val abiFolder = soFile.parentFile.name
            val destination = moduleFolder.resolve("zygisk/$abiFolder.so")
            soFile.copyTo(destination, overwrite = true)
        }
    }
}

tasks.register<Zip>("zip") {
    dependsOn("copyFiles")

    archiveFileName.set("GPhotosUnlimited.zip")
    destinationDirectory.set(project.rootDir.resolve("out"))

    from(project.rootDir.resolve("module"))
}

afterEvaluate {
    tasks["assembleRelease"].finalizedBy("copyFiles", "zip")
}
