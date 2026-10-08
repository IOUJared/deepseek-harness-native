//! Opt-in synthetic-window snapshots. Never reads the desktop or overwrites an existing file.
use rustix::fs::{Mode, OFlags, openat};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Component, Path},
};

type Error = Box<dyn std::error::Error>;

pub fn validate(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || path.extension().is_none_or(|extension| extension != "png") {
        return Err(
            "--snapshot requires an absolute new .png path inside this Slint package".into(),
        );
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("--snapshot path may not contain parent-directory components".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    let parent = path
        .parent()
        .ok_or("snapshot path has no parent directory")?;
    if !parent.canonicalize()?.starts_with(root) {
        return Err("--snapshot parent must be inside this Slint package".into());
    }
    // Check all ancestors, not just the output file. openat below repeats this as
    // NOFOLLOW descriptor traversal, preventing ancestor-replacement symlink races.
    for ancestor in parent.ancestors() {
        if std::fs::symlink_metadata(ancestor)?
            .file_type()
            .is_symlink()
        {
            return Err("--snapshot refuses symlink ancestors".into());
        }
    }
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err("--snapshot refuses an existing output path, including symlinks".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn create_new(path: &Path) -> Result<File, Error> {
    validate(path)?;
    let directory_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = openat(rustix::fs::CWD, "/", directory_flags, Mode::empty())?;
    for component in path
        .parent()
        .ok_or("snapshot path has no parent")?
        .components()
    {
        if let Component::Normal(name) = component {
            directory = openat(&directory, name, directory_flags, Mode::empty())?;
        }
    }
    let file = openat(
        &directory,
        path.file_name().ok_or("snapshot path has no filename")?,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?;
    Ok(File::from(file))
}

pub fn write_rgba(
    path: &Path,
    pixels: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
) -> Result<(), Error> {
    if pixels.width() == 0 || pixels.height() == 0 {
        return Err("snapshot has zero dimensions".into());
    }
    let mut output = BufWriter::new(create_new(path)?);
    let mut encoder = png::Encoder::new(&mut output, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels.as_bytes())?;
    writer.finish()?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_unsafe_paths_before_any_output() {
        assert!(validate(Path::new("relative.png")).is_err());
        assert!(validate(Path::new("/tmp/outside-slints-package.png")).is_err());
        assert!(validate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../outside.png")).is_err());
        assert!(validate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("not-a-png.txt")).is_err());
    }
    #[test]
    fn encodes_png_and_refuses_overwrite_and_symlink_ancestors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        let name = format!(
            "snapshot-policy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let fixture = root.join("target").join(name);
        std::fs::create_dir(&fixture).unwrap();
        let path = fixture.join("synthetic.png");
        let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &[25_u8, 23, 36, 255, 224, 222, 244, 255],
            2,
            1,
        );
        write_rgba(&path, &pixels).unwrap();
        assert!(write_rgba(&path, &pixels).is_err());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&decoded[..info.buffer_size()], pixels.as_bytes());
        let link = fixture.join("link");
        std::os::unix::fs::symlink(&fixture, &link).unwrap();
        assert!(validate(&link.join("forbidden.png")).is_err());
        assert!(create_new(&link.join("forbidden.png")).is_err());
        let output_link = fixture.join("existing-link.png");
        std::os::unix::fs::symlink(&path, &output_link).unwrap();
        assert!(create_new(&output_link).is_err());
        // Verify the absolute cleanup target is our own newly created fixture.
        assert_eq!(fixture.canonicalize().unwrap(), fixture);
        assert_eq!(fixture.parent().unwrap(), root.join("target"));
        assert!(
            fixture
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("snapshot-policy-")
        );
        std::fs::remove_dir_all(fixture).unwrap();
    }
}
