// This is just a quick port of the original JS I had written--there's room for
// improvement

#[derive(Debug, Clone, PartialEq)]
pub struct FitRecord {
    // We use the same bitdepth, but not the same epoch
    pub seconds_since_unix_epoch: u32,
    // Wattage
    pub power: Option<u16>,
    // BPM
    pub heart_rate: Option<u8>,
    // RPM
    pub cadence: Option<u8>,
    // GPS Lat
    pub latitude: Option<f64>,
    // GPS Lon
    pub longitude: Option<f64>,
    // GPS Alitude
    pub altitude: Option<f32>,
    // Cumulative distance traveled in meters
    pub distance: Option<f64>,
    // Instantaneous speed in meters/s
    pub speed: Option<f32>,
}

fn make_header(length: usize) -> Vec<u8> {
    vec![
        // Header length
        12,
        // protocol version
        0x20,
        // profile version (little endian)
        0xeb,
        0x07,
        // number of bytes excluding header and checksum (little endian)
        length as u8 & 0xff,
        (length >> 8) as u8 & 0xff,
        (length >> 16) as u8 & 0xff,
        (length >> 24) as u8 & 0xff,
        // ASCI for .FIT
        0x2e,
        0x46,
        0x49,
        0x54,
    ]
}

fn record_to_bytes(record: &FitRecord) -> Vec<u8> {
    let ts = record.seconds_since_unix_epoch - 631065600;
    let mut bytes = vec![
        // Type 0 (we only ever define type 0, and just continually change the definition)
        0,
        // Time
        ts as u8 & 0xff,
        (ts >> 8) as u8 & 0xff,
        (ts >> 16) as u8 & 0xff,
        (ts >> 24) as u8 & 0xff,
    ];

    if let Some(l) = record.latitude {
        let x = i32::to_le_bytes((l * ((1 << 29) as f64) / 45.0) as i32);
        bytes.extend(&x);
    }

    if let Some(l) = record.longitude {
        let x = i32::to_le_bytes((l * ((1 << 29) as f64) / 45.0) as i32);
        bytes.extend(&x);
    }

    if let Some(a) = record.altitude {
        let x = u16::to_le_bytes((5.0 * (a + 500.0)) as u16);
        bytes.extend(&x);
    }

    if let Some(p) = record.power {
        bytes.push(p as u8 & 0xff);
        bytes.push((p >> 8) as u8 & 0xff);
    };

    if let Some(hr) = record.heart_rate {
        bytes.push(hr);
    }

    if let Some(c) = record.cadence {
        bytes.push(c);
    }

    if let Some(d) = record.distance {
        let x = u32::to_le_bytes((100.0 * d) as u32);
        bytes.extend(&x);
    }

    if let Some(s) = record.speed {
        let x = u16::to_le_bytes((1000.0 * s) as u16);
        bytes.extend(&x);
    }

    bytes
}

fn record_def(record: &FitRecord) -> Vec<u8> {
    let field_count = 1
        + if let Some(_) = record.latitude { 1 } else { 0 }
        + if let Some(_) = record.longitude { 1 } else { 0 }
        + if let Some(_) = record.altitude { 1 } else { 0 }
        + if let Some(_) = record.power { 1 } else { 0 }
        + if let Some(_) = record.heart_rate {
            1
        } else {
            0
        }
        + if let Some(_) = record.cadence { 1 } else { 0 }
        + if let Some(_) = record.distance { 1 } else { 0 }
        + if let Some(_) = record.speed { 1 } else { 0 };

    let mut bytes = vec![
        // Field definition for message type 0
        64,
        // Reserved
        0,
        // Little Endian
        0,
        // Global Message Number (20 is for a typical data record)
        20,
        0,
        // Number of fields
        field_count,
        // Timestamp (field definition number, byte count, default type (u32))
        253,
        4,
        0x86,
    ];

    let lat_def = vec![
        // Latitude (field definition number, byte count, default type (i32))
        0, 4, 0x85,
    ];
    let lon_def = vec![
        // Longitude (field definition number, byte count, default type (i32))
        1, 4, 0x85,
    ];
    let alt_def = vec![
        // Altitude (field definition number, byte count, default type (u16))
        2, 2, 0x84,
    ];
    let power_def = vec![
        // Power (field definition number, byte count, default type (u16))
        7, 2, 0x84,
    ];
    let hr_def = vec![
        // HeartRate (field definition number, byte count, default type (u8))
        3, 1, 2,
    ];
    let cadence_def = vec![
        // Cadence (field definition number, byte count, default type (u8))
        4, 1, 2,
    ];
    let distance_def = vec![
        // distance (field definition number, byte count, default type (u32))
        5, 4, 0x86,
    ];
    let speed_def = vec![
        // Speed (field definition number, byte count, default type (u16))
        6, 2, 0x84,
    ];

    if let Some(_) = record.latitude {
        bytes.extend(lat_def);
    };

    if let Some(_) = record.longitude {
        bytes.extend(lon_def);
    };

    if let Some(_) = record.altitude {
        bytes.extend(alt_def);
    };

    if let Some(_) = record.power {
        bytes.extend(power_def);
    };

    if let Some(_) = record.heart_rate {
        bytes.extend(hr_def);
    }

    if let Some(_) = record.cadence {
        bytes.extend(cadence_def);
    }

    if let Some(_) = record.distance {
        bytes.extend(distance_def);
    }

    if let Some(_) = record.speed {
        bytes.extend(speed_def);
    }

    bytes
}

