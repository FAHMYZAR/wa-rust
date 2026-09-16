use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

/// Frame per detik stiker animasi Brat.
const FPS: usize = 12;
/// Jumlah frame minimum. Kalau katanya lebih banyak, satu kata tetap dapat
/// satu frame sendiri supaya tidak ada kata yang terlewat.
const ANIM_FRAMES: usize = 24;
/// Batas atas jumlah frame supaya stiker tidak jadi terlalu panjang.
const MAX_FRAMES: usize = 60;

pub struct BratvidFeature;

#[async_trait]
impl Feature for BratvidFeature {
    fn name(&self) -> &'static str {
        "bratvid"
    }

    fn description(&self) -> &'static str {
        "Buat stiker video gaya Brat (teks muncul per kata)"
    }

    fn usage(&self) -> &'static str {
        "bratvid <teks>"
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        if ctx.args.trim().is_empty() {
            ctx.reply("❌ Masukkan teks untuk Brat Video!\n\nContoh: .bratvid hello world")
                .await?;
            return Ok(());
        }
        let text = ctx.args.trim().to_string();

        let _ = ctx.react("⏳").await;

        let webp_bytes =
            tokio::task::spawn_blocking(move || -> Result<Vec<u8>> { render_bratvid(&text) })
                .await??;

        let upload = ctx
            .msg
            .client
            .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
            .await
            .context("upload bratvid gagal")?;

        let msg = media::sticker_message(upload, true);
        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}

/// Background colour of every Brat frame. White, matching `.brat`.
const BRAT_BG: &str = "0xFFFFFF";
/// Square canvas edge, in pixels.
const BRAT_SIZE: u32 = 512;
/// Padding from the canvas edge to the text block, in pixels. Words stack in
/// the top-left corner and grow downward as each one is revealed.
const BRAT_PAD: u32 = 32;
/// Horizontal stretch applied to the glyphs — the Brat "stretched text" look.
const BRAT_STRETCH: f64 = 1.25;
/// Average advance width of Arial Narrow lowercase, in em.
const BRAT_ADVANCE_EM: f64 = 0.46;
/// Fraction of the canvas the *unstretched* text block is allowed to span.
/// Kept below `1 / BRAT_STRETCH` so the horizontal stretch still lands inside
/// the 512px square — the left-anchored crop can only keep x=0..512.
const BRAT_FILL: f64 = 0.72;

/// Pick a font size plus a wrap width so `text` fills the square the way the
/// real Brat generator does.
fn brat_layout(text: &str) -> (u32, usize) {
    let n = text.chars().count().max(1);
    let mut fontsize: f64 = if n <= 6 {
        128.0
    } else if n <= 12 {
        104.0
    } else if n <= 24 {
        84.0
    } else if n <= 48 {
        64.0
    } else if n <= 96 {
        50.0
    } else if n <= 200 {
        38.0
    } else {
        30.0
    };

    // A single word can never be wrapped, so the longest word caps the size.
    let longest = text
        .split_whitespace()
        .map(|w| w.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let usable = BRAT_SIZE as f64 * BRAT_FILL;
    let cap = usable / (longest * BRAT_ADVANCE_EM * BRAT_STRETCH);
    if cap < fontsize {
        fontsize = cap;
    }
    fontsize = fontsize.clamp(16.0, 160.0);

    // Characters that still fit on one line once the glyphs are stretched.
    let max_chars =
        ((usable / (BRAT_ADVANCE_EM * BRAT_STRETCH * fontsize)).floor() as usize).max(1);
    (fontsize.round() as u32, max_chars)
}

/// Greedy word wrap at `max_chars`, joined with `\n` for `drawtext`.
fn brat_wrap(text: &str, max_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\n")
}

/// Render the animated Brat sticker.
///
/// Every word pops in one after another on the exact same canvas, font and
/// filter chain as the static `.brat` sticker — no sliding, no overlap.
///
/// A single unique token is minted up front and shared by the frame sequence,
/// the per-step text files and the output file, so the `-i` glob handed to
/// ffmpeg is byte-for-byte the pattern the frame generator wrote to.
fn render_bratvid(text: &str) -> Result<Vec<u8>> {
    media::require_program("ffmpeg")?;

    let dir = media::temp_dir();
    let token = media::temp_token("bratvid");
    let out_name = format!("{token}.webp");
    let out_path = dir.join(&out_name);

    let frames = render_bratvid_frames(text, &dir, &token)?;
    let frame_pattern = format!("{token}_%03d.png");

    let status = Command::new("ffmpeg")
        .current_dir(&dir)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-framerate",
            &FPS.to_string(),
            "-start_number",
            "1",
            "-f",
            "image2",
            "-i",
            &frame_pattern,
            "-c:v",
            "libwebp",
            "-loop",
            "0",
            "-q:v",
            "80",
            &out_name,
        ])
        .status()?;

    // image2 numbers the sequence from 1, so sweep a little past the end.
    for index in 0..=(frames + 2) {
        let _ = std::fs::remove_file(dir.join(format!("{token}_{index:03}.png")));
    }

    if !status.success() {
        let _ = std::fs::remove_file(&out_path);
        anyhow::bail!("encode animated webp gagal");
    }

    let bytes = std::fs::read(&out_path).context("baca bratvid gagal")?;
    let _ = std::fs::remove_file(&out_path);
    Ok(bytes)
}

