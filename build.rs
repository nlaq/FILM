fn main() {
    let mut config = slint_build::CompilerConfiguration::new();
    config = config.with_style("cupertino".to_string());
    // Esto compila el archivo completo y exportará tanto AppWindow como NewLensWindow
    slint_build::compile_with_config("ui/Main.slint", config).unwrap();
}