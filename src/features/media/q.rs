use crate::core::feature::{Category, CommandContext, Feature};
use crate::utils::media;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::process::Command;
use whatsapp_rust::download::MediaType;
use whatsapp_rust::upload::UploadOptions;
use whatsapp_rust::wacore::iq::contacts::ProfilePictureSpec;
use whatsapp_rust::Jid;

pub struct QFeature;

#[async_trait]
impl Feature for QFeature {
    fn name(&self) -> &'static str {
        "q"
    }

    fn description(&self) -> &'static str {
        "Ubah quoted message jadi stiker (Gaya Quotly)"
    }

    fn usage(&self) -> &'static str {
        "q (reply ke pesan teks)"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["quotly", "quote"]
    }

    fn category(&self) -> Category {
        Category::Media
    }

    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<()> {
        let ci = match crate::utils::group::context_info(&ctx.msg.message) {
            Some(c) => c,
            None => {
                ctx.reply("❌ Reply pesan yang ingin dijadikan stiker!")
                    .await?;
                return Ok(());
            }
        };

        let quoted_msg = match ci.quoted_message.as_option() {
            Some(qm) => qm,
            None => {
                ctx.reply("❌ Reply pesan yang ingin dijadikan stiker!")
                    .await?;
                return Ok(());
            }
        };

        // Pesan teks biasa di WhatsApp disimpan di field `conversation`, bukan
        // `extended_text_message` (yang hanya dipakai untuk reply/quote). Tanpa
        // pengecekan `conversation`, reply ke bubble teks polos selalu dianggap
        // "tidak memiliki teks". Caption gambar/video tetap dicek sebagai
        // fallback karena bukan bagian dari `conversation`.
        let text = quoted_msg
            .conversation
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                quoted_msg
                    .extended_text_message
                    .as_option()
                    .and_then(|et| et.text.clone())
            })
            .or_else(|| {
                quoted_msg
                    .image_message
                    .as_option()
                    .and_then(|im| im.caption.clone())
            })
            .or_else(|| {
                quoted_msg
                    .video_message
                    .as_option()
                    .and_then(|vm| vm.caption.clone())
            })
            .unwrap_or_default();

        if text.is_empty() {
            ctx.reply("pesan yang di-quote tidak memiliki teks!")
                .await?;
            return Ok(());
        }

        // Resolusi target JID: reply quote atau sender
        let target_jid: Jid = crate::utils::group::resolve_target_jid(ctx).unwrap_or_else(|_| {
            ci.participant
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .and_then(|p| p.parse::<Jid>().ok())
                .unwrap_or_else(|| ctx.msg.info.source.sender.clone())
        });

        // 1. Dapatkan display name
        // a) Jika quote pesan sender sendiri & push_name ada
        let mut resolved_name =
            if target_jid.user == ctx.sender_user() && !ctx.msg.info.push_name.trim().is_empty() {
                Some(ctx.msg.info.push_name.clone())
            } else {
                None
            };

        // b) Cari dari contact cache / SQLite store
        if resolved_name.is_none() {
            resolved_name = crate::utils::contact::get_contact_name(&target_jid.user);
        }

        // c) Jika di group, coba cari username dari metadata group
        if resolved_name.is_none() && ctx.is_group {
            if let Ok(meta) =
                crate::utils::group::get_metadata(&ctx.msg.client, ctx.group_jid()).await
            {
                let target_user = &target_jid.user;
                if let Some(p) = meta.participants.iter().find(|p| {
                    p.jid.user == *target_user
                        || p.phone_number
                            .as_ref()
                            .is_some_and(|pn| pn.user == *target_user)
                        || p.lid.as_ref().is_some_and(|lid| lid.user == *target_user)
                }) {
                    if let Some(u) = &p.username {
                        let u_str = u.to_string();
                        if !u_str.trim().is_empty() {
                            resolved_name = Some(u_str);
                        }
                    }
                }
            }
        }

        // d) Fallback ke nomor / ID user
        let name = resolved_name.unwrap_or_else(|| target_jid.user.to_string());

        let _ = ctx.react("⏳").await;

        // 2. Ambil profile picture (jika ada & diizinkan privacy WA)
        let avatar_bytes: Option<Vec<u8>> = {
            let pic = ctx
                .msg
                .client
                .execute(ProfilePictureSpec::preview(&target_jid))
                .await
                .ok()
                .flatten();

            if let Some(pic) = pic {
                let url = pic.url;
                tokio::task::spawn_blocking(move || crate::utils::http::get_bytes(&url, 5).ok())
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        };

        let result: Result<()> = async {
            let name_clone = name.clone();
            let text_clone = text.clone();
            let webp_bytes = tokio::task::spawn_blocking(move || {
                render_quote(&name_clone, &text_clone, avatar_bytes.as_deref())
            })
            .await??;

            let upload = ctx
                .msg
                .client
                .upload(webp_bytes, MediaType::Sticker, UploadOptions::default())
                .await
                .context("upload quote sticker gagal")?;

            let msg = media::sticker_message(upload, false);
            ctx.send(msg).await
        }
        .await;

        let _ = ctx.unreact().await;
        if result.is_err() {
            ctx.reply("❌ Gagal membuat stiker!").await?;
        }
        Ok(())
    }
}

