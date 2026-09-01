// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

fn main() {
    dimidiumlabs_ui_build::build().expect("failed to compile shared UI assets");
    println!("cargo:rerun-if-changed=assets");
}
