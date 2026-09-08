//! Generate or verify fixed preset assets with the production pipeline and color transform.
use image::{DynamicImage, ImageBuffer, Rgb, codecs::jpeg::JpegEncoder, imageops::FilterType};
use omalux::{
    develop::{DevelopPipeline, DevelopRenderContext},
    io::{
        DecodeOptions, ResourceLimits, SdrRangePolicy, SignalRelation,
        color::WorkingToSrgbTransform,
    },
    job::{CancellationToken, PhotoDecoder, ProductionPhotoDecoder},
    preset::{PresetCatalog, catalog::BUILTIN_PRESETS},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::Path};

type Rgb16Image = ImageBuffer<Rgb<u16>, Vec<u16>>;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Reference {
    version: u32,
    width: u32,
    height: u32,
    pixel_format: String,
    pixels_sha256: String,
}

impl Reference {
    fn from_image(image: &Rgb16Image) -> Self {
        let mut hash = Sha256::new();
        for sample in image.as_raw() {
            hash.update(sample.to_be_bytes());
        }
        Self {
            version: 1,
            width: image.width(),
            height: image.height(),
            pixel_format: "srgb-rgb16be".into(),
            pixels_sha256: format!("{:x}", hash.finalize()),
        }
    }
}

fn thumbnail_bytes(image: &Rgb16Image) -> Result<Vec<u8>, Box<dyn Error>> {
    let small = DynamicImage::ImageRgb16(image.clone())
        .resize(288, 288, FilterType::Lanczos3)
        .to_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 88).encode_image(&small)?;
    Ok(bytes)
}

fn save(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("missing parent")?)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let write = match arguments.next().as_deref() {
        Some("--write") => true,
        Some("--check") => false,
        _ => return Err("usage: cargo run --release --example preset_references -- (--write|--check) [preset-id ...]".into()),
    };
    let requested: Vec<String> = arguments.collect();
    let catalog = PresetCatalog::built_in()?;
    for id in &requested {
        if catalog.get(id).is_none() {
            return Err(format!("unknown preset: {id}").into());
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let options = DecodeOptions::default();
    let photo = ProductionPhotoDecoder::new()
        .decode_path_once(
            &root.join("reference pictures/main.jpg"),
            &options,
            &CancellationToken::new(),
        )?
        .photo;
    if photo.signal_relation() == SignalRelation::SceneRelatedRaw {
        return Err("the reference must be a display-referred raster".into());
    }
    let context = DevelopRenderContext::from_source_digest(*photo.source_digest().as_bytes());
    let limits = ResourceLimits::default();
    let transform = WorkingToSrgbTransform::new(&limits)?;
    let mut failures = 0;
    for source in BUILTIN_PRESETS {
        let id = source
            .directory
            .rsplit('/')
            .next()
            .ok_or("missing preset ID")?;
        let preset = catalog.get(id).ok_or("preset folder must match its ID")?;
        if !requested.is_empty() && !requested.iter().any(|wanted| wanted == id) {
            continue;
        }
        let mut developed = photo.image().clone();
        DevelopPipeline.process_bounded_with_context(
            &mut developed,
            &preset.settings,
            Some(&context),
            &limits,
        )?;
        let mut pixels = Vec::with_capacity(developed.pixels().len() * 3);
        let mut row = vec![[0.0; 4]; developed.width() as usize];
        for input in developed.pixels().chunks(developed.width() as usize) {
            transform.transform_scanline(
                input,
                &mut row,
                photo.signal_relation(),
                SdrRangePolicy::SoftenAndReport,
                &limits,
            )?;
            for rgba in &row {
                pixels.extend(
                    rgba[..3]
                        .iter()
                        .map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16),
                );
            }
        }
        let reference = Rgb16Image::from_raw(developed.width(), developed.height(), pixels)
            .ok_or("invalid reference dimensions")?;
        let directory = root.join("presets/builtin").join(source.directory);
        let reference_path = directory.join("reference.json");
        let thumbnail_path = directory.join("thumbnail.jpg");
        let fingerprint = Reference::from_image(&reference);
        let thumbnail = thumbnail_bytes(&reference)?;
        if write {
            let json = serde_json::to_string_pretty(&fingerprint)? + "\n";
            save(&reference_path, json.as_bytes())?;
            save(&thumbnail_path, &thumbnail)?;
            println!("WROTE {id}");
        } else {
            // Hash full-resolution RGB samples, independent of image file encoding.
            let stored: Reference = serde_json::from_slice(&fs::read(&reference_path)?)?;
            let exact = stored == fingerprint;
            let thumbnail_exact = fs::read(&thumbnail_path)? == thumbnail;
            if exact && thumbnail_exact {
                println!("OK {id}");
            } else {
                failures += 1;
                eprintln!(
                    "DIFF {id}: reference pixels match={exact}, thumbnail matches={thumbnail_exact}"
                );
            }
        }
    }
    if failures > 0 {
        return Err(format!("{failures} preset references differ; nothing was overwritten").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_uses_big_endian_samples_and_preserves_low_bits() {
        let mut image = Rgb16Image::from_raw(1, 1, vec![0, 1, 65535]).unwrap();
        let original = Reference::from_image(&image);
        assert_eq!(
            original.pixels_sha256,
            "00c181a241824f575fb14dd09ab4cf12c6db078ece2d59871bc92b2661bba637"
        );
        image.put_pixel(0, 0, Rgb([0, 0, 65535]));
        assert_ne!(
            Reference::from_image(&image).pixels_sha256,
            original.pixels_sha256
        );
    }

    #[test]
    fn dimensions_are_part_of_the_reference_identity() {
        let wide = Rgb16Image::from_raw(2, 1, vec![0; 6]).unwrap();
        let tall = Rgb16Image::from_raw(1, 2, vec![0; 6]).unwrap();
        assert_ne!(Reference::from_image(&wide), Reference::from_image(&tall));
    }
}