/// Render a Quotly-style quote sticker entirely offline with ffmpeg.
///
/// Pure Rust: no Node.js, no external HTTP API. The geometry is computed, not
/// hardcoded: the message is wrapped by estimated pixel width, the bubble hugs
/// its content, the circular avatar sits *outside* the bubble on the left, and
/// the finished component is scaled proportionally and centred on a transparent
/// 512x512 sticker canvas.
fn render_quote(name: &str, text: &str, avatar_data: Option<&[u8]>) -> Result<Vec<u8>> {
    media::require_program("ffmpeg")?;

    // measure -> wrap -> natural bubble -> whole component -> scale -> positions
    let layout = QuoteLayout::compute(name, text);

    let dir = media::temp_dir();
    let token = media::temp_token("quote");
    let out_name = format!("{token}.webp");
    let out_path = dir.join(&out_name);

    // drawtext reads its payload from files, so quotes, colons and percent
    // signs in the message never need escaping — the old inline filter strings
    // broke on exactly those characters.
    let initial_file = format!("{token}_ini.txt");
    let name_file = format!("{token}_name.txt");
    let msg_file = format!("{token}_msg.txt");
    std::fs::write(dir.join(&initial_file), initial_of(name))?;
    std::fs::write(dir.join(&name_file), name)?;
    std::fs::write(dir.join(&msg_file), layout.lines.join("\n"))?;

    let font = media::stage_font(&dir)
        .map(|f| format!("fontfile={f}:"))
        .unwrap_or_default();
    let accent = accent_colour(name);

    // `geq` rewrites the alpha channel pixel by pixel: the distance to the
    // nearest corner centre drives a one-pixel falloff, so the bubble corners
    // and the circular avatar stay smooth at any scale.
    let r = layout.bubble_radius;
    let bubble_alpha = format!(
        "clip(255*({r}-hypot(max(0,{r}-X)+max(0,X-{wr}),max(0,{r}-Y)+max(0,Y-{hr}))),0,255)",
        wr = layout.bubble_w - r,
        hr = layout.bubble_h - r,
    );
    let ar = layout.avatar_radius;
    let avatar_alpha = format!("clip(255*({ar}-hypot(X-{ar},Y-{ar})),0,255)");

    // The base layer is component-sized, not canvas-sized: overlays are placed
    // in component coordinates and the final `pad` centres the component on the
    // transparent 512x512 sticker canvas.
    let canvas = format!(
        "color=c=0x00000000:s={w}x{h},format=rgba",
        w = layout.canvas_w,
        h = layout.canvas_h
    );
    let card = format!(
        "color=c={BUBBLE_BG}:s={w}x{h},format=rgba",
        w = layout.bubble_w,
        h = layout.bubble_h
    );

    // Avatar source: real profile photo if downloaded, otherwise accent circle with initial
    let av_file = format!("{token}_av.jpg");
    let has_avatar_img = if let Some(bytes) = avatar_data {
        std::fs::write(dir.join(&av_file), bytes).is_ok()
    } else {
        false
    };

    let (av_input_args, av_filter_prep, av_initial_draw) = if has_avatar_img {
        (
            vec!["-i".to_string(), av_file.clone()],
            format!(
                "[2:v]scale={s}:{s}:flags=lanczos,format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='{avatar_alpha}'[av];",
                s = layout.avatar_size
            ),
            String::new(),
        )
    } else {
        (
            vec![
                "-f".to_string(),
                "lavfi".to_string(),
                "-i".to_string(),
                format!(
                    "color=c={accent}:s={s}x{s},format=rgba",
                    s = layout.avatar_size
                ),
            ],
            format!(
                "[2:v]format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='{avatar_alpha}'[av];"
            ),
            format!(
                "drawtext={font}textfile={initial_file}:fontcolor=white:fontsize={initial_size}:x={ax}+({av_size}-tw)/2:y={ay}+({av_size}-th)/2+{ini_dy},",
                initial_size = layout.initial_size,
                ax = layout.avatar_x,
                ay = layout.avatar_y,
                av_size = layout.avatar_size,
                ini_dy = layout.initial_y_offset,
            ),
        )
    };

    let filter = format!(
        "[1:v]format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='{bubble_alpha}'[card];\
         {av_filter_prep}\
         [0:v][card]overlay={bx}:{by}[bg];\
         [bg][av]overlay={ax}:{ay}[base];\
         [base]{av_initial_draw}\
         drawtext={font}textfile={name_file}:fontcolor={accent}:fontsize={name_size}:x={tx}:y={ny},\
         drawtext={font}textfile={msg_file}:fontcolor={MSG_COLOR}:fontsize={msg_size}:x={tx}:y={my}:line_spacing={line_spacing},\
         pad={canvas_size}:{canvas_size}:(ow-iw)/2:(oh-ih)/2:color=black@0[out]",
        bx = layout.bubble_x,
        by = layout.bubble_y,
        ax = layout.avatar_x,
        ay = layout.avatar_y,
        name_size = layout.name_size,
        msg_size = layout.msg_size,
        tx = layout.text_x,
        ny = layout.name_y,
        my = layout.message_y,
        line_spacing = layout.line_spacing,
        canvas_size = QUOTE_CANVAS,
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.current_dir(&dir).args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &canvas,
        "-f",
        "lavfi",
        "-i",
        &card,
    ]);
    cmd.args(&av_input_args);
    cmd.args([
        "-filter_complex",
        &filter,
        "-map",
        "[out]",
        "-frames:v",
        "1",
        "-c:v",
        "libwebp",
        "-lossless",
        "0",
        "-q:v",
        "85",
        &out_name,
    ]);

    let status = cmd.status()?;

    for file in [&initial_file, &name_file, &msg_file] {
        let _ = std::fs::remove_file(dir.join(file));
    }
    if has_avatar_img {
        let _ = std::fs::remove_file(dir.join(&av_file));
    }

    if !status.success() {
        let _ = std::fs::remove_file(&out_path);
        anyhow::bail!("render quote gagal");
    }
    let bytes = std::fs::read(&out_path).context("baca quote webp gagal")?;
    let _ = std::fs::remove_file(&out_path);
    Ok(bytes)
}

