fn main() {
    prost_build::compile_protos(&["proto/telemetry.proto"], &["proto/"]).unwrap();
}
