//! Generates the Kotlin bindings from the built library. Run by scripts/android.sh.

fn main() {
    uniffi::uniffi_bindgen_main()
}