/// First alphanumeric character of the display name, uppercased — the avatar
/// initial drawn inside the coloured square.
fn initial_of(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Deterministic accent colour for a sender, picked from a small palette so the
/// same person always gets the same avatar tint.
fn accent_colour(name: &str) -> &'static str {
    const PALETTE: &[&str] = &[
        "0x008069", // WhatsApp Green
        "0x1f7aec", // Blue
        "0xd84315", // Deep Orange
        "0x8e24aa", // Purple
        "0x00897b", // Teal
        "0xc2185b", // Pink
        "0x3949ab", // Indigo
        "0x2e7d32", // Forest Green
    ];
    let hash = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[(hash as usize) % PALETTE.len()]
}

// -- Quote layout engine -----------------------------------------------------
//
// The renderer never assumes a card size. The pipeline is:
//
//   measure content -> wrap by pixel width -> natural bubble -> whole component
//   -> proportional scale -> absolute positions -> ffmpeg filters
//
// Every coordinate used by `render_quote` comes from `QuoteLayout::compute`.

/// Output sticker canvas. The component is centred inside it.
const QUOTE_CANVAS: i32 = 512;
/// Usable square inside the canvas, leaving a margin around the artwork.
const QUOTE_AVAIL: i32 = 456;

const NAME_SIZE: i32 = 21;
const MSG_SIZE: i32 = 25;
/// Avatar initial size, as a fraction of the avatar diameter.
const INITIAL_RATIO: f32 = 0.52;