fn calculate_crc(blob: &Vec<u8>) -> u16 {
    let crc_table = [
        0x0000, 0xcc01, 0xd801, 0x1400, 0xf001, 0x3c00, 0x2800, 0xe401, 0xa001, 0x6c00, 0x7800,
        0xb401, 0x5000, 0x9c01, 0x8801, 0x4400,
    ];

    let mut crc = 0;
    for i in 0..blob.len() {
        let byte = blob[i] as u16;
        let mut tmp = crc_table[(crc & 0xf) as usize];
        crc = (crc >> 4) & 0x0fff;
        crc = crc ^ tmp ^ crc_table[(byte & 0xf) as usize];
        tmp = crc_table[(crc & 0xf) as usize];
        crc = (crc >> 4) & 0x0fff;
        crc = crc ^ tmp ^ crc_table[((byte >> 4) & 0xf) as usize];
    }

    crc
}

fn to_file_inner(list: &Vec<FitRecord>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut last_def: Option<Vec<u8>> = None;

    for record in list {
        let new_def = record_def(record);
        match last_def {
            Some(ld) => {
                if ld != new_def {
                    last_def = Some(new_def.clone());
                    bytes.extend(new_def)
                } else {
                    last_def = Some(ld);
                }
            }
            None => {
                last_def = Some(new_def.clone());
                bytes.extend(new_def);
            }
        }

        bytes.extend(record_to_bytes(record));
    }

    bytes
}

pub fn to_file(list: &Vec<FitRecord>) -> Vec<u8> {
    let record_buffer = to_file_inner(list);
    let mut bytes = make_header(record_buffer.len());
    bytes.extend(record_buffer);
    let crc = calculate_crc(&bytes);
    bytes.extend(vec![(crc & 0xff) as u8, ((crc >> 8) as u8) & 0xff]);
    bytes
}

#[cfg(test)]
mod tests {
    use super::to_file;
    use super::FitRecord;

