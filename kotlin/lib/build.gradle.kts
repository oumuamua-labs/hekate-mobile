// SPDX-License-Identifier: Apache-2.0

plugins {
    kotlin("jvm")
    `maven-publish`
}

group = "dev.oumuamua.hekate"
version = "0.1.0"

kotlin {
    jvmToolchain(17)
    explicitApi()
}

dependencies {
    api("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")

    testImplementation(kotlin("test"))
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
}

tasks.test {
    useJUnitPlatform()
}

java {
    withSourcesJar()
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])
            artifactId = "hekate"
            pom {
                name.set("hekate")
                description.set("Async + cancellation wrappers for UniFFI-generated Hekate prover bindings.")
                licenses {
                    license {
                        name.set("Apache-2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0")
                    }
                }
            }
        }
    }
}