const BUBBLE_PAD_L: i32 = 18;
const BUBBLE_PAD_R: i32 = 20;
const BUBBLE_PAD_T: i32 = 14;
const BUBBLE_PAD_B: i32 = 15;
/// Vertical gap between the sender name and the first message line.
const NAME_GAP: i32 = 5;
/// Extra pixels added between wrapped message lines.
const LINE_SPACING: i32 = 4;

/// Line box height as a multiple of the font size.
const NAME_LEADING: f32 = 1.18;
const MSG_LEADING: f32 = 1.18;

const MIN_BUBBLE_W: i32 = 120;
const MAX_BUBBLE_W: i32 = 400;
const MAX_TEXT_W: i32 = MAX_BUBBLE_W - BUBBLE_PAD_L - BUBBLE_PAD_R;

const BUBBLE_RADIUS: i32 = 18;
/// Bubble background: WhatsApp Light Green bubble (#D9FDD3)
const BUBBLE_BG: &str = "0xd9fdd3";
/// Message text color: WhatsApp dark text (#111B21)
const MSG_COLOR: &str = "0x111b21";

const AVATAR_SIZE: i32 = 46;
const AVATAR_GAP: i32 = 10;

/// Upper bound on the proportional upscale, so a one-word message does not
/// balloon into a full-canvas poster.
const MAX_SCALE: f32 = 1.6;

/// Resolved geometry for one quote sticker. All fields are in final (already
/// scaled) pixels, so they can be handed straight to ffmpeg.
struct QuoteLayout {
    canvas_w: i32,
    canvas_h: i32,

    bubble_x: i32,
    bubble_y: i32,
    bubble_w: i32,
    bubble_h: i32,
    bubble_radius: i32,

    avatar_x: i32,
    avatar_y: i32,
    avatar_size: i32,
    avatar_radius: i32,
    initial_size: i32,
    initial_y_offset: i32,

    text_x: i32,
    name_y: i32,
    message_y: i32,
    name_size: i32,
    msg_size: i32,
    line_spacing: i32,

    lines: Vec<String>,
}

