use tsclientlib::Version;

pub fn ts6_compatible_client_version() -> Version {
    Version::Custom {
        platform: "Windows".to_string(),
        version: "3.6.2 [Build: 1695203293]".to_string(),
        signature: vec![
            224, 23, 90, 102, 151, 96, 81, 35, 2, 184, 139, 60, 169, 201, 104, 36, 243, 113, 54,
            82, 120, 163, 180, 10, 159, 19, 2, 68, 238, 180, 153, 35, 147, 180, 150, 114, 42, 51,
            171, 24, 176, 38, 120, 1, 45, 44, 130, 99, 114, 57, 157, 74, 156, 156, 49, 180, 14, 33,
            95, 118, 43, 107, 215, 3,
        ],
    }
}
