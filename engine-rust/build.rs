fn main() {
    // MIT-licensed StormLib PKWARE DCL explode implementation.  It is only
    // used for the fifth compression method found in shipped MPQ maps.
    cc::Build::new()
        .file("third_party/pklib/explode.c")
        .file("third_party/pklib/crc32.c")
        .file("third_party/pklib/sc_shim.c")
        .include("third_party/pklib")
        .warnings(false)
        .compile("sc_pklib");
    println!("cargo:rerun-if-changed=third_party/pklib");
}
