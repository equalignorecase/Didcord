//! Local media compression before upload so oversized images and videos fit account limits.
use discord_api::upload::Source;
use image::{
	DynamicImage, ImageDecoder, ImageFormat, ImageReader,
	codecs::{jpeg::JpegEncoder, webp::WebPEncoder},
	imageops::FilterType,
};
use std::{
	fs,
	io::Cursor,
	path::{Path, PathBuf},
	process::{Command, Stdio},
	sync::atomic::{AtomicBool, Ordering},
	time::{Duration, Instant},
};

/// Standard free Discord accounts often enforce 10 MiB uploads; aim safely underneath it.
const FREE_TARGET_BYTES: u64 = 10 * 1024 * 1024 - 128 * 1024;
const FREE_REPORTED_LIMIT: u64 = 20 * 1024 * 1024;
const IMAGE_INPUT_CAP: u64 = 100 * 1024 * 1024;
const VIDEO_INPUT_CAP: u64 = 500 * 1024 * 1024;
const IMAGE_MAX_DIMENSION: u32 = 16_384;
const IMAGE_DECODE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

pub(crate) fn effective_target(upload_limit: u64) -> u64 {
	if upload_limit <= FREE_REPORTED_LIMIT {
		FREE_TARGET_BYTES.min(upload_limit.saturating_sub(128 * 1024))
	} else {
		upload_limit.saturating_sub(256 * 1024)
	}
}

pub(crate) async fn compress_if_needed(
	source: Source,
	target_bytes: u64,
	cancelled: &AtomicBool,
) -> Source {
	if source.size() <= target_bytes || target_bytes < 1024 || cancelled.load(Ordering::Acquire) {
		return source;
	}
	match classify(source.filename()) {
		Some(Kind::Image) => compress_image(source, target_bytes, cancelled).await,
		Some(Kind::Video) => compress_video(source, target_bytes, cancelled).await,
		None => source,
	}
}

#[derive(Clone, Copy)]
enum Kind {
	Image,
	Video,
}

fn classify(filename: &str) -> Option<Kind> {
	let ext = Path::new(filename)
		.extension()
		.and_then(|e| e.to_str())?
		.to_ascii_lowercase();
	match ext.as_str() {
		// Leave GIF untouched so animated frames are never flattened to a still frame.
		"png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "heif" => {
			Some(Kind::Image)
		}
		"mp4" | "mov" | "webm" | "mkv" | "avi" | "m4v" => Some(Kind::Video),
		_ => None,
	}
}

async fn compress_image(source: Source, target_bytes: u64, cancelled: &AtomicBool) -> Source {
	let Some(raw) = source.preview_bytes(IMAGE_INPUT_CAP).await else {
		return source;
	};
	if cancelled.load(Ordering::Acquire) {
		return source;
	}
	let original_name = source.filename().to_owned();
	let original_size = source.size();
	let compressed = tokio::task::spawn_blocking(move || {
		encode_image_under_budget(&raw, &original_name, target_bytes)
	})
	.await
	.ok()
	.flatten();
	let Some((new_name, bytes)) = compressed else {
		return source;
	};
	if (bytes.len() as u64) >= original_size || (bytes.len() as u64) > target_bytes {
		return source;
	}
	Source::compressed_bytes(new_name, bytes).unwrap_or(source)
}

fn decode_image(raw: &[u8], filename: &str) -> Option<DynamicImage> {
	let ext = Path::new(filename)
		.extension()
		.and_then(|e| e.to_str())
		.unwrap_or_default()
		.to_ascii_lowercase();
	if matches!(ext.as_str(), "heic" | "heif") || platform::heic::is_heic(raw) {
		let (w, h, rgba) = platform::heic::decode(
			raw,
			IMAGE_MAX_DIMENSION,
			IMAGE_DECODE_ALLOC_BYTES,
			IMAGE_MAX_DIMENSION,
		)?;
		if w == 0 || h == 0 || w > IMAGE_MAX_DIMENSION || h > IMAGE_MAX_DIMENSION {
			return None;
		}
		return image::RgbaImage::from_raw(w, h, rgba).map(DynamicImage::ImageRgba8);
	}
	let reader = ImageReader::new(Cursor::new(raw))
		.with_guessed_format()
		.ok()?;
	let mut limits = image::Limits::default();
	limits.max_image_width = Some(IMAGE_MAX_DIMENSION);
	limits.max_image_height = Some(IMAGE_MAX_DIMENSION);
	limits.max_alloc = Some(IMAGE_DECODE_ALLOC_BYTES);
	let mut decoder = reader.into_decoder().ok()?;
	decoder.set_limits(limits).ok()?;
	DynamicImage::from_decoder(decoder).ok()
}

fn replace_extension(filename: &str, new_ext: &str) -> String {
	let stem = Path::new(filename)
		.file_stem()
		.and_then(|s| s.to_str())
		.filter(|s| !s.is_empty())
		.unwrap_or("image");
	format!("{stem}.{new_ext}")
}

