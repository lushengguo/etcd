fn main() {
    // 不再需要编译 protobuf
    println!("cargo:rerun-if-changed=src");
}