    #[test]
    fn to_file_for_empty_vec() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x00, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x36, 0xc1
            ),
            to_file(&Vec::new()),
        );
    }

    #[test]
    fn to_file_for_single_record() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x1b, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x04, 0xfd, 0x04, 0x86, 0x07, 0x02, 0x84, 0x03, 0x01, 0x02, 0x04,
                0x01, 0x02, 0x00, 0xe8, 0x98, 0xc9, 0x38, 0xb4, 0x00, 0x78, 0x5a, 0xe4, 0xc1
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: Some(180),
                heart_rate: Some(120),
                cadence: Some(90),
                latitude: None,
                longitude: None,
                altitude: None,
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_two_records_with_reused_definition() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x24, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x04, 0xfd, 0x04, 0x86, 0x07, 0x02, 0x84, 0x03, 0x01, 0x02, 0x04,
                0x01, 0x02, 0x00, 0xe8, 0x98, 0xc9, 0x38, 0xb4, 0x00, 0x78, 0x5a, 0x00, 0xe9, 0x98,
                0xc9, 0x38, 0xb5, 0x00, 0x79, 0x5b, 0x7b, 0x97
            ),
            to_file(&vec!(
                FitRecord {
                    seconds_since_unix_epoch: 1583801576,
                    power: Some(180),
                    heart_rate: Some(120),
                    cadence: Some(90),
                    latitude: None,
                    longitude: None,
                    altitude: None,
                    distance: None,
                    speed: None,
                },
                FitRecord {
                    seconds_since_unix_epoch: 1583801577,
                    power: Some(181),
                    heart_rate: Some(121),
                    cadence: Some(91),
                    latitude: None,
                    longitude: None,
                    altitude: None,
                    distance: None,
                    speed: None,
                }
            )),
        );
    }

    #[test]
    fn to_file_for_two_records_with_separate_definitions() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x3c, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x04, 0xfd, 0x04, 0x86, 0x07, 0x02, 0x84, 0x03, 0x01, 0x02, 0x04,
                0x01, 0x02, 0x00, 0xe8, 0x98, 0xc9, 0x38, 0xb4, 0x00, 0x78, 0x5a, //
                0x40, 0x00, 0x00, 0x14, 0x00, 0x04, 253, 0x04, 0x86, 0x00, 0x04, 0x85, 0x01, 0x04,
                0x85, 0x02, 0x02, 0x84, 0x00, 0xe9, 0x98, 0xc9, 0x38, 0x33, 0xab, 0x58, 0x20, 0xd3,
                0xc7, 0xe7, 0xa8, 0x5d, 0x0b, //
                176, 0x0b
            ),
            to_file(&vec!(
                FitRecord {
                    seconds_since_unix_epoch: 1583801576,
                    power: Some(180),
                    heart_rate: Some(120),
                    cadence: Some(90),
                    latitude: None,
                    longitude: None,
                    altitude: None,
                    distance: None,
                    speed: None,
                },
                FitRecord {
                    seconds_since_unix_epoch: 1583801577,
                    power: None,
                    heart_rate: None,
                    cadence: None,
                    latitude: Some(45.48707197420299),
                    longitude: Some(-122.4767913389951),
                    altitude: Some(81.79999999999995),
                    distance: None,
                    speed: None,
                }
            )),
        );
    }

    #[test]
    fn to_file_for_single_record_without_power() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x16, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x03, 0xfd, 0x04, 0x86, 0x03, 0x01, 0x02, 0x04, 0x01, 0x02, 0x00,
                0xe8, 0x98, 0xc9, 0x38, 0x78, 0x5a, 0x9b, 0x59
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: None,
                heart_rate: Some(120),
                cadence: Some(90),
                latitude: None,
                longitude: None,
                altitude: None,
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_single_record_without_heart_rate() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x17, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x03, 0xfd, 0x04, 0x86, 0x07, 0x02, 0x84, 0x04, 0x01, 0x02, 0x00,
                0xe8, 0x98, 0xc9, 0x38, 0xb4, 0x00, 0x5a, 0xf9, 0xbe
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: Some(180),
                heart_rate: None,
                cadence: Some(90),
                latitude: None,
                longitude: None,
                altitude: None,
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_single_record_without_cadence() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x17, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x03, 0xfd, 0x04, 0x86, 0x07, 0x02, 0x84, 0x03, 0x01, 0x02, 0x00,
                0xe8, 0x98, 0xc9, 0x38, 0xb4, 0x00, 0x78, 0x63, 0xd3
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: Some(180),
                heart_rate: Some(120),
                cadence: None,
                latitude: None,
                longitude: None,
                altitude: None,
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_lat_lon_alt() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x21, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x04, 253, 0x04, 0x86, 0x00, 0x04, 0x85, 0x01, 0x04, 0x85, 0x02,
                0x02, 0x84, 0x00, 0xe8, 0x98, 0xc9, 0x38, 0x33, 0xab, 0x58, 0x20, 0xd3, 0xc7, 0xe7,
                0xa8, 0x5d, 0x0b, 0x4d, 0xb6
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: None,
                heart_rate: None,
                cadence: None,
                latitude: Some(45.48707197420299),
                longitude: Some(-122.4767913389951),
                altitude: Some(81.79999999999995),
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_lat_lon_alt_cadence_hr_power() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x2e, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x07, 253, 0x04, 0x86, 0x00, 0x04, 0x85, 0x01, 0x04, 0x85, 0x02,
                0x02, 0x84, 0x07, 0x02, 0x84, 0x03, 0x01, 0x02, 0x04, 0x01, 0x02, 0, 0xe8, 0x98,
                0xc9, 0x38, 0x33, 0xab, 0x58, 0x20, 0xd3, 0xc7, 0xe7, 0xa8, 0x5d, 0x0b, 0xb5, 0x00,
                0x79, 0x5b, 0xe9, 0x1b
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: Some(181),
                heart_rate: Some(121),
                cadence: Some(91),
                latitude: Some(45.48707197420299),
                longitude: Some(-122.4767913389951),
                altitude: Some(81.79999999999995),
                distance: None,
                speed: None,
            })),
        );
    }

    #[test]
    fn to_file_for_speed_and_distance_only() {
        assert_eq!(
            vec!(
                0x0c, 0x20, 0xeb, 0x07, 0x1a, 0x00, 0x00, 0x00, 0x2e, 0x46, 0x49, 0x54, 0x40, 0x00,
                0x00, 0x14, 0x00, 0x03, 253, 0x04, 0x86, //
                0x05, 0x04, 0x86, // dist def
                0x06, 0x02, 0x84, // speed def
                0,    // record type
                0xe8, 0x98, 0xc9, 0x38, // time data
                0xa0, 0x86, 0x01, 0x00, // dist data
                0x70, 0x17, // speed data
                0xf3, 0x74 // crc
            ),
            to_file(&vec!(FitRecord {
                seconds_since_unix_epoch: 1583801576,
                power: None,
                heart_rate: None,
                cadence: None,
                latitude: None,
                longitude: None,
                altitude: None,
                distance: Some(1000.0), // 1km
                speed: Some(6.0),       // 21.6 km
            })),
        );
    }
}
