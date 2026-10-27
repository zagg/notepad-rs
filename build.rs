fn main() {
    println!("cargo:rerun-if-changed=notepad.exe.manifest");
    // Embeds the same manifest as the original stub (Common-Controls v6, asInvoker, PerMonitorV2, longPathAware).
    embed_manifest::embed_manifest_file("notepad.exe.manifest").ok();
}