fn encode_webp_lossless(image: &DynamicImage) -> Option<Vec<u8>> {
	let rgba = image.to_rgba8();
	let mut out = Vec::new();
	WebPEncoder::new_lossless(&mut out)
		.encode(
			rgba.as_raw(),
			rgba.width(),
			rgba.height(),
			image::ExtendedColorType::Rgba8,
		)
		.ok()?;
	Some(out)
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
	let rgb = image.to_rgb8();
	let mut out = Vec::new();
	JpegEncoder::new_with_quality(&mut out, quality)
		.encode(
			rgb.as_raw(),
			rgb.width(),
			rgb.height(),
			image::ExtendedColorType::Rgb8,
		)
		.ok()?;
	Some(out)
}

fn encode_image_under_budget(
	raw: &[u8],
	filename: &str,
	target_bytes: u64,
) -> Option<(String, Vec<u8>)> {
	let decoded = decode_image(raw, filename)?;
	// Preserve alpha first via lossless WebP if it already shrinks below the upload ceiling.
	let has_alpha = decoded.color().has_alpha();
	if !has_alpha {
		if let Some(webp) = encode_webp_lossless(&decoded)
			&& (webp.len() as u64) <= target_bytes
		{
			return Some((replace_extension(filename, "webp"), webp));
		}
	} else {
		for max_edge in [None, Some(2560_u32), Some(1920), Some(1280)] {
			let candidate = match max_edge {
				Some(edge) if decoded.width() > edge || decoded.height() > edge => {
					decoded.resize(edge, edge, FilterType::Lanczos3)
				}
				Some(_) => continue,
				None => decoded.clone(),
			};
			if let Some(webp) = encode_webp_lossless(&candidate)
				&& (webp.len() as u64) <= target_bytes
			{
				return Some((replace_extension(filename, "webp"), webp));
			}
		}
	}

	// Progressive JPEG compression: step down quality before downscaling dimensions.
	let scales: [Option<u32>; 5] = [None, Some(2560), Some(1920), Some(1280), Some(960)];
	let qualities: [u8; 5] = [85, 76, 66, 56, 46];
	for max_edge in scales {
		let scaled = match max_edge {
			Some(edge) if decoded.width() > edge || decoded.height() > edge => {
				decoded.resize(edge, edge, FilterType::Lanczos3)
			}
			Some(_) => continue,
			None => decoded.clone(),
		};
		for quality in qualities {
			if let Some(jpg) = encode_jpeg(&scaled, quality)
				&& (jpg.len() as u64) <= target_bytes
			{
				let keep_ext = Path::new(filename)
					.extension()
					.and_then(|e| e.to_str())
					.is_some_and(|e| {
						matches!(ImageFormat::from_extension(e), Some(ImageFormat::Jpeg))
					});
				let out_name = if keep_ext {
					filename.to_owned()
				} else {
					replace_extension(filename, "jpg")
				};
				return Some((out_name, jpg));
			}
		}
	}
	None
}

async fn compress_video(source: Source, target_bytes: u64, cancelled: &AtomicBool) -> Source {
	let Some(local_path) = source.local_path().map(Path::to_path_buf) else {
		return source;
	};
	if source.size() > VIDEO_INPUT_CAP {
		return source;
	}
	let cancelled_flag = cancelled.load(Ordering::Acquire);
	if cancelled_flag {
		return source;
	}
	let original_name = source.filename().to_owned();
	let original_size = source.size();
	let result = tokio::task::spawn_blocking(move || {
		transcode_video_under_budget(&local_path, &original_name, target_bytes)
	})
	.await
	.ok()
	.flatten();
	let Some((new_name, bytes)) = result else {
		return source;
	};
	if (bytes.len() as u64) >= original_size || (bytes.len() as u64) > target_bytes {
		return source;
	}
	Source::compressed_bytes(new_name, bytes).unwrap_or(source)
}

struct TempDir(PathBuf);
impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

fn find_ffmpeg() -> Option<&'static str> {
	[
		"/opt/homebrew/bin/ffmpeg",
		"/usr/local/bin/ffmpeg",
		"/usr/bin/ffmpeg",
	]
	.into_iter()
	.find(|path| Path::new(path).is_file())
}

fn create_private_temp_dir() -> Option<TempDir> {
	let mut random = [0u8; 16];
	getrandom::fill(&mut random).ok()?;
	let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
	let dir = std::env::temp_dir().join(format!("didcord-upload-compress-{suffix}"));
	let builder = {
		#[cfg(unix)]
		{
			use std::os::unix::fs::DirBuilderExt;
			let mut builder = fs::DirBuilder::new();
			builder.mode(0o700);
			builder
		}
		#[cfg(not(unix))]
		{
			fs::DirBuilder::new()
		}
	};
	builder.create(&dir).ok()?;
	Some(TempDir(dir))
}

