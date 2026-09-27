# Every development task, named by what you are trying to do. `make` alone lists them.
# The deskpress CLI is not wrapped here: `make install` puts it on PATH, then run it directly.

SHELL := sh
.SHELLFLAGS := -eu -c
.DEFAULT_GOAL := help

# Versions the repo builds with. Everything else is pinned where it lives: Rust in
# rust-toolchain.toml, crates in Cargo.lock, Gradle in its wrapper, the app's libraries in
# apps/android/gradle/libs.versions.toml, the build JDK in gradle-daemon-jvm.properties.
NDK := 30.0.16248370
PLATFORM := android-37.2
BUILD_TOOLS := 37.0.0
MAESTRO_VERSION := 2.10.0
ABIS := arm64-v8a x86_64
AVD ?= UE_pixel_6_API_35

ifeq ($(OS),Windows_NT)
  ANDROID_HOME ?= $(subst \,/,$(LOCALAPPDATA))/Android/Sdk
  BAT := .bat
else ifeq ($(shell uname),Darwin)
  ANDROID_HOME ?= $(HOME)/Library/Android/sdk
else
  ANDROID_HOME ?= $(HOME)/Android/Sdk
endif
export ANDROID_HOME
export ANDROID_NDK_HOME := $(ANDROID_HOME)/ndk/$(NDK)

APP := apps/android
SRC := $(APP)/app/src
GRADLE := cd $(APP) && ./gradlew --quiet
ADB := $(ANDROID_HOME)/platform-tools/adb
SDKMANAGER := $(ANDROID_HOME)/cmdline-tools/latest/bin/sdkmanager$(BAT)
MAESTRO_HOME := $(HOME)/.maestro/$(MAESTRO_VERSION)
MAESTRO := $(MAESTRO_HOME)/maestro/bin/maestro$(BAT)

.PHONY: help setup check test fmt lint coverage install bindings android-test apk run e2e \
	emulator android ci clean

help: ## List the tasks
	@grep -E '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-13s %s\n", $$1, $$2}'

# Setup

setup: $(MAESTRO) ## Install the Rust targets, cargo tools, Android SDK parts and Maestro
	rustup show active-toolchain
	rustup target add aarch64-linux-android x86_64-linux-android
	cargo install --locked cargo-ndk cargo-llvm-cov
# On Windows sdkmanager cannot replace its own folder: if cmdline-tools fails to update, copy
# cmdline-tools/latest aside, run that copy's sdkmanager once, and delete the copy.
	yes | "$(SDKMANAGER)" "platforms;$(PLATFORM)" "build-tools;$(BUILD_TOOLS)" "ndk;$(NDK)" \
		"cmdline-tools;latest" platform-tools emulator > /dev/null

# Maestro goes in a folder per version, checked against the release's checksum.
$(MAESTRO):
	mkdir -p "$(MAESTRO_HOME)"
	url=https://github.com/mobile-dev-inc/Maestro/releases/download/cli-$(MAESTRO_VERSION); \
	cd "$(MAESTRO_HOME)"; \
	curl -fsSL -o maestro.zip "$$url/maestro.zip"; \
	curl -fsSL "$$url/checksums_sha256.txt" | grep ' maestro.zip$$' | sha256sum -c -; \
	unzip -qo maestro.zip; \
	rm maestro.zip

# Engine

check: ## The gate: format, lint, tests at 100% coverage. Run it before handing anything over
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo llvm-cov --workspace --fail-under-lines 100 --fail-under-functions 100 \
		--fail-under-regions 100

test: ## Run the Rust tests, no coverage
	cargo test --workspace

fmt: ## Format the Rust code
	cargo fmt --all

lint: ## Run clippy as the gate does
	cargo clippy --workspace --all-targets -- -D warnings

coverage: ## Write the coverage report to target/llvm-cov/html, to find what a test misses
	cargo llvm-cov --workspace --html

install: ## Put the deskpress CLI on PATH
	cargo install --path crates/cli

# Android app

bindings: ## Build the engine for the app and the desk, and write its Kotlin bindings
	cargo ndk $(foreach abi,$(ABIS),-t $(abi)) -o $(SRC)/main/jniLibs build --release \
		-p deskpress-ffi
	cargo build -p deskpress-ffi --features cli
	cargo run -q -p deskpress-ffi --features cli --bin uniffi-bindgen -- generate --no-format \
		--library $(SRC)/main/jniLibs/x86_64/libdeskpress_ffi.so --language kotlin \
		--out-dir $(SRC)/generated/kotlin

android-test: bindings ## Run the app's JVM tests against the real engine
	$(GRADLE) testDebugUnitTest

apk: bindings ## Build the debug APK into apps/android/app/build/outputs/apk/debug
	$(GRADLE) assembleDebug

run: bindings ## Install the debug app on the running device or emulator and open it
	$(GRADLE) installDebug
	"$(ADB)" shell am start -n dev.deskpress.app/.MainActivity

e2e: bindings $(MAESTRO) ## Install the debug app and run the regression flows on the device
	$(GRADLE) installDebug
	"$(MAESTRO)" test $(APP)/flows

emulator: ## Start the emulator AVD (AVD=name to pick another) and wait until it boots
	nohup "$(ANDROID_HOME)/emulator/emulator" -avd $(AVD) > /dev/null 2>&1 &
	"$(ADB)" wait-for-device shell 'while [ "$$(getprop sys.boot_completed)" != 1 ]; do sleep 1; done'

android: bindings ## Run the app's tests and build the APK
	$(GRADLE) testDebugUnitTest assembleDebug

# Everything

ci: check android ## Everything a change has to pass: the gate, then the app

clean: ## Remove every build output, the generated bindings and the built libraries
	cargo clean
	rm -rf $(SRC)/generated $(SRC)/main/jniLibs $(APP)/app/build $(APP)/build $(APP)/.gradle
