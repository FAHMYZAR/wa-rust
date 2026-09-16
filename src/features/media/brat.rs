use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;

pub struct BratFeature;

#[async_trait]
impl Feature for BratFeature {
    fn name(&self) -> &'static str {
        "brat"
    }

    fn description(&self) -> &'static str {
        "Buat stiker teks gaya Brat (Charli XCX)"
    }

    fn usage(&self) -> &'static str {
        "brat <teks> atau reply pesan dengan .brat"
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let text = if !ctx.args.trim().is_empty() {
            ctx.args.trim().to_string()
        } else {
            let quoted = crate::utils::group::context_info(&ctx.msg.message)
                .and_then(|ci| ci.quoted_message.as_option())
                .and_then(|qm| {
                    qm.extended_text_message
                        .as_option()
                        .and_then(|et| et.text.clone())
                })
                .unwrap_or_default();
            if quoted.is_empty() {
                ctx.reply("❌ Masukkan teks atau reply pesan!\n\nContoh:\n> .brat hello world\n> Reply pesan + .brat")
                    .await?;
                return Ok(());
            }
            quoted
        };

        let _ = ctx.react("⏳").await;

        let webp_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let temp_out = media::temp_file_path_str("brat", "webp");
            render_brat(&text, &temp_out)?;
            let out = std::fs::read(&temp_out).context("baca hasil brat webp gagal")?;
            let _ = std::fs::remove_file(&temp_out);
            Ok(out)
        })
        .await??;

        let upload = ctx
            .msg
            .client
            .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
            .await
            .context("upload stiker brat gagal")?;

        let msg = media::sticker_message(upload, false);
        let _ = ctx.react("").await;
        ctx.send(msg).await
    }
}

/// Background colour of every Brat image. White, matching the reference
/// sample: black condensed lowercase on a plain white square.
const BRAT_BG: &str = "0xFFFFFF";
/// Square canvas edge, in pixels.
const BRAT_SIZE: u32 = 512;
/// Padding from the canvas edge to the text block, in pixels. The Brat
/// reference sits in the top-left corner with a small margin, not centred.
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
/// real Brat generator does: few words get huge type, long text shrinks and
/// wraps into a dense justified-looking block.
///
/// Arial Narrow advances roughly `0.48em` per glyph, so the wrap width is the
/// largest number of characters that still fits inside a 90% margin.
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

/// Greedy word wrap at `max_chars`, joined with `\n` for `drawtext`/`caption:`.
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

/// Render a Brat sticker. ffmpeg is preferred because it gives an identical
/// result on every platform; ImageMagick is only used when ffmpeg is missing.
fn render_brat(text: &str, out_path: &str) -> Result<()> {
    if media::require_program("ffmpeg").is_ok() {
        return render_brat_ffmpeg(text, out_path);
    }
    render_brat_magick(text, out_path)
}

/// ffmpeg renderer.
///
/// Runs with `current_dir` set to the scratch directory so the filter string
/// only ever contains bare filenames.
///
/// Filter chain, in order:
/// 1. `drawtext` — black lowercase Arial Narrow, top-left anchored, tight
///    leading.
/// 2. `scale=iw*1.25` + `crop` — stretches the glyphs horizontally (the Brat
///    "stretched text" look) while keeping the 512x512 frame.
/// 3. `scale=192` then `scale=512` — crush to low resolution and blow it back
///    up with bilinear filtering, producing the blurry/aliased print look.
/// 4. `gblur` — a faint blur so edges never look vector-crisp.
fn render_brat_ffmpeg(text: &str, out_path: &str) -> Result<()> {
    let out = std::path::PathBuf::from(out_path);
    let dir = out
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path keluaran brat tak valid"))?
        .to_path_buf();
    let out_name = out
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("nama file keluaran brat tak valid"))?
        .to_string();

    let token = media::temp_token("brat_txt");
    let txt_name = format!("{token}.txt");
    let (fontsize, max_chars) = brat_layout(text);
    std::fs::write(dir.join(&txt_name), brat_wrap(text, max_chars))?;

    let mut draw = String::from("drawtext=");
    if let Some(font) = media::stage_font(&dir) {
        draw.push_str(&format!("fontfile={font}:"));
    }
    draw.push_str(&format!(
        "textfile={txt_name}:fontcolor=black:fontsize={fontsize}:"
    ));
    draw.push_str(&format!(
        "x={p}:y={p}:line_spacing=-6:text_align=L",
        p = BRAT_PAD
    ));

    // Crop from the top-left corner: the text block is left-aligned, so the
    // default centre crop would slice the stretched glyphs off the canvas.
    let filter = format!(
        "{draw},scale=iw*{BRAT_STRETCH}:ih,crop={s}:{s}:0:0,",
        s = BRAT_SIZE
    );
    let filter = format!(
        "{filter}scale=170:170:flags=bilinear,scale={s}:{s}:flags=bilinear,gblur=sigma=0.9",
        s = BRAT_SIZE
    );

    let status = Command::new("ffmpeg")
        .current_dir(&dir)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={BRAT_BG}:s={BRAT_SIZE}x{BRAT_SIZE}"),
            "-vf",
            &filter,
            "-frames:v",
            "1",
            &out_name,
        ])
        .status()?;
    let _ = std::fs::remove_file(dir.join(&txt_name));
    if !status.success() {
        anyhow::bail!("ffmpeg brat render gagal");
    }
    Ok(())
}

/// ImageMagick fallback, used only when ffmpeg is unavailable.
fn render_brat_magick(text: &str, out_path: &str) -> Result<()> {
    media::require_program("magick")?;
    let (fontsize, max_chars) = brat_layout(text);
    let status = Command::new("magick")
        .args([
            "-size",
            "512x512",
            "xc:#FFFFFF",
            "-font",
            "DejaVu-Sans-Condensed",
            "-pointsize",
            &fontsize.to_string(),
            "-fill",
            "black",
            "-gravity",
            "center",
            "-interline-spacing",
            "-6",
            &format!("caption:{}", brat_wrap(text, max_chars)),
            "-composite",
            "-resize",
            "125%x100%",
            "-gravity",
            "center",
            "-extent",
            "512x512",
            "-blur",
            "0x0.8",
            out_path,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("magick brat render gagal");
    }
    Ok(())
}