impl QuoteLayout {
    /// Measure the content, then derive every position from it.
    fn compute(name: &str, text: &str) -> Self {
        let lines = wrap_text(text, MSG_SIZE, MAX_TEXT_W);

        // Content width: the longest of the name and the wrapped message.
        let name_w = measure_text_width(name, NAME_SIZE);
        let msg_w = lines
            .iter()
            .map(|line| measure_text_width(line, MSG_SIZE))
            .max()
            .unwrap_or(0);
        let content_w = name_w.max(msg_w);

        let name_h = (NAME_SIZE as f32 * NAME_LEADING).round() as i32;
        let msg_h = (MSG_SIZE as f32 * MSG_LEADING).round() as i32;
        let rows = lines.len() as i32;

        // Natural (unscaled) bubble: content hugging, never a fixed card.
        let natural_bubble_w =
            (content_w + BUBBLE_PAD_L + BUBBLE_PAD_R).clamp(MIN_BUBBLE_W, MAX_BUBBLE_W);
        let natural_bubble_h = BUBBLE_PAD_T
            + name_h
            + NAME_GAP
            + msg_h * rows
            + LINE_SPACING * (rows - 1)
            + BUBBLE_PAD_B;

        let natural_w = AVATAR_SIZE + AVATAR_GAP + natural_bubble_w;
        let natural_h = natural_bubble_h.max(AVATAR_SIZE);

        // Scale the whole component proportionally: avatar, fonts, padding and
        // radius all grow together, so a short message stays large without the
        // bubble losing its proportions.
        let scale = (QUOTE_AVAIL as f32 / natural_w as f32)
            .min(QUOTE_AVAIL as f32 / natural_h as f32)
            .min(MAX_SCALE);
        let s = |v: i32| (v as f32 * scale).round() as i32;

        let canvas_w = s(natural_w);
        let canvas_h = s(natural_h);

        let bubble_w = s(natural_bubble_w);
        let bubble_h = s(natural_bubble_h);
        let avatar_size = s(AVATAR_SIZE);

        // The radius must stay inside the bubble on both axes.
        let bubble_radius = s(BUBBLE_RADIUS).min(bubble_w / 2).min(bubble_h / 2).max(1);

        let bubble_x = avatar_size + s(AVATAR_GAP);
        let bubble_y = (canvas_h - bubble_h) / 2;
        let avatar_x = 0;
        // For short bubbles (1-2 lines), center the avatar vertically against
        // the bubble. For tall multi-line bubbles, keep it neatly anchored near
        // the header so it aligns with the sender name.
        let avatar_y = bubble_y
            + ((bubble_h - avatar_size) / 2).clamp(s(BUBBLE_PAD_T - 2), s(BUBBLE_PAD_T + 6));

        let name_y = bubble_y + s(BUBBLE_PAD_T);
        let text_x = bubble_x + s(BUBBLE_PAD_L);
        let message_y = name_y + s(name_h) + s(NAME_GAP);
        let initial_size = (avatar_size as f32 * INITIAL_RATIO).round() as i32;
        let initial_y_offset = (initial_size as f32 * 0.08).round() as i32;

        Self {
            canvas_w,
            canvas_h,
            bubble_x,
            bubble_y,
            bubble_w,
            bubble_h,
            bubble_radius,
            avatar_x,
            avatar_y,
            avatar_size,
            avatar_radius: avatar_size / 2,
            initial_size,
            initial_y_offset,
            text_x,
            name_y,
            message_y,
            name_size: (NAME_SIZE as f32 * scale).round() as i32,
            msg_size: (MSG_SIZE as f32 * scale).round() as i32,
            line_spacing: s(LINE_SPACING),
            lines,
        }
    }
}