fn transcode_video_under_budget(
	input: &Path,
	filename: &str,
	target_bytes: u64,
) -> Option<(String, Vec<u8>)> {
	let executable = find_ffmpeg()?;
	let temp = create_private_temp_dir()?;
	let output = temp.0.join("compressed.mp4");
	// Try progressively tighter bitrate/resolution presets until cleanly below target_bytes.
	let profiles = [
		(1280_u32, "28", "128k"),
		(960_u32, "33", "96k"),
		(640_u32, "38", "64k"),
	];
	for (attempt, (max_width, crf, audio_bitrate)) in profiles.into_iter().enumerate() {
		let attempt_out = temp.0.join(format!("attempt-{attempt}.mp4"));
		let scale = format!("scale='min({max_width},iw)':-2:force_original_aspect_ratio=decrease");
		let fs_ceil = target_bytes.saturating_add(1024 * 1024).to_string();
		let mut cmd = build_ffmpeg_command(
			executable,
			input,
			&attempt_out,
			&scale,
			crf,
			audio_bitrate,
			&fs_ceil,
		);
		let mut child = cmd.spawn().ok()?;
		let deadline = Instant::now() + Duration::from_secs(90);
		let status = loop {
			if Instant::now() >= deadline {
				let _ = child.kill();
				let _ = child.wait();
				return None;
			}
			match child.try_wait() {
				Ok(Some(status)) => break status,
				Ok(None) => std::thread::sleep(Duration::from_millis(25)),
				Err(_) => {
					let _ = child.kill();
					let _ = child.wait();
					return None;
				}
			}
		};
		if !status.success() {
			continue;
		}
		let len = fs::metadata(&attempt_out).ok()?.len();
		if len > 0 && len <= target_bytes {
			fs::rename(&attempt_out, &output).ok()?;
			let bytes = fs::read(&output).ok()?;
			if (bytes.len() as u64) <= target_bytes {
				return Some((replace_extension(filename, "mp4"), bytes));
			}
		}
	}
	None
}

fn build_ffmpeg_command(
	executable: &str,
	input: &Path,
	output: &Path,
	scale: &str,
	crf: &str,
	audio_bitrate: &str,
	fs_ceil: &str,
) -> Command {
	#[cfg(unix)]
	let mut cmd = {
		let mut c = Command::new("/bin/sh");
		c.args([
			"-c",
			"ulimit -f 262144 || exit 1; exec \"$@\"",
			"didcord-upload-compress",
			executable,
		]);
		c
	};
	#[cfg(not(unix))]
	let mut cmd = Command::new(executable);
	cmd.args([
		"-nostdin",
		"-hide_banner",
		"-loglevel",
		"error",
		"-max_alloc",
		"16777216",
		"-threads",
		"2",
		"-protocol_whitelist",
		"file",
		"-format_whitelist",
		"mov,mp4,m4a,3gp,3g2,mj2,matroska,webm,avi",
		"-i",
	])
	.arg(input)
	.args([
		"-map",
		"0:v:0",
		"-map",
		"0:a:0?",
		"-sn",
		"-dn",
		"-map_metadata",
		"-1",
		"-vf",
		scale,
		"-c:v",
		"libx264",
		"-preset",
		"veryfast",
		"-crf",
		crf,
		"-pix_fmt",
		"yuv420p",
		"-c:a",
		"aac",
		"-b:a",
		audio_bitrate,
		"-ac",
		"2",
		"-ar",
		"48000",
		"-fs",
		fs_ceil,
		"-movflags",
		"+faststart",
		"-y",
	])
	.arg(output)
	.stdin(Stdio::null())
	.stdout(Stdio::null())
	.stderr(Stdio::null())
	.env_clear();
	cmd
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn oversized_synthetic_png_compresses_below_target_and_decodes() {
		let width = 640_u32;
		let height = 480_u32;
		let mut rgba = image::RgbaImage::new(width, height);
		for (x, y, pixel) in rgba.enumerate_pixels_mut() {
			let r = ((x * 13 + y * 7) % 255) as u8;
			let g = ((x * 5 + y * 19) % 255) as u8;
			let b = ((x ^ y) % 255) as u8;
			*pixel = image::Rgba([r, g, b, 255]);
		}
		let mut png_bytes = Vec::new();
		DynamicImage::ImageRgba8(rgba)
			.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
			.unwrap();
		// Target noticeably smaller than raw PNG bytes so compression must trigger.
		let target = (png_bytes.len() as u64) / 3;
		assert!(target > 8_192);
		let (out_name, compressed) =
			encode_image_under_budget(&png_bytes, "noisy.png", target).unwrap();
		assert!(out_name.ends_with(".jpg") || out_name.ends_with(".webp"));
		assert!((compressed.len() as u64) <= target);
		let decoded = ImageReader::new(Cursor::new(&compressed))
			.with_guessed_format()
			.unwrap()
			.decode()
			.unwrap();
		assert!(decoded.width() > 0 && decoded.height() > 0);
	}
}
