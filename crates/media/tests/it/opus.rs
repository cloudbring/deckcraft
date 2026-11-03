use craft_opus::{Decoder, OpusHead};

fn head(channels: usize, streams: usize, coupled_streams: usize, mapping: Vec<u8>) -> OpusHead {
    OpusHead { channels, pre_skip: 0, input_sample_rate: 48000, output_gain: 0, mapping_family: 1, streams, coupled_streams, mapping }
}

#[test]
fn from_head_rejects_inconsistent_headers() {
    for (header, reason) in [
        (head(1, 0, 0, vec![0]), "zero streams"),
        (head(2, 1, 0, vec![0]), "short mapping"),
        (head(1, 1, 0, vec![1]), "mapping index out of range"),
        (head(1, 1, 2, vec![0]), "more coupled than streams"),
        (head(0, 1, 0, vec![]), "zero channels"),
    ] {
        assert!(Decoder::from_head(header, 48000).is_err(), "{reason}");
    }
}

#[test]
fn from_head_accepts_valid_headers() {
    for header in [head(1, 1, 0, vec![0]), head(2, 1, 1, vec![0, 1]), head(3, 2, 1, vec![0, 1, 2]), head(2, 1, 0, vec![0, 255])] {
        assert!(Decoder::from_head(header, 48000).is_ok());
    }
}

#[test]
fn fixture_pcm_matches_original_fork_within_transform_tolerance() {
    let golden_samples = [
        (0, 0x3c8d2685),
        (1, 0x3cdc2387),
        (119, 0x3d9acdba),
        (120, 0x3da62002),
        (311, 0xbdbacc26),
        (312, 0xbdb07e22),
        (959, 0xbdecadca),
        (960, 0xbde63851),
        (1023, 0x3d930c7d),
        (4095, 0xbd42c88a),
        (8191, 0x3da24f19),
        (12000, 0x3c9c7252),
        (16383, 0x3df4cd16),
        (23999, 0xbba73535),
    ];
    for (name, sample_count, final_bits) in [("tone.opus", 24000, 0xbba73535), ("clip-vp9.webm", 24648, 0x3aff846f)] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        let bytes = std::sync::Arc::new(std::fs::read(path).unwrap());
        let pcm = deckcraft_media::audio::decode(&bytes).unwrap();
        assert_eq!((pcm.rate, pcm.channels, pcm.samples.len()), (48000, 1, sample_count), "{name}");
        assert!(pcm.samples.iter().all(|sample| sample.is_finite()), "{name}");
        for (index, bits) in golden_samples.into_iter().chain([(sample_count - 1, final_bits)]) {
            let deviation = (pcm.samples[index] - f32::from_bits(bits)).abs();
            assert!(deviation <= 1e-6, "{name} sample {index}: deviation {deviation}");
        }
    }
}
