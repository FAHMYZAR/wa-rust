use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;

/// Renders a pairing payload as a scannable block-character QR.
pub fn render_ascii(payload: &str) -> qrcode::QrResult<String> {
    let code = QrCode::new(payload.as_bytes())?;
    Ok(code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Dark)
        .light_color(Dense1x2::Light)
        .quiet_zone(true)
        .build())
}