/// Generate the word-by-word frame sequence.
///
/// Every word owns a slice of the timeline. For each slice a `drawtext` filter
/// holding that slice's cumulative sentence is stacked on top of the previous
/// ones, each with its own `enable='between(n,start,end)'` window — so at frame
/// `n` the canvas shows exactly the words revealed so far and nothing else.
/// The font size and wrap width are taken from the finished sentence so the
/// layout never shifts while the words appear.
///
/// Returns the number of frames written. Runs with `current_dir` set to the
/// scratch directory.
fn render_bratvid_frames(text: &str, dir: &std::path::Path, token: &str) -> Result<usize> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        anyhow::bail!("teks bratvid kosong");
    }
    let total = words.len();

    // At least one frame per word so nothing is skipped, capped so the sticker
    // never runs longer than a few seconds.
    let frames = ANIM_FRAMES.max(total).min(MAX_FRAMES);
    let (fontsize, max_chars) = brat_layout(text);

    // One cumulative text file per reveal step.
    let mut step_files = Vec::with_capacity(total);
    for index in 0..total {
        let cumulative = words[..=index].join(" ");
        let name = format!("{token}_step_{index:02}.txt");
        std::fs::write(dir.join(&name), brat_wrap(&cumulative, max_chars))?;
        step_files.push(name);
    }

    let font = media::stage_font(dir);
    let mut filters: Vec<String> = Vec::with_capacity(total + 2);
    for (index, step_file) in step_files.iter().enumerate() {
        let start = index * frames / total;
        let end = if index + 1 == total {
            frames - 1
        } else {
            ((index + 1) * frames / total) - 1
        };
        let mut draw = String::from("drawtext=");
        if let Some(font) = font.as_deref() {
            draw.push_str(&format!("fontfile={font}:"));
        }
        draw.push_str(&format!(
            "textfile={step_file}:fontcolor=black:fontsize={fontsize}:x={p}:y={p}:line_spacing=-6:text_align=L:enable='between(n,{start},{end})'",
            p = BRAT_PAD
        ));
        filters.push(draw);
    }

    // Same finishing chain as the static sticker: stretch, crush, blur.
    // Left-anchored crop keeps the top-left anchored block inside the canvas.
    filters.push(format!(
        "scale=iw*{BRAT_STRETCH}:ih,crop={s}:{s}:0:0",
        s = BRAT_SIZE
    ));
    filters.push(format!(
        "scale=170:170:flags=bilinear,scale={s}:{s}:flags=bilinear,gblur=sigma=0.9",
        s = BRAT_SIZE
    ));
    let filter = filters.join(",");
    let frame_pattern = format!("{token}_%03d.png");

    let status = Command::new("ffmpeg")
        .current_dir(dir)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={BRAT_BG}:s={BRAT_SIZE}x{BRAT_SIZE}:r={FPS}"),
            "-vf",
            &filter,
            "-frames:v",
            &frames.to_string(),
            &frame_pattern,
        ])
        .status()?;

    for name in &step_files {
        let _ = std::fs::remove_file(dir.join(name));
    }

    if !status.success() {
        anyhow::bail!("ffmpeg bratvid frames gagal");
    }
    Ok(frames)
}
