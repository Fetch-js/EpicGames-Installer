fn main() {
    tauri_build::build();

    cc::Build::new()
        .cpp(true)
        .file("native/installer.cpp")
        .flag_if_supported("/std:c++20")
        .flag_if_supported("-std=c++20")
        .warnings(true)
        .compile("native_installer");
}