/// Estimated advance width of one character, in em units, for a humanist
/// sans-serif. FFmpeg's `drawtext` exposes no measurement API, so the engine
/// approximates glyph advances instead of counting characters — "iiii" and
/// "WWWW" are both four characters but nowhere near the same width.
fn char_em(c: char) -> f32 {
    match c {
        ' ' | '\t' => 0.28,
        'i' | 'j' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' | '`' | 'I' => 0.28,
        'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '{' | '}' | '-' | '"' | '/' | '\\' => 0.38,
        'm' | 'w' | 'M' | 'W' => 0.92,
        'A'..='Z' => 0.68,
        '0'..='9' => 0.58,
        c if c.is_ascii_lowercase() => 0.54,
        _ => 1.0,
    }
}

/// Approximate rendered width of `text` at `font_size` pixels. A small bias
/// keeps the bubble a touch wider than the real glyphs so text never touches
/// the right edge on fonts that are wider than the estimate.
fn measure_text_width(text: &str, font_size: i32) -> i32 {
    let em: f32 = text.chars().map(char_em).sum();
    (em * font_size as f32 * 1.06).ceil() as i32
}

/// Greedily wrap `text` so no line exceeds `max_width` pixels. Words longer
/// than the bubble are hard-broken by character rather than allowed to overflow.
fn wrap_text(text: &str, font_size: i32, max_width: i32) -> Vec<String> {
    let space_w = measure_text_width(" ", font_size);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0i32;

    for word in text.split_whitespace() {
        let word_w = measure_text_width(word, font_size);

        if !current.is_empty() {
            if current_w + space_w + word_w <= max_width {
                current.push(' ');
                current.push_str(word);
                current_w += space_w + word_w;
                continue;
            }
            lines.push(std::mem::take(&mut current));
            current_w = 0;
        }

        if word_w <= max_width {
            current.push_str(word);
            current_w = word_w;
            continue;
        }

        for ch in word.chars() {
            let cw = measure_text_width(&ch.to_string(), font_size);
            if current_w + cw > max_width && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_w = 0;
            }
            current.push(ch);
            current_w += cw;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-word message must hug its content: the avatar sits fully outside
    /// the bubble, and neither dimension is padded out to a fixed card size.
    #[test]
    fn short_message_bubble_hugs_content() {
        let layout = QuoteLayout::compute("YouTheSlayer", "Hmm");

        assert_eq!(layout.lines, vec!["Hmm".to_string()]);
        // Avatar is a separate element to the left of the bubble.
        assert_eq!(layout.avatar_x, 0);
        assert!(layout.avatar_x + layout.avatar_size <= layout.bubble_x);
        // Natural height stays around two content rows, nowhere near 220px.
        assert!(layout.bubble_h < 200, "bubble_h={}", layout.bubble_h);
        // Longest visible line is the name, and the bubble is not canvas-wide.
        assert!(layout.bubble_w < QUOTE_CANVAS);
        assert!(layout.canvas_w <= QUOTE_CANVAS);
        assert!(layout.canvas_h <= QUOTE_CANVAS);
    }

    /// Height must follow the wrapped line count, not a fixed dimension.
    #[test]
    fn height_grows_with_wrapped_lines() {
        let short = QuoteLayout::compute("User", "Hmm");
        let long = QuoteLayout::compute(
            "User",
            "apakah memang seperti itu atau sebenarnya ada penyebab lain?",
        );

        assert!(long.lines.len() > short.lines.len());
        assert!(long.bubble_h > short.bubble_h);
        assert!(long.bubble_w <= MAX_BUBBLE_W);
    }

    /// Wrapping is driven by estimated pixel width, so a run of narrow glyphs
    /// stays on one line while the same number of wide glyphs wraps.
    #[test]
    fn wrapping_uses_pixel_width_not_char_count() {
        let narrow = wrap_text("iiiiiiiiii", MSG_SIZE, 100);
        let wide = wrap_text("WWWWWWWWWW", MSG_SIZE, 100);

        assert_eq!(narrow.len(), 1);
        assert!(wide.len() > 1);
    }

    /// Words wider than the bubble are hard-broken instead of overflowing.
    #[test]
    fn oversized_word_is_broken() {
        let lines = wrap_text("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", MSG_SIZE, MAX_TEXT_W);
        for line in &lines {
            assert!(measure_text_width(line, MSG_SIZE) <= MAX_TEXT_W);
        }
    }

    /// Accent colours are stable per sender.
    #[test]
    fn accent_colour_is_deterministic() {
        assert_eq!(accent_colour("Enjha"), accent_colour("Enjha"));
    }

    /// End-to-end render test against the user's reference cases:
    /// 1. Short: "YouTheSlayer" / "Hmm"
    /// 2. Medium: "Enjha" / "Astaghfirullah Arab"
    /// 3. Wrapped: "User" / "apakah memang seperti itu atau sebenarnya ada penyebab lain?"
    /// Writes WebP files to disk/tmp/ for visual inspection.
    #[test]
    fn render_reference_cases() {
        let cases = [
            ("YouTheSlayer", "Hmm", "case1_short.webp"),
            ("Enjha", "Sumpah Cimob cupu", "case2_medium.webp"),
            (
                "ICB-ESBIR",
                "@Syplayerpepep anak kontol",
                "case3_compact.webp",
            ),
            (
                "User",
                "apakah memang seperti itu atau sebenarnya ada penyebab lain?",
                "case4_wrapped.webp",
            ),
        ];

        let tmp = crate::utils::media::temp_dir();
        for (name, text, filename) in cases {
            let bytes = render_quote(name, text, None).expect("render_quote failed");
            assert!(!bytes.is_empty(), "rendered webp was empty");
            let p = tmp.join(filename);
            std::fs::write(&p, &bytes).expect("write sample failed");
            let _ = std::fs::remove_file(&p);
        }

        // Also test with a real image as profile photo
        if let Ok(banner_bytes) = std::fs::read("disk/welcome-banner.jpg") {
            let photo_webp = render_quote(
                "YouTheSlayer",
                "Tes dengan foto profil real!",
                Some(&banner_bytes),
            )
            .expect("render_quote with photo failed");
            let p = tmp.join("case_with_photo.webp");
            std::fs::write(&p, &photo_webp).expect("write photo sample failed");
            let _ = std::fs::remove_file(&p);
        }
    }
}
